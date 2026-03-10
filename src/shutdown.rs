use tokio::sync::mpsc;

use crate::message;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("event channel closed: {0}")]
    EventChannelClosed(mpsc::error::SendError<message::Event>),
}

pub fn kill_all(cmd_senders: &[(String, mpsc::UnboundedSender<message::Command>)]) {
    for (_name, tx) in cmd_senders {
        if let Err(_send_err) = tx.send(message::Command::Kill) {}
    }
}

pub fn emit_killing_all(
    names: &[String],
    reason: &str,
    event_tx: &mpsc::UnboundedSender<message::Event>,
) -> Result<(), Error> {
    for name in names {
        send_event(
            event_tx,
            message::Event::Killing {
                name: name.clone(),
                reason: reason.to_string(),
            },
        )?;
    }
    Ok(())
}

pub fn send_event(
    event_tx: &mpsc::UnboundedSender<message::Event>,
    event: message::Event,
) -> Result<(), Error> {
    event_tx.send(event).map_err(Error::EventChannelClosed)
}
