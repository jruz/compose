use std::collections::HashSet;
use std::os::unix::process::ExitStatusExt as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::signal;
use tokio::sync::mpsc;

use crate::cli::Command;
use crate::config::Config;
use crate::health;
use crate::process;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("dependency {dependency} not healthy when starting {process}")]
    DependencyNotHealthy { dependency: String, process: String },

    #[error(transparent)]
    Health(#[from] health::Error),

    #[error("exit code {code} out of u8 range for process {name}")]
    InvalidExitCode { code: i32, name: String },

    #[error("failed to kill process {name}: {source}")]
    Kill {
        name: String,
        source: std::io::Error,
    },

    #[error("log channel closed")]
    LogChannelClosed,

    #[error("no processes defined")]
    NoProcesses,

    #[error(transparent)]
    Process(#[from] process::Error),

    #[error("process at index {index} not found in config")]
    ProcessIndexOutOfBounds { index: usize },

    #[error("process {name} not found in config")]
    ProcessNotFound { name: String },

    #[error("failed to wait on process {name}: {source}")]
    ProcessWait {
        name: String,
        source: std::io::Error,
    },

    #[error("failed to register signal handler: {source}")]
    SignalHandler { source: std::io::Error },

    #[error("process {name} exited with unknown status")]
    UnknownExitStatus { name: String },
}

enum Event {
    ProcessExited {
        name: String,
        status: std::process::ExitStatus,
    },
    Signal,
}

pub async fn run(
    config: Config,
    mode: Command,
    log_tx: mpsc::UnboundedSender<String>,
) -> Result<u8, Error> {
    let pad_width = config
        .process
        .iter()
        .map(|proc| proc.name.len())
        .max()
        .ok_or(Error::NoProcesses)?;

    let mut children: Vec<process::SpawnedProcess> = Vec::new();
    let mut healthy_set: HashSet<String> = HashSet::new();

    let start_order = resolve_start_order(&config);

    for idx in &start_order {
        let proc_def = config
            .process
            .get(*idx)
            .ok_or(Error::ProcessIndexOutOfBounds { index: *idx })?;
        let style = process::color_for_index(*idx);
        let tag = process::make_tag(&proc_def.name, pad_width, style);

        for dep in &proc_def.depends_on {
            if !healthy_set.contains(dep) {
                return Err(Error::DependencyNotHealthy {
                    dependency: dep.clone(),
                    process: proc_def.name.clone(),
                });
            }
        }

        send_log(&log_tx, format!("{tag} starting: {}", proc_def.command))?;
        let spawned = process::spawn(
            &proc_def.name,
            &proc_def.command,
            pad_width,
            style,
            log_tx.clone(),
        )?;
        children.push(spawned);

        if let Some(health_check) = &proc_def.health {
            let interval = Duration::from_secs(health_check.interval_secs);
            let timeout = Duration::from_secs(health_check.timeout_secs);
            send_log(&log_tx, format!("{tag} waiting for health check..."))?;
            health::wait_healthy(&proc_def.name, &health_check.command, interval, timeout).await?;
            send_log(&log_tx, format!("{tag} healthy"))?;
        }

        healthy_set.insert(proc_def.name.clone());
    }

    wait_for_completion(&mut children, &config, mode, pad_width, &log_tx).await
}

fn exit_code_from_status(name: &str, status: std::process::ExitStatus) -> Result<u8, Error> {
    match (status.code(), status.signal()) {
        (Some(code), _) => u8::try_from(code).map_err(|_try_err| Error::InvalidExitCode {
            code,
            name: name.to_string(),
        }),
        (None, Some(sig)) => {
            u8::try_from(128_i32 + sig).map_err(|_try_err| Error::InvalidExitCode {
                code: 128_i32 + sig,
                name: name.to_string(),
            })
        }
        (None, None) => Err(Error::UnknownExitStatus {
            name: name.to_string(),
        }),
    }
}

fn handle_process_exit(
    children: &mut Vec<process::SpawnedProcess>,
    config: &Config,
    name: &str,
    status: std::process::ExitStatus,
    main_index: Option<usize>,
    pad_width: usize,
    log_tx: &mpsc::UnboundedSender<String>,
) -> Result<u8, Error> {
    let exited_index = config
        .process
        .iter()
        .position(|proc| proc.name == name)
        .ok_or_else(|| Error::ProcessNotFound {
            name: name.to_string(),
        })?;

    let style = process::color_for_index(exited_index);
    let tag = process::make_tag(name, pad_width, style);
    let code = exit_code_from_status(name, status)?;

    if Some(exited_index) == main_index {
        send_log(log_tx, format!("{tag} exited with code {code}"))?;
        kill_all(children, pad_width, log_tx)?;
        return Ok(code);
    }

    send_log(log_tx, format!("{tag} crashed with code {code}"))?;
    kill_all(children, pad_width, log_tx)?;
    Ok(1_u8)
}

fn kill_all(
    children: &mut Vec<process::SpawnedProcess>,
    pad_width: usize,
    log_tx: &mpsc::UnboundedSender<String>,
) -> Result<(), Error> {
    let mut first_error: Option<Error> = None;

    for (idx, spawned) in children.iter_mut().enumerate() {
        let style = process::color_for_index(idx);
        let tag = process::make_tag(&spawned.name, pad_width, style);
        send_log(log_tx, format!("{tag} killing"))?;

        if let Err(source) = spawned.child.start_kill()
            && first_error.is_none()
        {
            first_error = Some(Error::Kill {
                name: spawned.name.clone(),
                source,
            });
        }
    }

    children.clear();

    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

fn resolve_start_order(config: &Config) -> Vec<usize> {
    let mut order = Vec::new();
    let mut resolved: HashSet<&str> = HashSet::new();
    let mut remaining: Vec<usize> = (0..config.process.len()).collect();

    while !remaining.is_empty() {
        let mut progress = false;

        remaining.retain(|&idx| {
            let Some(proc_def) = config.process.get(idx) else {
                return false;
            };
            let all_deps_resolved = proc_def
                .depends_on
                .iter()
                .all(|dep| resolved.contains(dep.as_str()));

            if all_deps_resolved {
                order.push(idx);
                resolved.insert(&proc_def.name);
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

fn send_log(log_tx: &mpsc::UnboundedSender<String>, msg: String) -> Result<(), Error> {
    log_tx
        .send(msg)
        .map_err(|_send_err| Error::LogChannelClosed)
}

fn spawn_signal_listener(flag: Arc<AtomicBool>) -> Result<(), Error> {
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
        .map_err(|source| Error::SignalHandler { source })?;

    let flag_term = Arc::<AtomicBool>::clone(&flag);

    tokio::spawn(async move {
        sigterm.recv().await;
        flag_term.store(true, Ordering::SeqCst);
    });

    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => flag.store(true, Ordering::SeqCst),
            Err(_err) => {}
        }
    });

    Ok(())
}

async fn wait_for_completion(
    children: &mut Vec<process::SpawnedProcess>,
    config: &Config,
    mode: Command,
    pad_width: usize,
    log_tx: &mpsc::UnboundedSender<String>,
) -> Result<u8, Error> {
    let main_index = match mode {
        Command::Run => Some(config.process.len() - 1_usize),
        Command::Up => None,
    };

    let signal_received = Arc::new(AtomicBool::new(false));

    spawn_signal_listener(Arc::<AtomicBool>::clone(&signal_received))?;

    let event = wait_for_event(children, &signal_received).await?;

    match event {
        Event::ProcessExited { name, status } => handle_process_exit(
            children, config, &name, status, main_index, pad_width, log_tx,
        ),
        Event::Signal => {
            kill_all(children, pad_width, log_tx)?;
            Ok(130_u8)
        }
    }
}

async fn wait_for_event(
    children: &mut Vec<process::SpawnedProcess>,
    signal_flag: &AtomicBool,
) -> Result<Event, Error> {
    loop {
        for spawned in children.iter_mut() {
            match spawned.child.try_wait() {
                Ok(Some(status)) => {
                    let name = spawned.name.clone();
                    children.retain(|ch| ch.name != name);
                    return Ok(Event::ProcessExited { name, status });
                }
                Ok(None) => {}
                Err(source) => {
                    return Err(Error::ProcessWait {
                        name: spawned.name.clone(),
                        source,
                    });
                }
            }
        }

        if signal_flag.load(Ordering::SeqCst) {
            return Ok(Event::Signal);
        }

        tokio::time::sleep(std::time::Duration::from_millis(50_u64)).await;
    }
}
