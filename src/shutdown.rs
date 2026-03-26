use tokio::sync::mpsc;

use crate::message;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("event channel closed: {0}")]
    EventChannelClosed(mpsc::error::SendError<message::Event>),
}

pub fn kill_all(actors: &[message::ActorHandle]) {
    for actor in actors {
        actor.kill();
    }
}

pub fn emit_killing_all(
    actors: &[message::ActorHandle],
    exclude: Option<&str>,
    reason: &str,
    event_tx: &mpsc::UnboundedSender<message::Event>,
) -> Result<(), Error> {
    for actor in actors {
        if exclude.is_none_or(|name| actor.name != name) {
            send_event(
                event_tx,
                message::Event::Killing {
                    name: actor.name.clone(),
                    reason: reason.to_string(),
                },
            )?;
        }
    }
    Ok(())
}

pub fn send_event(
    event_tx: &mpsc::UnboundedSender<message::Event>,
    event: message::Event,
) -> Result<(), Error> {
    event_tx.send(event).map_err(Error::EventChannelClosed)
}
