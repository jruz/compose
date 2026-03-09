use tokio::sync::mpsc;

use crate::message;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("display channel closed: {0}")]
    DisplayChannelClosed(mpsc::error::SendError<message::DisplayMsg>),
}

pub fn kill_all(cmd_senders: &[(String, mpsc::UnboundedSender<message::Command>)]) {
    for (_name, tx) in cmd_senders {
        if let Err(_send_err) = tx.send(message::Command::Kill) {}
    }
}

pub fn emit_killing_all(
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

pub fn send_display(
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    msg: message::DisplayMsg,
) -> Result<(), Error> {
    display_tx.send(msg).map_err(Error::DisplayChannelClosed)
}
