use std::path::PathBuf;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::cli;
use crate::config;
use crate::docker_infra;
use crate::env;
use crate::formatter;
use crate::lifecycle;
use crate::message;
use crate::startup;

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
    project_name: Option<String>,
    log_tx: mpsc::UnboundedSender<String>,
) -> Result<message::ExitResult, Error> {
    let has_docker = config.processes.values().any(|proc| proc.docker.is_some());
    let binary = config::runtime_binary(&config.runtime);
    let names = display_names(&config, has_docker, binary);

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let (display_tx, display_rx) = mpsc::unbounded_channel();
    let (notify_tx, notify_rx) = mpsc::unbounded_channel();
    let translator_handle = spawn_translator(notify_rx, display_tx.clone());
    let formatter_handle = formatter::spawn_task(&names, display_rx, log_tx)?;

    let start_order = startup::resolve_start_order(&config);
    let shutdown = resolve_shutdown(&mode, &start_order)?;
    check_main_not_oneshot(&shutdown, &config)?;
    let network_name = if has_docker {
        Some(
            docker_infra::setup(
                binary,
                &config,
                &start_order,
                project_name.as_deref(),
                &display_tx,
            )
            .await?,
        )
    } else {
        None
    };
    let global_env = env::load_global(global_env_file.as_deref())?;

    let channels = startup::Channels {
        binary,
        events: event_tx.clone(),
        notify: notify_tx.clone(),
        output: display_tx.clone(),
    };
    let startup_result = startup::start_all(
        &config,
        &start_order,
        &global_env,
        &channels,
        &mut event_rx,
        network_name.as_deref(),
    )
    .await?;
    let result = run_lifecycle(startup_result, event_tx, shutdown, &notify_tx, event_rx).await;

    if let Some(net) = &network_name {
        docker_infra::cleanup(binary, &config, &start_order, net, &display_tx).await;
    }
    drop(notify_tx);
    drop(channels);
    drop(translator_handle.await);
    drop(display_tx);
    drop(formatter_handle.await);
    result
}

fn display_names(config: &config::Config, has_docker: bool, runtime_label: &str) -> Vec<String> {
    let mut names: Vec<String> = config.processes.keys().cloned().collect();
    if has_docker {
        names.push(runtime_label.to_string());
    }
    names
}

async fn run_lifecycle(
    startup_result: startup::StartupResult,
    event_tx: mpsc::UnboundedSender<message::Event>,
    shutdown: lifecycle::ShutdownStrategy,
    notify_tx: &mpsc::UnboundedSender<message::Event>,
    event_rx: mpsc::UnboundedReceiver<message::Event>,
) -> Result<message::ExitResult, Error> {
    match startup_result {
        startup::StartupResult::CrashedDuringStartup { code, failed_name } => {
            Ok(message::ExitResult {
                code,
                failed_name: Some(failed_name),
            })
        }
        startup::StartupResult::Ready(cmd_senders) => {
            let all_names: Vec<String> =
                cmd_senders.iter().map(|(name, _tx)| name.clone()).collect();
            drop(event_tx);
            lifecycle::manage(cmd_senders, &all_names, shutdown, notify_tx, event_rx)
                .await
                .map_err(Error::from)
        }
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
        Err(config::Error::LastProcessOneshot { name: name.clone() }.into())
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
            Ok(lifecycle::ShutdownStrategy::MainProcess(main))
        }
        cli::Command::Up(_) => Ok(lifecycle::ShutdownStrategy::AnyExit),
    }
}

fn spawn_translator(
    mut notify_rx: mpsc::UnboundedReceiver<message::Event>,
    display_tx: mpsc::UnboundedSender<message::DisplayMsg>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(event) = notify_rx.recv().await {
            for msg in event_to_display(&event) {
                if let Err(_send_err) = display_tx.send(msg) {
                    break;
                }
            }
        }
    })
}

fn event_to_display(event: &message::Event) -> Vec<message::DisplayMsg> {
    match event {
        message::Event::Completed { name } => vec![message::DisplayMsg::Lifecycle(
            message::LifecycleMsg::Completed { name: name.clone() },
        )],
        message::Event::Crashed {
            name,
            code,
            stderr_lines,
        } => {
            let mut msgs = vec![message::DisplayMsg::Lifecycle(
                message::LifecycleMsg::Crashed {
                    name: name.clone(),
                    code: *code,
                },
            )];
            if !stderr_lines.is_empty() {
                msgs.push(message::DisplayMsg::Lifecycle(
                    message::LifecycleMsg::Summary {
                        name: name.clone(),
                        stderr_lines: stderr_lines.clone(),
                    },
                ));
            }
            msgs
        }
        message::Event::Exited { name, code, .. } => vec![message::DisplayMsg::Lifecycle(
            message::LifecycleMsg::Exited {
                name: name.clone(),
                code: *code,
            },
        )],
        message::Event::HealthFailed { name, reason } => vec![message::DisplayMsg::Lifecycle(
            message::LifecycleMsg::HealthFailed {
                name: name.clone(),
                reason: reason.clone(),
            },
        )],
        message::Event::Healthy { name } => vec![message::DisplayMsg::Lifecycle(
            message::LifecycleMsg::Healthy { name: name.clone() },
        )],
        message::Event::Killing { name, reason } => vec![message::DisplayMsg::Lifecycle(
            message::LifecycleMsg::Killing {
                name: name.clone(),
                reason: reason.clone(),
            },
        )],
        message::Event::Starting { name, command } => vec![message::DisplayMsg::Lifecycle(
            message::LifecycleMsg::Starting {
                name: name.clone(),
                command: command.clone(),
            },
        )],
        message::Event::WaitingCompletion { name } => vec![message::DisplayMsg::Lifecycle(
            message::LifecycleMsg::WaitingCompletion { name: name.clone() },
        )],
        message::Event::WaitingHealth { name } => vec![message::DisplayMsg::Lifecycle(
            message::LifecycleMsg::WaitingHealth { name: name.clone() },
        )],
    }
}
