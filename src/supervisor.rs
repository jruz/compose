use std::path::PathBuf;

use tokio::sync::mpsc;

use crate::cli;
use crate::config;
use crate::docker_infra;
use crate::env;
use crate::formatter;
use crate::lifecycle;
use crate::message;
use crate::startup;

const ACTOR_SHUTDOWN_TIMEOUT_SECS: u64 = 12;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::Error),

    #[error(transparent)]
    DockerInfra(#[from] docker_infra::Error),

    #[error(transparent)]
    Env(#[from] env::Error),

    #[error(transparent)]
    Formatter(#[from] formatter::Error),

    #[error(transparent)]
    Lifecycle(#[from] lifecycle::Error),

    #[error("process {name} not found")]
    ProcessNotFound { name: String },

    #[error(transparent)]
    Startup(#[from] startup::Error),
}

pub async fn run(
    config: config::Config,
    mode: cli::Command,
    global_env_file: Option<PathBuf>,
    project_name: String,
    log_tx: mpsc::UnboundedSender<String>,
) -> Result<message::ExitResult, Error> {
    let has_docker = config.processes.values().any(|proc| proc.docker.is_some());
    let binary = config::runtime_binary(&config.runtime);
    let names = display_names(&config, has_docker, binary);

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let (display_tx, display_rx) = mpsc::unbounded_channel();
    let formatter_handle = formatter::spawn_task(&names, display_rx, log_tx)?;

    let start_order = startup::resolve_start_order(&config);
    let shutdown = resolve_shutdown(&mode, &start_order)?;
    check_main_not_oneshot(&shutdown, &config)?;
    let env_path = global_env_file.as_deref();
    let global_env = env::load_global(env_path)?;

    let network_name = if has_docker {
        let project = &project_name;
        let network = docker_infra::setup(
            binary,
            &config,
            &start_order,
            project,
            &global_env,
            &display_tx,
        )
        .await?;
        Some(network)
    } else {
        None
    };

    let channels = startup::Channels {
        binary,
        display: display_tx.clone(),
        events: event_tx.clone(),
    };
    let network_ref = network_name.as_deref();
    let (startup_result, actors) = startup::start_all(
        &config,
        &start_order,
        &global_env,
        &channels,
        &mut event_rx,
        network_ref,
    )
    .await?;
    let result = run_lifecycle(
        startup_result,
        event_tx,
        shutdown,
        &display_tx,
        event_rx,
        &actors,
    )
    .await;

    await_actor_shutdown(actors, &display_tx).await;
    if let Some(net) = &network_name {
        docker_infra::cleanup(binary, &config, &start_order, net, &display_tx).await;
    }
    drop(channels);
    drop(display_tx);
    let formatter_result = formatter_handle.await;
    drop(formatter_result);
    result
}

fn display_names(config: &config::Config, has_docker: bool, runtime_label: &str) -> Vec<String> {
    let mut names: Vec<String> = config.processes.keys().cloned().collect();
    if has_docker {
        let label = runtime_label.to_string();
        names.push(label);
    }
    names
}

async fn run_lifecycle(
    startup_result: startup::StartupResult,
    event_tx: mpsc::UnboundedSender<message::ActorExited>,
    shutdown: lifecycle::ShutdownStrategy,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    event_rx: mpsc::UnboundedReceiver<message::ActorExited>,
    actors: &[message::ActorHandle],
) -> Result<message::ExitResult, Error> {
    match startup_result {
        startup::StartupResult::CrashedDuringStartup { code, failed_name } => {
            Ok(message::ExitResult {
                code,
                failed_name: Some(failed_name),
            })
        }
        startup::StartupResult::Ready => {
            drop(event_tx);
            lifecycle::manage(actors, shutdown, display_tx, event_rx)
                .await
                .map_err(Error::from)
        }
    }
}

async fn await_actor_shutdown(
    actors: Vec<message::ActorHandle>,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) {
    let mut actor_set = tokio::task::JoinSet::new();
    for actor in actors {
        actor_set.spawn(async move { drop(actor.join.await) });
    }
    let deadline = std::time::Duration::from_secs(ACTOR_SHUTDOWN_TIMEOUT_SECS);
    if let Err(elapsed) = tokio::time::timeout(deadline, async {
        while actor_set.join_next().await.is_some() {}
    })
    .await
    {
        let msg = message::DisplayMsg::Lifecycle(message::LifecycleMsg::Killing {
            name: "supervisor".to_string(),
            reason: format!("actor shutdown timed out after {elapsed}"),
        });
        let send_result = display_tx.send(msg);
        drop(send_result);
        actor_set.abort_all();
    }
}

fn check_main_not_oneshot(
    shutdown: &lifecycle::ShutdownStrategy,
    config: &config::Config,
) -> Result<(), Error> {
    let is_oneshot = matches!(shutdown, lifecycle::ShutdownStrategy::MainProcess(name)
        if config.processes.get(name).is_some_and(|proc| proc.process_type == config::ProcessType::Oneshot)
    );
    if let (true, lifecycle::ShutdownStrategy::MainProcess(name)) = (is_oneshot, shutdown) {
        let err: Error = config::Error::LastProcessOneshot { name: name.clone() }.into();
        Err(err)
    } else {
        Ok(())
    }
}

fn resolve_shutdown(
    mode: &cli::Command,
    start_order: &[String],
) -> Result<lifecycle::ShutdownStrategy, Error> {
    match mode {
        cli::Command::Run(_) => {
            let main = start_order
                .last()
                .ok_or_else(|| Error::ProcessNotFound {
                    name: "main".to_string(),
                })?
                .clone();
            let strategy = lifecycle::ShutdownStrategy::MainProcess(main);
            Ok(strategy)
        }
        cli::Command::Up(_) => Ok(lifecycle::ShutdownStrategy::AnyExit),
    }
}
