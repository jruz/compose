use std::collections::{BTreeMap, HashSet};

use tokio::sync::mpsc;

use crate::config;
use crate::docker_actor;
use crate::env;
use crate::message;
use crate::process;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("display channel closed: {0}")]
    DisplayChannelClosed(mpsc::error::SendError<message::DisplayMsg>),

    #[error(transparent)]
    DockerActor(#[from] docker_actor::Error),

    #[error(transparent)]
    Env(#[from] env::Error),

    #[error("event channel closed")]
    EventChannelClosed,

    #[error("{name}: {reason}")]
    HealthFailed { name: String, reason: String },

    #[error("{name}: missing command for exec process")]
    MissingCommand { name: String },

    #[error(transparent)]
    Process(#[from] process::Error),

    #[error("process {name} not found")]
    ProcessNotFound { name: String },
}

pub enum StartupResult {
    CrashedDuringStartup {
        code: u8,
        failed_name: String,
    },
    Ready(
        Vec<(String, mpsc::UnboundedSender<message::Command>)>,
        mpsc::UnboundedReceiver<message::Event>,
    ),
}

enum OneshotOutcome {
    Completed,
    Failed {
        code: u8,
    },
    OtherCrashed {
        crashed_name: String,
        code: u8,
        stderr_lines: Vec<String>,
    },
}

enum ServiceOutcome {
    CrashedDuringStartup { code: u8, failed_name: String },
    Ready,
}

pub fn resolve_start_order(config: &config::Config) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    let mut resolved: HashSet<&str> = HashSet::new();
    let mut remaining: Vec<&str> = config.processes.keys().map(String::as_str).collect();

    while !remaining.is_empty() {
        let mut progress = false;

        remaining.retain(|&name| {
            let Some(proc_def) = config.processes.get(name) else {
                return false;
            };
            let all_deps_resolved = proc_def
                .depends_on
                .iter()
                .all(|dep| resolved.contains(dep.as_str()));

            if all_deps_resolved {
                order.push(name.to_string());
                resolved.insert(name);
                progress = true;
                false
            } else {
                true
            }
        });

        if !progress {
            break;
        }
    }

    order
}

pub async fn start_all(
    config: &config::Config,
    start_order: &[String],
    global_env: &BTreeMap<String, String>,
    event_tx: &mpsc::UnboundedSender<message::Event>,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    mut event_rx: mpsc::UnboundedReceiver<message::Event>,
    network: Option<&str>,
) -> Result<StartupResult, Error> {
    let shell = match &config.shell {
        Some(shell) => shell.as_str(),
        None => "sh",
    };
    let mut cmd_senders: Vec<(String, mpsc::UnboundedSender<message::Command>)> = Vec::new();
    for name in start_order {
        let proc_def = config
            .processes
            .get(name)
            .ok_or_else(|| Error::ProcessNotFound { name: name.clone() })?;

        let is_docker = proc_def.docker.is_some();
        let is_oneshot = proc_def.process_type == config::ProcessType::Oneshot;

        if !is_docker && proc_def.command.is_none() {
            continue;
        }

        let start_label = match (&proc_def.command, &proc_def.docker) {
            (Some(cmd), _) => cmd.clone(),
            (None, Some(_)) => format!("docker:{name}"),
            (None, None) => continue,
        };

        send_display(
            display_tx,
            message::DisplayMsg::Starting {
                name: name.clone(),
                command: start_label,
            },
        )?;

        let env = env::resolve(global_env, proc_def)?;

        let cmd_tx =
            spawn_process_actor(name, proc_def, shell, &env, network, event_tx, display_tx)?;

        if is_oneshot {
            send_display(
                display_tx,
                message::DisplayMsg::WaitingCompletion { name: name.clone() },
            )?;
            match start_oneshot(name, &cmd_senders, &mut event_rx, display_tx).await? {
                OneshotOutcome::Completed => {
                    send_display(
                        display_tx,
                        message::DisplayMsg::Completed { name: name.clone() },
                    )?;
                }
                OneshotOutcome::Failed { code } => {
                    send_display(
                        display_tx,
                        message::DisplayMsg::Exited {
                            name: name.clone(),
                            code,
                        },
                    )?;
                    kill_all(&cmd_senders);
                    return Ok(StartupResult::CrashedDuringStartup {
                        code: 1,
                        failed_name: name.clone(),
                    });
                }
                OneshotOutcome::OtherCrashed {
                    crashed_name,
                    code,
                    stderr_lines,
                } => {
                    return handle_crash_during_startup(
                        &crashed_name,
                        code,
                        &stderr_lines,
                        &cmd_senders,
                        display_tx,
                    );
                }
            }
        } else {
            cmd_senders.push((name.clone(), cmd_tx));

            if proc_def.health.is_some() {
                send_display(
                    display_tx,
                    message::DisplayMsg::WaitingHealth { name: name.clone() },
                )?;
            }

            match start_service(name, proc_def, &cmd_senders, &mut event_rx, display_tx).await? {
                ServiceOutcome::Ready => {}
                ServiceOutcome::CrashedDuringStartup { code, failed_name } => {
                    return Ok(StartupResult::CrashedDuringStartup { code, failed_name });
                }
            }
        }
    }

    Ok(StartupResult::Ready(cmd_senders, event_rx))
}

async fn start_oneshot(
    name: &str,
    cmd_senders: &[(String, mpsc::UnboundedSender<message::Command>)],
    event_rx: &mut mpsc::UnboundedReceiver<message::Event>,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<OneshotOutcome, Error> {
    loop {
        let event = event_rx.recv().await.ok_or(Error::EventChannelClosed)?;

        match event {
            message::Event::Healthy { name: event_name } if event_name == name => {}
            message::Event::Healthy { .. } => {}
            message::Event::Exited {
                name: event_name,
                code,
                stderr_lines,
            } => {
                if event_name == name {
                    if code == 0 {
                        return Ok(OneshotOutcome::Completed);
                    }
                    return Ok(OneshotOutcome::Failed { code });
                }
                return Ok(OneshotOutcome::OtherCrashed {
                    crashed_name: event_name,
                    code,
                    stderr_lines,
                });
            }
            message::Event::HealthFailed {
                name: event_name,
                reason,
            } => {
                send_display(
                    display_tx,
                    message::DisplayMsg::HealthFailed {
                        name: event_name.clone(),
                        reason: reason.clone(),
                    },
                )?;
                kill_all(cmd_senders);
                return Err(Error::HealthFailed {
                    name: event_name,
                    reason,
                });
            }
        }
    }
}

async fn start_service(
    name: &str,
    proc_def: &config::ProcessDef,
    cmd_senders: &[(String, mpsc::UnboundedSender<message::Command>)],
    event_rx: &mut mpsc::UnboundedReceiver<message::Event>,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<ServiceOutcome, Error> {
    loop {
        let event = event_rx.recv().await.ok_or(Error::EventChannelClosed)?;

        match &event {
            message::Event::Healthy { name: event_name } if event_name == name => {
                if proc_def.health.is_some() {
                    send_display(
                        display_tx,
                        message::DisplayMsg::Healthy {
                            name: name.to_string(),
                        },
                    )?;
                }
                return Ok(ServiceOutcome::Ready);
            }
            message::Event::HealthFailed {
                name: event_name,
                reason,
            } => {
                send_display(
                    display_tx,
                    message::DisplayMsg::HealthFailed {
                        name: event_name.clone(),
                        reason: reason.clone(),
                    },
                )?;
                kill_all(cmd_senders);
                return Err(Error::HealthFailed {
                    name: event_name.clone(),
                    reason: reason.clone(),
                });
            }
            message::Event::Exited {
                name: event_name,
                code,
                stderr_lines,
            } if event_name != name => {
                send_display(
                    display_tx,
                    message::DisplayMsg::Crashed {
                        name: event_name.clone(),
                        code: *code,
                    },
                )?;
                let remaining: Vec<String> = cmd_senders
                    .iter()
                    .map(|(nm, _)| nm.clone())
                    .filter(|nm| nm != event_name)
                    .collect();
                emit_killing_all(&remaining, &format!("{event_name} crashed"), display_tx)?;
                if !stderr_lines.is_empty() {
                    send_display(
                        display_tx,
                        message::DisplayMsg::Summary {
                            name: event_name.clone(),
                            stderr_lines: stderr_lines.clone(),
                        },
                    )?;
                }
                kill_all(cmd_senders);
                return Ok(ServiceOutcome::CrashedDuringStartup {
                    code: 1,
                    failed_name: event_name.clone(),
                });
            }
            message::Event::Exited { .. } | message::Event::Healthy { .. } => {}
        }
    }
}

fn spawn_process_actor(
    name: &str,
    proc_def: &config::ProcessDef,
    shell: &str,
    env: &std::collections::BTreeMap<String, String>,
    network: Option<&str>,
    event_tx: &mpsc::UnboundedSender<message::Event>,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<mpsc::UnboundedSender<message::Command>, Error> {
    if let (Some(docker_config), Some(net)) = (&proc_def.docker, network) {
        let image = docker_image_tag(name, docker_config);
        let run_config = docker_actor::RunConfig {
            command: proc_def.command.as_deref(),
            env,
            exec: proc_def.exec.as_deref(),
            health: proc_def.health.as_ref(),
            image: &image,
            name,
            network: net,
            ports: &docker_config.ports,
            stop_timeout_secs: docker_config.stop_timeout_secs,
            volumes: &docker_config.volumes,
        };
        let cmd_tx = docker_actor::spawn_actor(&run_config, event_tx.clone(), display_tx.clone())?;
        Ok(cmd_tx)
    } else {
        let Some(command) = &proc_def.command else {
            return Err(Error::MissingCommand {
                name: name.to_string(),
            });
        };
        let cmd_tx = process::spawn_actor(
            name,
            command,
            shell,
            env,
            proc_def.health.as_ref(),
            event_tx.clone(),
            display_tx.clone(),
        )?;
        Ok(cmd_tx)
    }
}

fn docker_image_tag(name: &str, docker_config: &config::DockerConfig) -> String {
    match &docker_config.image {
        Some(image) => image.clone(),
        None => name.to_string(),
    }
}

fn handle_crash_during_startup(
    crashed_name: &str,
    code: u8,
    stderr_lines: &[String],
    cmd_senders: &[(String, mpsc::UnboundedSender<message::Command>)],
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<StartupResult, Error> {
    send_display(
        display_tx,
        message::DisplayMsg::Crashed {
            name: crashed_name.to_string(),
            code,
        },
    )?;
    let remaining: Vec<String> = cmd_senders
        .iter()
        .map(|(nm, _)| nm.clone())
        .filter(|nm| nm != crashed_name)
        .collect();
    emit_killing_all(&remaining, &format!("{crashed_name} crashed"), display_tx)?;
    if !stderr_lines.is_empty() {
        send_display(
            display_tx,
            message::DisplayMsg::Summary {
                name: crashed_name.to_string(),
                stderr_lines: stderr_lines.to_vec(),
            },
        )?;
    }
    kill_all(cmd_senders);
    Ok(StartupResult::CrashedDuringStartup {
        code: 1,
        failed_name: crashed_name.to_string(),
    })
}

fn send_display(
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    msg: message::DisplayMsg,
) -> Result<(), Error> {
    display_tx.send(msg).map_err(Error::DisplayChannelClosed)
}

fn kill_all(cmd_senders: &[(String, mpsc::UnboundedSender<message::Command>)]) {
    for (_name, tx) in cmd_senders {
        if let Err(_send_err) = tx.send(message::Command::Kill) {}
    }
}

fn emit_killing_all(
    names: &[String],
    reason: &str,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<(), Error> {
    for name in names {
        send_display(
            display_tx,
            message::DisplayMsg::Killing {
                name: name.clone(),
                reason: reason.to_string(),
            },
        )?;
    }
    Ok(())
}
