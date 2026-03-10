use std::collections::BTreeMap;

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
    let mut cmd = tokio::process::Command::new(shell);
    cmd.arg("-c")
        .arg(command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    for (key, value) in env {
        cmd.env(key, value);
    }

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
        |proc_child| Box::pin(actor::graceful_kill(proc_child)),
        |health_cmd| {
            Box::pin(async move {
                run_exec_health(health_cmd)
                    .await
                    .map_err(|err| err.to_string())
            })
        },
    ));

    Ok(cmd_tx)
}

async fn run_exec_health(cmd: &actor::HealthCommand) -> std::io::Result<std::process::Output> {
    match cmd {
        actor::HealthCommand::Shell(shell_cmd) => {
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(shell_cmd)
                .output()
                .await
        }
        actor::HealthCommand::Exec(args) => {
            let (program, rest) = args.split_first().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "exec args empty")
            })?;
            tokio::process::Command::new(program)
                .args(rest)
                .output()
                .await
        }
    }
}

pub fn spawn_child(
    cmd: &mut tokio::process::Command,
) -> Result<tokio::process::Child, std::io::Error> {
    cmd.spawn()
}
