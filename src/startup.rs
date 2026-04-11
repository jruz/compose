use std::collections::{BTreeMap, HashSet};

use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::config;
use crate::docker;
use crate::docker_actor;
use crate::env;
use crate::health;
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

pub struct Channels<'cfg> {
    pub binary: &'cfg str,
    pub display: mpsc::UnboundedSender<message::DisplayMsg>,
    pub events: mpsc::UnboundedSender<message::ActorExited>,
}

pub enum StartupResult {
    CrashedDuringStartup { code: u8, failed_name: String },
    Ready,
}

enum OneshotOutcome {
    Completed,
    Failed {
        code: u8,
    },
    OtherCrashed {
        code: u8,
        crashed_name: String,
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
            let all_deps_resolved = proc_def.depends_on.iter().all(|dep| {
                let dep_str = dep.as_str();
                resolved.contains(dep_str)
            });

            if all_deps_resolved {
                let owned = name.to_string();
                order.push(owned);
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
    channels: &Channels<'_>,
    event_rx: &mut mpsc::UnboundedReceiver<message::ActorExited>,
    network: Option<&str>,
) -> Result<(StartupResult, Vec<message::ActorHandle>), Error> {
    let shell = match &config.shell {
        Some(shell) => shell.as_str(),
        None => "sh",
    };
    let mut actors: Vec<message::ActorHandle> = Vec::new();
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
            (None, Some(_)) => format!("{}:{name}", channels.binary),
            (None, None) => continue,
        };

        send_lifecycle(
            &channels.display,
            message::LifecycleMsg::Starting {
                name: name.clone(),
                command: start_label,
            },
        )?;

        let env = env::resolve(global_env, proc_def)?;

        let actor = spawn_process_actor(name, proc_def, shell, &env, network, channels)?;

        if is_oneshot {
            if let Some(result) = handle_oneshot(name, &actors, event_rx, &channels.display).await?
            {
                actors.push(actor);
                return Ok((result, actors));
            }
            drop(actor.join.await);
        } else {
            actors.push(actor);

            let health_info = build_health_info(proc_def, &env, name, network, channels.binary);

            if health_info.is_some() {
                send_lifecycle(
                    &channels.display,
                    message::LifecycleMsg::WaitingHealth { name: name.clone() },
                )?;
            }

            match start_service(name, health_info, &actors, event_rx, &channels.display).await? {
                ServiceOutcome::Ready => {}
                ServiceOutcome::CrashedDuringStartup { code, failed_name } => {
                    return Ok((
                        StartupResult::CrashedDuringStartup { code, failed_name },
                        actors,
                    ));
                }
            }
        }
    }

    Ok((StartupResult::Ready, actors))
}

fn build_health_info(
    proc_def: &config::ProcessDef,
    env: &BTreeMap<String, String>,
    name: &str,
    network: Option<&str>,
    binary: &str,
) -> Option<(health::HealthConfig, health::HealthDriver)> {
    let hc = proc_def.health.as_ref()?;
    let expanded = expand_health(hc, env);
    let config = health::parse_config(&expanded);
    let driver = if let Some(net) = network.filter(|_| proc_def.docker.is_some()) {
        health::HealthDriver::Docker {
            binary: binary.to_string(),
            container: docker::container_name(net, name),
        }
    } else {
        health::HealthDriver::Exec
    };
    Some((config, driver))
}

async fn handle_oneshot(
    name: &str,
    actors: &[message::ActorHandle],
    event_rx: &mut mpsc::UnboundedReceiver<message::ActorExited>,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<Option<StartupResult>, Error> {
    send_lifecycle(
        display_tx,
        message::LifecycleMsg::WaitingCompletion {
            name: name.to_string(),
        },
    )?;
    match start_oneshot(name, event_rx).await? {
        OneshotOutcome::Completed => {
            send_lifecycle(
                display_tx,
                message::LifecycleMsg::Completed {
                    name: name.to_string(),
                },
            )?;
            Ok(None)
        }
        OneshotOutcome::Failed { code } => {
            send_lifecycle(
                display_tx,
                message::LifecycleMsg::Exited {
                    name: name.to_string(),
                    code,
                },
            )?;
            shutdown::kill_all(actors);
            let result = StartupResult::CrashedDuringStartup {
                code: 1,
                failed_name: name.to_string(),
            };
            let option = Some(result);
            Ok(option)
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
                actors,
                display_tx,
            )?;
            let option = Some(result);
            Ok(option)
        }
    }
}

async fn start_oneshot(
    name: &str,
    event_rx: &mut mpsc::UnboundedReceiver<message::ActorExited>,
) -> Result<OneshotOutcome, Error> {
    let event = event_rx.recv().await.ok_or(Error::EventChannelClosed)?;
    if event.name == name {
        if event.code == 0 {
            return Ok(OneshotOutcome::Completed);
        }
        return Ok(OneshotOutcome::Failed { code: event.code });
    }
    Ok(OneshotOutcome::OtherCrashed {
        code: event.code,
        crashed_name: event.name,
        stderr_lines: event.stderr_lines,
    })
}

