use tokio::signal;
use tokio::sync::mpsc;

use crate::message;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("display channel closed: {0}")]
    DisplayChannelClosed(mpsc::error::SendError<message::DisplayMsg>),

    #[error("{name}: {reason}")]
    HealthFailed { name: String, reason: String },

    #[error("failed to register signal handler: {source}")]
    SignalHandler { source: std::io::Error },
}

pub enum ShutdownStrategy {
    AnyExit,
    MainProcess(String),
}

pub async fn manage(
    cmd_senders: Vec<(String, mpsc::UnboundedSender<message::Command>)>,
    all_names: &[String],
    shutdown: ShutdownStrategy,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    mut event_rx: mpsc::UnboundedReceiver<message::Event>,
) -> Result<message::ExitResult, Error> {
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
        .map_err(|source| Error::SignalHandler { source })?;

    tokio::select! {
        Some(event) = event_rx.recv() => {
            handle_event(event, &shutdown, all_names, &cmd_senders, display_tx)
        }
        Some(()) = sigterm.recv() => {
            emit_killing_all(all_names, "signal received", display_tx)?;
            kill_all(&cmd_senders);
            Ok(message::ExitResult { code: 130, failed_name: None })
        }
        result = signal::ctrl_c() => {
            result.map_err(|source| Error::SignalHandler { source })?;
            emit_killing_all(all_names, "signal received", display_tx)?;
            kill_all(&cmd_senders);
            Ok(message::ExitResult { code: 130, failed_name: None })
        }
    }
}

fn handle_event(
    event: message::Event,
    shutdown: &ShutdownStrategy,
    all_names: &[String],
    cmd_senders: &[(String, mpsc::UnboundedSender<message::Command>)],
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<message::ExitResult, Error> {
    match event {
        message::Event::Exited {
            name,
            code,
            stderr_lines,
        } => {
            let remaining: Vec<String> = all_names
                .iter()
                .filter(|nm| nm.as_str() != name)
                .cloned()
                .collect();

            match shutdown {
                ShutdownStrategy::MainProcess(main_name) if *main_name == name => {
                    send_display(
                        display_tx,
                        message::DisplayMsg::Exited {
                            name: name.clone(),
                            code,
                        },
                    )?;
                    emit_killing_all(&remaining, &format!("{name} exited"), display_tx)?;
                    kill_all(cmd_senders);
                    Ok(message::ExitResult {
                        code,
                        failed_name: None,
                    })
                }
                ShutdownStrategy::MainProcess(_) | ShutdownStrategy::AnyExit => {
                    let reason = format!("{name} crashed");
                    send_display(
                        display_tx,
                        message::DisplayMsg::Crashed {
                            name: name.clone(),
                            code,
                        },
                    )?;
                    emit_killing_all(&remaining, &reason, display_tx)?;
                    if !stderr_lines.is_empty() {
                        send_display(
                            display_tx,
                            message::DisplayMsg::Summary {
                                name: name.clone(),
                                stderr_lines,
                            },
                        )?;
                    }
                    kill_all(cmd_senders);
                    Ok(message::ExitResult {
                        code: 1,
                        failed_name: Some(name),
                    })
                }
            }
        }
        message::Event::Healthy { .. } => Ok(message::ExitResult {
            code: 0,
            failed_name: None,
        }),
        message::Event::HealthFailed { name, reason } => {
            send_display(
                display_tx,
                message::DisplayMsg::HealthFailed {
                    name: name.clone(),
                    reason: reason.clone(),
                },
            )?;
            emit_killing_all(all_names, &format!("{name} health failed"), display_tx)?;
            kill_all(cmd_senders);
            Err(Error::HealthFailed { name, reason })
        }
    }
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

fn send_display(
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    msg: message::DisplayMsg,
) -> Result<(), Error> {
    display_tx.send(msg).map_err(Error::DisplayChannelClosed)
}
