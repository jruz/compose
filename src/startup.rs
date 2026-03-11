use std::collections::{BTreeMap, HashSet};

use tokio::sync::mpsc;

use crate::config;
use crate::docker_actor;
use crate::env;
use crate::message;
use crate::process;
use crate::shutdown;

#[derive(Debug, thiserror::Error)]
pub enum Error {
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

    #[error(transparent)]
    Shutdown(#[from] shutdown::Error),
}

pub struct Channels {
    pub events: mpsc::UnboundedSender<message::Event>,
    pub notify: mpsc::UnboundedSender<message::Event>,
    pub output: mpsc::UnboundedSender<message::DisplayMsg>,
}

pub enum StartupResult {
    CrashedDuringStartup { code: u8, failed_name: String },
    Ready(Vec<(String, mpsc::UnboundedSender<message::Command>)>),
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
    channels: &Channels,
    event_rx: &mut mpsc::UnboundedReceiver<message::Event>,
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

        shutdown::send_event(
            &channels.notify,
            message::Event::Starting {
                name: name.clone(),
                command: start_label,
            },
        )?;

        let env = env::resolve(global_env, proc_def)?;

        let cmd_tx = spawn_process_actor(
            name,
            proc_def,
            shell,
            &env,
            network,
            &channels.events,
            &channels.output,
        )?;

        if is_oneshot {
            if let Some(result) =
                handle_oneshot(name, &cmd_senders, event_rx, &channels.notify).await?
            {
                return Ok(result);
            }
        } else {
            cmd_senders.push((name.clone(), cmd_tx));

            if proc_def.health.is_some() {
                shutdown::send_event(
                    &channels.notify,
                    message::Event::WaitingHealth { name: name.clone() },
                )?;
            }

            match start_service(name, proc_def, &cmd_senders, event_rx, &channels.notify).await? {
                ServiceOutcome::Ready => {}
                ServiceOutcome::CrashedDuringStartup { code, failed_name } => {
                    return Ok(StartupResult::CrashedDuringStartup { code, failed_name });
                }
            }
        }
    }

    Ok(StartupResult::Ready(cmd_senders))
}

async fn handle_oneshot(
    name: &str,
    cmd_senders: &[(String, mpsc::UnboundedSender<message::Command>)],
    event_rx: &mut mpsc::UnboundedReceiver<message::Event>,
    notify_tx: &mpsc::UnboundedSender<message::Event>,
) -> Result<Option<StartupResult>, Error> {
    shutdown::send_event(
        notify_tx,
        message::Event::WaitingCompletion {
            name: name.to_string(),
        },
    )?;
    match start_oneshot(name, cmd_senders, event_rx, notify_tx).await? {
        OneshotOutcome::Completed => {
            shutdown::send_event(
                notify_tx,
                message::Event::Completed {
                    name: name.to_string(),
                },
            )?;
            Ok(None)
        }
        OneshotOutcome::Failed { code } => {
            shutdown::send_event(
                notify_tx,
                message::Event::Exited {
                    name: name.to_string(),
                    code,
                    stderr_lines: vec![],
                },
            )?;
            shutdown::kill_all(cmd_senders);
            Ok(Some(StartupResult::CrashedDuringStartup {
                code: 1,
                failed_name: name.to_string(),
            }))
        }
        OneshotOutcome::OtherCrashed {
            crashed_name,
            code,
            stderr_lines,
        } => {
            let result = handle_crash_during_startup(
                &crashed_name,
                code,
                &stderr_lines,
                cmd_senders,
                notify_tx,
            )?;
            Ok(Some(result))
        }
    }
}

