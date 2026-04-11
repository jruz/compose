use tokio::signal;
use tokio::sync::mpsc;

use crate::message;
use crate::shutdown;

#[derive(Debug, thiserror::Error)]
pub enum Error {
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
    mut event_rx: mpsc::UnboundedReceiver<message::ActorExited>,
) -> Result<message::ExitResult, Error> {
    let signal_kind = signal::unix::SignalKind::terminate();
    let mut sigterm =
        signal::unix::signal(signal_kind).map_err(|source| Error::SignalHandler { source })?;

    tokio::select! {
        Some(exited) = event_rx.recv() => {
            handle_exited(exited, &shutdown, actors, display_tx)
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

fn handle_exited(
    exited: message::ActorExited,
    shutdown: &ShutdownStrategy,
    actors: &[message::ActorHandle],
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<message::ExitResult, Error> {
    match shutdown {
        ShutdownStrategy::MainProcess(main_name) if *main_name == exited.name => {
            send_lifecycle(
                display_tx,
                message::LifecycleMsg::Exited {
                    code: exited.code,
                    name: exited.name.clone(),
                },
            )?;
            let reason = format!("{} exited", exited.name);
            let name_ref = exited.name.as_str();
            let exclude = Some(name_ref);
            shutdown::emit_killing_all(actors, exclude, &reason, display_tx)?;
            shutdown::kill_all(actors);
            let failed_name = if exited.code == 0 {
                None
            } else {
                Some(exited.name)
            };
            Ok(message::ExitResult {
                code: exited.code,
                failed_name,
            })
        }
        ShutdownStrategy::MainProcess(_) | ShutdownStrategy::AnyExit => {
            send_crashed(display_tx, &exited.name, exited.code, &exited.stderr_lines)?;
            let reason = format!("{} crashed", exited.name);
            let name_ref = exited.name.as_str();
            let exclude = Some(name_ref);
            shutdown::emit_killing_all(actors, exclude, &reason, display_tx)?;
            shutdown::kill_all(actors);
            Ok(message::ExitResult {
                code: 1,
                failed_name: Some(exited.name),
            })
        }
    }
}

fn send_lifecycle(
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    msg: message::LifecycleMsg,
) -> Result<(), shutdown::Error> {
    let display_msg = message::DisplayMsg::Lifecycle(msg);
    shutdown::send_display(display_tx, display_msg)
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
            code,
            name: name.to_string(),
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
