use std::path::PathBuf;

use tokio::sync::mpsc;

use crate::cli;
use crate::config;
use crate::docker;
use crate::env;
use crate::formatter;
use crate::lifecycle;
use crate::message;
use crate::startup;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("display channel closed: {0}")]
    DisplayChannelClosed(mpsc::error::SendError<message::DisplayMsg>),

    #[error(transparent)]
    Docker(#[from] docker::Error),

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
    let names: Vec<String> = config.processes.keys().cloned().collect();

    let (event_tx, event_rx) = mpsc::unbounded_channel::<message::Event>();
    let (display_tx, display_rx) = mpsc::unbounded_channel::<message::DisplayMsg>();

    let _formatter_handle = formatter::spawn_task(&names, display_rx, log_tx)?;

    let start_order = startup::resolve_start_order(&config);

    let shutdown = match mode {
        cli::Command::Run(_) => lifecycle::ShutdownStrategy::MainProcess(
            start_order
                .last()
                .ok_or_else(|| Error::ProcessNotFound {
                    name: "main".to_string(),
                })?
                .clone(),
        ),
        cli::Command::Up(_) => lifecycle::ShutdownStrategy::AnyExit,
    };

    let first_docker_name = start_order
        .iter()
        .find(|name| {
            config
                .processes
                .get(name.as_str())
                .is_some_and(|proc| proc.docker.is_some())
        })
        .cloned();

    let network_name = match &first_docker_name {
        Some(docker_name) => {
            let net = generate_network_name(project_name.as_deref());
            send_display(
                &display_tx,
                message::DisplayMsg::CreatingNetwork {
                    name: docker_name.clone(),
                },
            )?;
            docker::network_create(&net).await?;
            Some(net)
        }
        None => None,
    };

    if first_docker_name.is_some() {
        build_images(&config, &start_order, &display_tx).await?;
    }

    let global_env = env::load_global(global_env_file.as_deref())?;

    let result = match startup::start_all(
        &config,
        &start_order,
        &global_env,
        &event_tx,
        &display_tx,
        event_rx,
        network_name.as_deref(),
    )
    .await?
    {
        startup::StartupResult::CrashedDuringStartup { code, failed_name } => {
            Ok(message::ExitResult {
                code,
                failed_name: Some(failed_name),
            })
        }
        startup::StartupResult::Ready(cmd_senders, lifecycle_rx) => {
            let all_names: Vec<String> =
                cmd_senders.iter().map(|(name, _tx)| name.clone()).collect();
            drop(event_tx);
            lifecycle::manage(cmd_senders, &all_names, shutdown, &display_tx, lifecycle_rx)
                .await
                .map_err(Error::from)
        }
    };

    if let (Some(net), Some(tag_name)) = (&network_name, &first_docker_name) {
        let docker_names: Vec<String> = start_order
            .iter()
            .filter(|name| {
                config
                    .processes
                    .get(name.as_str())
                    .is_some_and(|proc| proc.docker.is_some())
            })
            .cloned()
            .collect();

        cleanup_docker(&docker_names, net, tag_name, &display_tx).await;
    }

    result
}

async fn cleanup_docker(
    docker_names: &[String],
    network: &str,
    tag_name: &str,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) {
    if !docker_names.is_empty() {
        if let Err(_send_err) = send_display(
            display_tx,
            message::DisplayMsg::RemovingContainers {
                name: tag_name.to_string(),
            },
        ) {}
        for name in docker_names {
            let container = docker::container_name(network, name);
            if let Err(rm_err) = docker::rm(&container).await
                && let Err(_send_err) = send_display(
                    display_tx,
                    message::DisplayMsg::CleanupError {
                        name: name.clone(),
                        reason: rm_err.to_string(),
                    },
                )
            {}
        }
    }

    if let Err(_send_err) = send_display(
        display_tx,
        message::DisplayMsg::RemovingNetwork {
            name: tag_name.to_string(),
        },
    ) {}
    if let Err(rm_err) = docker::network_rm(network).await
        && let Err(_send_err) = send_display(
            display_tx,
            message::DisplayMsg::CleanupError {
                name: tag_name.to_string(),
                reason: rm_err.to_string(),
            },
        )
    {}
}

async fn build_images(
    config: &config::Config,
    start_order: &[String],
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<(), Error> {
    for name in start_order {
        let Some(proc_def) = config.processes.get(name) else {
            continue;
        };
        let Some(docker_config) = &proc_def.docker else {
            continue;
        };
        let Some(build_context) = &docker_config.build else {
            continue;
        };

        send_display(
            display_tx,
            message::DisplayMsg::Building { name: name.clone() },
        )?;

        let tag = match &docker_config.image {
            Some(image) => image.as_str(),
            None => name,
        };
        docker::build(tag, build_context, docker_config.dockerfile.as_deref()).await?;

        send_display(
            display_tx,
            message::DisplayMsg::Built { name: name.clone() },
        )?;
    }

    Ok(())
}

fn generate_network_name(project_name: Option<&str>) -> String {
    let pid = std::process::id();
    match project_name {
        Some(name) => format!("sc-{name}-{pid}"),
        None => format!("sc-{pid}"),
    }
}

fn send_display(
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    msg: message::DisplayMsg,
) -> Result<(), Error> {
    display_tx.send(msg).map_err(Error::DisplayChannelClosed)
}
