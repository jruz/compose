use std::collections::BTreeMap;
use std::os::unix::process::CommandExt as _;

use tokio::sync::mpsc;

use crate::actor;
use crate::config;
use crate::message;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to spawn process {name}: {source}")]
    Spawn {
        name: String,
        source: std::io::Error,
    },
}

pub fn spawn_actor(
    name: &str,
    command: &str,
    shell: &str,
    env: &BTreeMap<String, String>,
    health: Option<&config::HealthCheck>,
    event_tx: mpsc::UnboundedSender<message::Event>,
    display_tx: mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<mpsc::UnboundedSender<message::Command>, Error> {
    let mut std_cmd = std::process::Command::new(shell);
    std_cmd
        .arg("-c")
        .arg(command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .process_group(0);

    for (key, value) in env {
        std_cmd.env(key, value);
    }

    let mut cmd = tokio::process::Command::from(std_cmd);
    cmd.kill_on_drop(true);

    let mut spawned = spawn_child(&mut cmd).map_err(|source| Error::Spawn {
        name: name.to_string(),
        source,
    })?;

    let drain = actor::setup_output_drain(name, &mut spawned, display_tx);
    let health_config = health.map(actor::parse_health_config);
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

    let actor_name = name.to_string();
    let channels = actor::ActorChannels { cmd_rx, event_tx };
    tokio::spawn(actor::actor_loop(
        actor_name,
        spawned,
        channels,
        health_config,
        drain,
        actor::Driver::Exec,
    ));

    Ok(cmd_tx)
}

pub fn spawn_child(
    cmd: &mut tokio::process::Command,
) -> Result<tokio::process::Child, std::io::Error> {
    cmd.spawn()
}
