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
            shutdown::emit_killing_all(all_names, "signal received", display_tx)?;
            shutdown::kill_all(&cmd_senders);
            Ok(message::ExitResult { code: 130, failed_name: None })
        }
        result = signal::ctrl_c() => {
            result.map_err(|source| Error::SignalHandler { source })?;
            shutdown::emit_killing_all(all_names, "signal received", display_tx)?;
            shutdown::kill_all(&cmd_senders);
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
                    shutdown::send_display(
                        display_tx,
                        message::DisplayMsg::Exited {
                            name: name.clone(),
                            code,
                        },
                    )?;
                    shutdown::emit_killing_all(&remaining, &format!("{name} exited"), display_tx)?;
                    shutdown::kill_all(cmd_senders);
                    let failed_name = if code == 0 { None } else { Some(name) };
                    Ok(message::ExitResult { code, failed_name })
                }
                ShutdownStrategy::MainProcess(_) | ShutdownStrategy::AnyExit => {
                    let reason = format!("{name} crashed");
                    shutdown::send_display(
                        display_tx,
                        message::DisplayMsg::Crashed {
                            name: name.clone(),
                            code,
                        },
                    )?;
                    shutdown::emit_killing_all(&remaining, &reason, display_tx)?;
                    if !stderr_lines.is_empty() {
                        shutdown::send_display(
                            display_tx,
                            message::DisplayMsg::Summary {
                                name: name.clone(),
                                stderr_lines,
                            },
                        )?;
                    }
                    shutdown::kill_all(cmd_senders);
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
            shutdown::send_display(
                display_tx,
                message::DisplayMsg::HealthFailed {
                    name: name.clone(),
                    reason: reason.clone(),
                },
            )?;
            shutdown::emit_killing_all(all_names, &format!("{name} health failed"), display_tx)?;
            shutdown::kill_all(cmd_senders);
            Err(Error::HealthFailed { name, reason })
        }
    }
}
