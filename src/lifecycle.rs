use tokio::signal;
use tokio::sync::mpsc;

use crate::message;
use crate::shutdown;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{name}: {reason}")]
    HealthFailed { name: String, reason: String },

    #[error(transparent)]
    Shutdown(#[from] shutdown::Error),

    #[error("failed to register signal handler: {source}")]
    SignalHandler { source: std::io::Error },
}

pub enum ShutdownStrategy {
    AnyExit,
    MainProcess(String),
}

pub async fn manage(
    actors: &[message::ActorHandle],
    shutdown: ShutdownStrategy,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    mut event_rx: mpsc::UnboundedReceiver<message::ActorEvent>,
) -> Result<message::ExitResult, Error> {
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
        .map_err(|source| Error::SignalHandler { source })?;

    tokio::select! {
        Some(event) = event_rx.recv() => {
            handle_event(event, &shutdown, actors, display_tx)
        }
        Some(()) = sigterm.recv() => {
            shutdown::emit_killing_all(actors, None, "signal received", display_tx)?;
            shutdown::kill_all(actors);
            Ok(message::ExitResult { code: 130, failed_name: None })
        }
        result = signal::ctrl_c() => {
            result.map_err(|source| Error::SignalHandler { source })?;
            shutdown::emit_killing_all(actors, None, "signal received", display_tx)?;
            shutdown::kill_all(actors);
            Ok(message::ExitResult { code: 130, failed_name: None })
        }
    }
}

fn handle_event(
    event: message::ActorEvent,
    shutdown: &ShutdownStrategy,
    actors: &[message::ActorHandle],
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<message::ExitResult, Error> {
    match event {
        message::ActorEvent::Exited {
            name,
            code,
            stderr_lines,
        } => match shutdown {
            ShutdownStrategy::MainProcess(main_name) if *main_name == name => {
                send_lifecycle(
                    display_tx,
                    message::LifecycleMsg::Exited {
                        name: name.clone(),
                        code,
                    },
                )?;
                shutdown::emit_killing_all(
                    actors,
                    Some(&name),
                    &format!("{name} exited"),
                    display_tx,
                )?;
                shutdown::kill_all(actors);
                let failed_name = if code == 0 { None } else { Some(name) };
                Ok(message::ExitResult { code, failed_name })
            }
            ShutdownStrategy::MainProcess(_) | ShutdownStrategy::AnyExit => {
                send_crashed(display_tx, &name, code, &stderr_lines)?;
                shutdown::emit_killing_all(
                    actors,
                    Some(&name),
                    &format!("{name} crashed"),
                    display_tx,
                )?;
                shutdown::kill_all(actors);
                Ok(message::ExitResult {
                    code: 1,
                    failed_name: Some(name),
                })
            }
        },
        message::ActorEvent::HealthFailed { name, reason } => {
            send_lifecycle(
                display_tx,
                message::LifecycleMsg::HealthFailed {
                    name: name.clone(),
                    reason: reason.clone(),
                },
            )?;
            shutdown::emit_killing_all(actors, None, &format!("{name} health failed"), display_tx)?;
            shutdown::kill_all(actors);
            Err(Error::HealthFailed { name, reason })
        }
        message::ActorEvent::Healthy { .. } => Ok(message::ExitResult {
            code: 0,
            failed_name: None,
        }),
    }
}

fn send_lifecycle(
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    msg: message::LifecycleMsg,
) -> Result<(), shutdown::Error> {
    shutdown::send_display(display_tx, message::DisplayMsg::Lifecycle(msg))
}

fn send_crashed(
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    name: &str,
    code: u8,
    stderr_lines: &[String],
) -> Result<(), shutdown::Error> {
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