async fn start_oneshot(
    name: &str,
    cmd_senders: &[(String, mpsc::UnboundedSender<message::Command>)],
    event_rx: &mut mpsc::UnboundedReceiver<message::Event>,
    notify_tx: &mpsc::UnboundedSender<message::Event>,
) -> Result<OneshotOutcome, Error> {
    loop {
        let event = event_rx.recv().await.ok_or(Error::EventChannelClosed)?;

        match event {
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
                shutdown::send_event(
                    notify_tx,
                    message::Event::HealthFailed {
                        name: event_name.clone(),
                        reason: reason.clone(),
                    },
                )?;
                shutdown::kill_all(cmd_senders);
                return Err(Error::HealthFailed {
                    name: event_name,
                    reason,
                });
            }
            message::Event::Completed { .. }
            | message::Event::Crashed { .. }
            | message::Event::Healthy { .. }
            | message::Event::Killing { .. }
            | message::Event::Starting { .. }
            | message::Event::WaitingCompletion { .. }
            | message::Event::WaitingHealth { .. } => {}
        }
    }
}

async fn start_service(
    name: &str,
    proc_def: &config::ProcessDef,
    cmd_senders: &[(String, mpsc::UnboundedSender<message::Command>)],
    event_rx: &mut mpsc::UnboundedReceiver<message::Event>,
    notify_tx: &mpsc::UnboundedSender<message::Event>,
) -> Result<ServiceOutcome, Error> {
    loop {
        let event = event_rx.recv().await.ok_or(Error::EventChannelClosed)?;

        match &event {
            message::Event::Healthy { name: event_name } if event_name == name => {
                if proc_def.health.is_some() {
                    shutdown::send_event(
                        notify_tx,
                        message::Event::Healthy {
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
                shutdown::send_event(
                    notify_tx,
                    message::Event::HealthFailed {
                        name: event_name.clone(),
                        reason: reason.clone(),
                    },
                )?;
                shutdown::kill_all(cmd_senders);
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
                shutdown::send_event(
                    notify_tx,
                    message::Event::Crashed {
                        name: event_name.clone(),
                        code: *code,
                        stderr_lines: stderr_lines.clone(),
                    },
                )?;
                let remaining: Vec<String> = cmd_senders
                    .iter()
                    .map(|(nm, _)| nm.clone())
                    .filter(|nm| nm != event_name)
                    .collect();
                shutdown::emit_killing_all(
                    &remaining,
                    &format!("{event_name} crashed"),
                    notify_tx,
                )?;
                shutdown::kill_all(cmd_senders);
                return Ok(ServiceOutcome::CrashedDuringStartup {
                    code: 1,
                    failed_name: event_name.clone(),
                });
            }
            message::Event::Completed { .. }
            | message::Event::Crashed { .. }
            | message::Event::Exited { .. }
            | message::Event::Healthy { .. }
            | message::Event::Killing { .. }
            | message::Event::Starting { .. }
            | message::Event::WaitingCompletion { .. }
            | message::Event::WaitingHealth { .. } => {}
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
    output_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<mpsc::UnboundedSender<message::Command>, Error> {
    if let (Some(docker_config), Some(net)) = (&proc_def.docker, network) {
        let image = docker_image_tag(name, docker_config);
        let volumes: Vec<String> = docker_config
            .volumes
            .iter()
            .map(|vol| env::expand_vars(vol, env))
            .collect();
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
            volumes: &volumes,
        };
        let cmd_tx = docker_actor::spawn_actor(&run_config, event_tx.clone(), output_tx.clone())?;
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
            output_tx.clone(),
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
    notify_tx: &mpsc::UnboundedSender<message::Event>,
) -> Result<StartupResult, Error> {
    shutdown::send_event(
        notify_tx,
        message::Event::Crashed {
            name: crashed_name.to_string(),
            code,
            stderr_lines: stderr_lines.to_vec(),
        },
    )?;
    let remaining: Vec<String> = cmd_senders
        .iter()
        .map(|(nm, _)| nm.clone())
        .filter(|nm| nm != crashed_name)
        .collect();
    shutdown::emit_killing_all(&remaining, &format!("{crashed_name} crashed"), notify_tx)?;
    shutdown::kill_all(cmd_senders);
    Ok(StartupResult::CrashedDuringStartup {
        code: 1,
        failed_name: crashed_name.to_string(),
    })
}
