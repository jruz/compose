use tokio::sync::mpsc;

use crate::message;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("display channel closed: {0}")]
    DisplayChannelClosed(mpsc::error::SendError<message::DisplayMsg>),
}

pub fn kill_all(actors: &[message::ActorHandle]) {
    for actor in actors {
        message::kill_actor(actor);
    }
}

pub fn emit_killing_all(
    actors: &[message::ActorHandle],
    exclude: Option<&str>,
    reason: &str,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<(), Error> {
    for actor in actors {
        if exclude.is_none_or(|name| actor.name != name) {
            send_display(
                display_tx,
                message::DisplayMsg::Lifecycle(message::LifecycleMsg::Killing {
                    name: actor.name.clone(),
                    reason: reason.to_string(),
                }),
            )?;
        }
    }
    Ok(())
}

pub fn send_display(
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    msg: message::DisplayMsg,
) -> Result<(), Error> {
    display_tx.send(msg).map_err(Error::DisplayChannelClosed)
}
