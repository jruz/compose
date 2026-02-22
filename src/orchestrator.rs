use std::collections::HashSet;
use std::os::unix::process::ExitStatusExt as _;
use std::time::Duration;

use tokio::signal;
use tokio::sync::mpsc;

use crate::cli::Command;
use crate::config::Config;
use crate::formatter;
use crate::health;
use crate::process;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("dependency {dependency} not healthy when starting {process}")]
    DependencyNotHealthy { dependency: String, process: String },

    #[error(transparent)]
    Formatter(#[from] formatter::Error),

    #[error(transparent)]
    Health(#[from] health::Error),

    #[error("exit code {code} out of u8 range for process {name}: {source}")]
    InvalidExitCode {
        code: i32,
        name: String,
        source: std::num::TryFromIntError,
    },

    #[error("log channel closed: {0}")]
    LogChannelClosed(mpsc::error::SendError<String>),

    #[error("no processes defined")]
    NoProcesses,

    #[error(transparent)]
    Process(#[from] process::Error),

    #[error("process {name} not found")]
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

struct ExitEvent {
    name: String,
    result: Result<std::process::ExitStatus, std::io::Error>,
}

enum ShutdownStrategy {
    AnyExit,
    MainProcess(String),
}

pub async fn run(
    config: Config,
    mode: Command,
    log_tx: mpsc::UnboundedSender<String>,
) -> Result<u8, Error> {
    let names: Vec<String> = config
        .process
        .iter()
        .map(|proc| proc.name.clone())
        .collect();
    let fmt_ctx = formatter::build_context(&names).ok_or(Error::NoProcesses)?;

    let children = start_all(&config, &fmt_ctx, &log_tx).await?;

    let shutdown = match mode {
        Command::Run => ShutdownStrategy::MainProcess(
            config
                .process
                .last()
                .ok_or(Error::NoProcesses)?
                .name
                .clone(),
        ),
        Command::Up => ShutdownStrategy::AnyExit,
    };

    manage_lifecycle(children, &names, &fmt_ctx, shutdown, &log_tx).await
}

async fn start_all(
    config: &Config,
    fmt_ctx: &formatter::Context,
    log_tx: &mpsc::UnboundedSender<String>,
) -> Result<Vec<(String, tokio::process::Child)>, Error> {
    let mut children: Vec<(String, tokio::process::Child)> = Vec::new();
    let mut healthy_set: HashSet<String> = HashSet::new();
    let start_order = resolve_start_order(config);

    for name in &start_order {
        let proc_def = config
            .process
            .iter()
            .find(|proc| &proc.name == name)
            .ok_or_else(|| Error::ProcessNotFound { name: name.clone() })?;

        for dep in &proc_def.depends_on {
            if !healthy_set.contains(dep) {
                return Err(Error::DependencyNotHealthy {
                    dependency: dep.clone(),
                    process: name.clone(),
                });
            }
        }

        emit(
            log_tx,
            fmt_ctx,
            &formatter::Event::Starting {
                name,
                command: &proc_def.command,
            },
        )?;

        let style = formatter::lookup_style(fmt_ctx, name)?;
        let pad_width = formatter::context_pad_width(fmt_ctx);
        let child = process::spawn(name, &proc_def.command, pad_width, style, log_tx.clone())?;
        children.push((name.clone(), child));

        if let Some(health_check) = &proc_def.health {
            let interval = Duration::from_secs(health_check.interval_secs);
            let timeout = Duration::from_secs(health_check.timeout_secs);
            emit(log_tx, fmt_ctx, &formatter::Event::WaitingHealth { name })?;
            health::wait_healthy(name, &health_check.command, interval, timeout).await?;
            emit(log_tx, fmt_ctx, &formatter::Event::Healthy { name })?;
        }

        healthy_set.insert(name.clone());
    }

    Ok(children)
}

async fn manage_lifecycle(
    children: Vec<(String, tokio::process::Child)>,
    all_names: &[String],
    fmt_ctx: &formatter::Context,
    shutdown: ShutdownStrategy,
    log_tx: &mpsc::UnboundedSender<String>,
) -> Result<u8, Error> {
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
        .map_err(|source| Error::SignalHandler { source })?;

    let (exit_tx, mut exit_rx) = mpsc::unbounded_channel::<ExitEvent>();

    for (name, mut child) in children {
        let tx = exit_tx.clone();
        tokio::spawn(async move {
            let result = child.wait().await;
            let _send = tx.send(ExitEvent { name, result });
        });
    }

    drop(exit_tx);

    tokio::select! {
        Some(event) = exit_rx.recv() => {
            let status = event.result.map_err(|source| Error::ProcessWait {
                name: event.name.clone(),
                source,
            })?;
            let code = exit_code_from_status(&event.name, status)?;
            let remaining: Vec<String> = all_names
                .iter()
                .filter(|nm| nm.as_str() != event.name)
                .cloned()
                .collect();
            handle_exit_result(&event.name, code, &shutdown, &remaining, fmt_ctx, log_tx)
        }
        _ = sigterm.recv() => {
            emit_killing_all(all_names, fmt_ctx, log_tx)?;
            Ok(130_u8)
        }
        result = signal::ctrl_c() => {
            result.map_err(|source| Error::SignalHandler { source })?;
            emit_killing_all(all_names, fmt_ctx, log_tx)?;
            Ok(130_u8)
        }
    }
}

fn handle_exit_result(
    name: &str,
    code: u8,
    shutdown: &ShutdownStrategy,
    remaining: &[String],
    fmt_ctx: &formatter::Context,
    log_tx: &mpsc::UnboundedSender<String>,
) -> Result<u8, Error> {
    match shutdown {
        ShutdownStrategy::MainProcess(main_name) if main_name == name => {
            emit(log_tx, fmt_ctx, &formatter::Event::Exited { name, code })?;
            emit_killing_all(remaining, fmt_ctx, log_tx)?;
            Ok(code)
        }
        ShutdownStrategy::MainProcess(_) | ShutdownStrategy::AnyExit => {
            emit(log_tx, fmt_ctx, &formatter::Event::Crashed { name, code })?;
            emit_killing_all(remaining, fmt_ctx, log_tx)?;
            Ok(1_u8)
        }
    }
}

fn exit_code_from_status(name: &str, status: std::process::ExitStatus) -> Result<u8, Error> {
    match (status.code(), status.signal()) {
        (Some(code), _) => u8::try_from(code).map_err(|source| Error::InvalidExitCode {
            code,
            name: name.to_string(),
            source,
        }),
        (None, Some(sig)) => u8::try_from(128_i32 + sig).map_err(|source| Error::InvalidExitCode {
            code: 128_i32 + sig,
            name: name.to_string(),
            source,
        }),
        (None, None) => Err(Error::UnknownExitStatus {
            name: name.to_string(),
        }),
    }
}

fn emit_killing_all(
    names: &[String],
    fmt_ctx: &formatter::Context,
    log_tx: &mpsc::UnboundedSender<String>,
) -> Result<(), Error> {
    for name in names {
        emit(log_tx, fmt_ctx, &formatter::Event::Killing { name })?;
    }
    Ok(())
}

fn resolve_start_order(config: &Config) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    let mut resolved: HashSet<&str> = HashSet::new();
    let mut remaining: Vec<&str> = config
        .process
        .iter()
        .map(|proc| proc.name.as_str())
        .collect();

    while !remaining.is_empty() {
        let mut progress = false;

        remaining.retain(|&name| {
            let Some(proc_def) = config.process.iter().find(|proc| proc.name == name) else {
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

fn emit(
    log_tx: &mpsc::UnboundedSender<String>,
    fmt_ctx: &formatter::Context,
    event: &formatter::Event<'_>,
) -> Result<(), Error> {
    let msg = formatter::render(fmt_ctx, event)?;
    log_tx.send(msg).map_err(Error::LogChannelClosed)
}
