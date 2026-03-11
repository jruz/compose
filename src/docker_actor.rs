use std::collections::BTreeMap;

use tokio::sync::mpsc;

use crate::actor;
use crate::config;
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
    pub command: Option<&'cfg str>,
    pub env: &'cfg BTreeMap<String, String>,
    pub exec: Option<&'cfg [String]>,
    pub health: Option<&'cfg config::HealthCheck>,
    pub image: &'cfg str,
    pub name: &'cfg str,
    pub network: &'cfg str,
    pub ports: &'cfg [String],
    pub stop_timeout_secs: Option<u64>,
    pub volumes: &'cfg [String],
}

pub fn spawn_actor(
    run: &RunConfig<'_>,
    event_tx: mpsc::UnboundedSender<message::Event>,
    display_tx: mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<mpsc::UnboundedSender<message::Command>, Error> {
    let container = docker::container_name(run.network, run.name);
    let docker_run = docker::RunConfig {
        command: run.command,
        container_name: &container,
        env: run.env,
        exec: run.exec,
        image: run.image,
        network: run.network,
        network_alias: run.name,
        ports: run.ports,
        volumes: run.volumes,
    };
    let mut docker_cmd = docker::run_command(&docker_run);
    let mut spawned =
        crate::process::spawn_child(&mut docker_cmd).map_err(|source| Error::Spawn {
            name: run.name.to_string(),
            source,
        })?;

    let drain = actor::setup_output_drain(run.name, &mut spawned, display_tx);
    let health_config = run.health.map(actor::parse_health_config);
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

    let actor_name = run.name.to_string();
    let channels = actor::ActorChannels { cmd_rx, event_tx };
    let driver = actor::Driver::Docker {
        container,
        stop_timeout_secs: run.stop_timeout_secs,
    };
    tokio::spawn(actor::actor_loop(
        actor_name,
        spawned,
        channels,
        health_config,
        drain,
        driver,
    ));

    Ok(cmd_tx)
}