async fn start_service(
    name: &str,
    health_info: Option<(health::HealthConfig, health::HealthDriver)>,
    actors: &[message::ActorHandle],
    event_rx: &mut mpsc::UnboundedReceiver<message::ActorExited>,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<ServiceOutcome, Error> {
    let Some((health_cfg, health_driver)) = health_info else {
        return Ok(ServiceOutcome::Ready);
    };

    let deadline = Instant::now() + health_cfg.timeout;

    loop {
        tokio::select! {
            event = event_rx.recv() => {
                let event = event.ok_or(Error::EventChannelClosed)?;
                if event.name == name {
                    let reason = "exited before becoming healthy".to_string();
                    send_lifecycle(
                        display_tx,
                        message::LifecycleMsg::HealthFailed {
                            name: name.to_string(),
                            reason: reason.clone(),
                        },
                    )?;
                    shutdown::kill_all(actors);
                    return Err(Error::HealthFailed {
                        name: name.to_string(),
                        reason,
                    });
                }
                send_crashed(display_tx, &event.name, event.code, &event.stderr_lines)?;
                shutdown::emit_killing_all(
                    actors,
                    Some(&event.name),
                    &format!("{} crashed", event.name),
                    display_tx,
                )?;
                shutdown::kill_all(actors);
                return Ok(ServiceOutcome::CrashedDuringStartup {
                    code: 1,
                    failed_name: event.name,
                });
            }
            () = tokio::time::sleep(health_cfg.interval) => {
                match health::check(&health_driver, &health_cfg.command).await {
                    Ok(output) if output.status.success() => {
                        send_lifecycle(
                            display_tx,
                            message::LifecycleMsg::Healthy {
                                name: name.to_string(),
                            },
                        )?;
                        return Ok(ServiceOutcome::Ready);
                    }
                    _ => {
                        if Instant::now() >= deadline {
                            let reason = format!(
                                "health check timed out after {}s",
                                health_cfg.timeout.as_secs()
                            );
                            send_lifecycle(
                                display_tx,
                                message::LifecycleMsg::HealthFailed {
                                    name: name.to_string(),
                                    reason: reason.clone(),
                                },
                            )?;
                            shutdown::kill_all(actors);
                            return Err(Error::HealthFailed {
                                name: name.to_string(),
                                reason,
                            });
                        }
                    }
                }
            }
        }
    }
}

fn expand_health(
    hc: &config::HealthCheck,
    env: &std::collections::BTreeMap<String, String>,
) -> config::HealthCheck {
    config::HealthCheck {
        command: hc.command.as_ref().map(|cmd| env::expand_vars(cmd, env)),
        exec: hc
            .exec
            .as_ref()
            .map(|args| args.iter().map(|arg| env::expand_vars(arg, env)).collect()),
        interval_secs: hc.interval_secs,
        timeout_secs: hc.timeout_secs,
    }
}

fn spawn_process_actor(
    name: &str,
    proc_def: &config::ProcessDef,
    shell: &str,
    env: &std::collections::BTreeMap<String, String>,
    network: Option<&str>,
    channels: &Channels<'_>,
) -> Result<message::ActorHandle, Error> {
    let expanded_command = proc_def
        .command
        .as_ref()
        .map(|cmd| env::expand_vars(cmd, env));
    let expanded_exec: Option<Vec<String>> = proc_def
        .exec
        .as_ref()
        .map(|args| args.iter().map(|arg| env::expand_vars(arg, env)).collect());
    if let (Some(docker_config), Some(net)) = (&proc_def.docker, network) {
        let image_ref = docker_config.image.as_deref();
        let image = docker::image_tag(net, name, image_ref);
        let ports: Vec<String> = docker_config
            .ports
            .iter()
            .map(|port| env::expand_vars(port, env))
            .collect();
        let volumes: Vec<String> = docker_config
            .volumes
            .iter()
            .map(|vol| env::expand_vars(vol, env))
            .collect();
        let run_config = docker_actor::RunConfig {
            binary: channels.binary,
            command: expanded_command.as_deref(),
            env,
            exec: expanded_exec.as_deref(),
            image: &image,
            name,
            network: net,
            ports: &ports,
            stop_timeout_secs: docker_config.stop_timeout_secs,
            volumes: &volumes,
        };
        let events = channels.events.clone();
        let display = channels.display.clone();
        let actor = docker_actor::spawn_actor(&run_config, events, display)?;
        Ok(actor)
    } else {
        let expanded_command = expanded_command.ok_or_else(|| Error::MissingCommand {
            name: name.to_string(),
        })?;
        let events = channels.events.clone();
        let display = channels.display.clone();
        let actor = process::spawn_actor(name, &expanded_command, shell, env, events, display)?;
        Ok(actor)
    }
}

fn handle_crash_during_startup(
    crashed_name: &str,
    code: u8,
    stderr_lines: &[String],
    actors: &[message::ActorHandle],
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<StartupResult, Error> {
    send_crashed(display_tx, crashed_name, code, stderr_lines)?;
    let reason = format!("{crashed_name} crashed");
    let exclude = Some(crashed_name);
    shutdown::emit_killing_all(actors, exclude, &reason, display_tx)?;
    shutdown::kill_all(actors);
    Ok(StartupResult::CrashedDuringStartup {
        code: 1,
        failed_name: crashed_name.to_string(),
    })
}

fn send_lifecycle(
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    msg: message::LifecycleMsg,
) -> Result<(), Error> {
    let display_msg = message::DisplayMsg::Lifecycle(msg);
    shutdown::send_display(display_tx, display_msg).map_err(Error::from)
}

fn send_crashed(
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    name: &str,
    code: u8,
    stderr_lines: &[String],
) -> Result<(), Error> {
    send_lifecycle(
        display_tx,
        message::LifecycleMsg::Crashed {
            name: name.to_string(),
            code,
        },
    )?;
    if !stderr_lines.is_empty() {
        send_lifecycle(
            display_tx,
            message::LifecycleMsg::Summary {
                name: name.to_string(),
                stderr_lines: stderr_lines.to_vec(),
            },
        )?;
    }
    Ok(())
}
