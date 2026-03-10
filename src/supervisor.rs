use std::path::PathBuf;

use tokio::io::BufReader;
use tokio::sync::mpsc;

use crate::actor;
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
    let has_docker = config.processes.values().any(|proc| proc.docker.is_some());
    let mut names: Vec<String> = config.processes.keys().cloned().collect();
    if has_docker {
        names.push("docker".to_string());
    }

    let (event_tx, event_rx) = mpsc::unbounded_channel::<message::Event>();
    let (display_tx, display_rx) = mpsc::unbounded_channel::<message::DisplayMsg>();
    let (notify_tx, mut notify_rx) = mpsc::unbounded_channel::<message::Event>();

    let translator_tx = display_tx.clone();
    let _translator_handle = tokio::spawn(async move {
        while let Some(event) = notify_rx.recv().await {
            for msg in event_to_display(&event) {
                if let Err(_send_err) = translator_tx.send(msg) {
                    break;
                }
            }
        }
    });

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

    let network_name = if has_docker {
        let net = generate_network_name(project_name.as_deref());
        send_display(
            &display_tx,
            message::DisplayMsg::Docker(message::DockerMsg::CreatingNetwork {
                name: "docker".to_string(),
            }),
        )?;
        docker::network_create(&net).await?;
        build_images(&config, &start_order, &display_tx).await?;
        Some(net)
    } else {
        None
    };

    let global_env = env::load_global(global_env_file.as_deref())?;

    let startup_channels = startup::Channels {
        events: event_tx.clone(),
        notify: notify_tx.clone(),
        output: display_tx.clone(),
    };

    let result = match startup::start_all(
        &config,
        &start_order,
        &global_env,
        &startup_channels,
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
            lifecycle::manage(cmd_senders, &all_names, shutdown, &notify_tx, lifecycle_rx)
                .await
                .map_err(Error::from)
        }
    };

    if let Some(net) = &network_name {
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

        cleanup_docker(&docker_names, net, &display_tx).await;
    }

    result
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

async fn cleanup_docker(
    docker_names: &[String],
    network: &str,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) {
    if !docker_names.is_empty() {
        if let Err(_send_err) = send_display(
            display_tx,
            message::DisplayMsg::Docker(message::DockerMsg::RemovingContainers {
                name: "docker".to_string(),
            }),
        ) {}
        for name in docker_names {
            let container = docker::container_name(network, name);
            if let Err(rm_err) = docker::rm(&container).await
                && let Err(_send_err) = send_display(
                    display_tx,
                    message::DisplayMsg::Docker(message::DockerMsg::CleanupError {
                        name: "docker".to_string(),
                        reason: rm_err.to_string(),
                    }),
                )
            {}
        }
    }

    if let Err(_send_err) = send_display(
        display_tx,
        message::DisplayMsg::Docker(message::DockerMsg::RemovingNetwork {
            name: "docker".to_string(),
        }),
    ) {}
    if let Err(rm_err) = docker::network_rm(network).await
        && let Err(_send_err) = send_display(
            display_tx,
            message::DisplayMsg::Docker(message::DockerMsg::CleanupError {
                name: "docker".to_string(),
                reason: rm_err.to_string(),
            }),
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
            message::DisplayMsg::Docker(message::DockerMsg::Building { name: name.clone() }),
        )?;

        let tag = match &docker_config.image {
            Some(image) => image.as_str(),
            None => name,
        };

        let mut child =
            docker::build_command(tag, build_context, docker_config.dockerfile.as_deref())
                .spawn()
                .map_err(|source| docker::Error::Build {
                    context: build_context.clone(),
                    source,
                })?;

        let stdout_handle = child.stdout.take().map(|stdout| {
            let tx = display_tx.clone();
            let proc_name = name.clone();
            tokio::spawn(async move {
                actor::stream_lines(&proc_name, BufReader::new(stdout), &tx).await;
            })
        });

        let stderr_handle = child.stderr.take().map(|stderr| {
            let tx = display_tx.clone();
            let proc_name = name.clone();
            tokio::spawn(async move {
                actor::stream_lines(&proc_name, BufReader::new(stderr), &tx).await;
            })
        });

        let status = child.wait().await.map_err(|source| docker::Error::Build {
            context: build_context.clone(),
            source,
        })?;

        if let Some(handle) = stdout_handle {
            drop(handle.await);
        }
        if let Some(handle) = stderr_handle {
            drop(handle.await);
        }

        if !status.success() {
            let Some(code) = status.code() else {
                return Err(docker::Error::BuildKilled {
                    context: build_context.clone(),
                }
                .into());
            };
            return Err(docker::Error::BuildFailed {
                context: build_context.clone(),
                code,
            }
            .into());
        }

        send_display(
            display_tx,
            message::DisplayMsg::Docker(message::DockerMsg::Built { name: name.clone() }),
        )?;
    }

    Ok(())
}

fn generate_network_name(project_name: Option<&str>) -> String {
    let pid = std::process::id();
    match project_name {
        Some(name) => format!("compose-{name}-{pid}"),
        None => format!("compose-{pid}"),
    }
}

fn send_display(
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    msg: message::DisplayMsg,
) -> Result<(), Error> {
    display_tx.send(msg).map_err(Error::DisplayChannelClosed)
}
