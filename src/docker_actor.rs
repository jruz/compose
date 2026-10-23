use std::collections::BTreeMap;

use tokio::sync::mpsc;

use crate::actor;
use crate::docker;
use crate::message;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Docker(#[from] docker::Error),

    #[error("failed to spawn docker run for {name}: {source}")]
    Spawn {
        name: String,
        source: std::io::Error,
    },
}

pub struct RunConfig<'cfg> {
    pub binary: &'cfg str,
    pub command: Option<&'cfg str>,
    pub env: &'cfg BTreeMap<String, String>,
    pub exec: Option<&'cfg [String]>,
    pub image: &'cfg str,
    pub init: Option<bool>,
    pub name: &'cfg str,
    pub network: &'cfg str,
    pub ports: &'cfg [String],
    pub stop_timeout_secs: Option<u64>,
    pub volumes: &'cfg [String],
}

pub fn spawn_actor(
    run: &RunConfig<'_>,
    event_tx: mpsc::UnboundedSender<message::ActorExited>,
    display_tx: mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<message::ActorHandle, Error> {
    let container = docker::container_name(run.network, run.name);
    let docker_run = docker::RunConfig {
        command: run.command,
        container_name: &container,
        env: run.env,
        exec: run.exec,
        image: run.image,
        init: run.init,
        network: run.network,
        network_alias: run.name,
        ports: run.ports,
        volumes: run.volumes,
    };
    let std_cmd = docker::run_command(run.binary, &docker_run);
    let mut cmd = tokio::process::Command::from(std_cmd);
    cmd.kill_on_drop(true);
    let mut spawned = crate::process::spawn_child(&mut cmd).map_err(|source| Error::Spawn {
        name: run.name.to_string(),
        source,
    })?;

    let drain = actor::setup_output_drain(run.name, &mut spawned, display_tx);
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

    let actor_name = run.name.to_string();
    let channels = actor::ActorChannels { cmd_rx, event_tx };
    let driver = actor::Driver::Docker {
        binary: run.binary.to_string(),
        container,
        stop_timeout_secs: run.stop_timeout_secs,
    };
    let actor_future = actor::actor_loop(actor_name, spawned, channels, drain, driver);
    let handle = tokio::spawn(actor_future);

    Ok(message::ActorHandle {
        cmd_tx,
        join: handle,
        name: run.name.to_string(),
    })
}
