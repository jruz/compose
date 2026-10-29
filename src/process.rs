use std::collections::BTreeMap;
use std::os::unix::process::CommandExt as _;

use tokio::sync::mpsc;

use crate::actor;
use crate::message;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("spawned process has no PID")]
    MissingPid,

    #[error(transparent)]
    Pid(#[from] std::num::TryFromIntError),

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
    event_tx: mpsc::UnboundedSender<message::ActorExited>,
    display_tx: mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<message::ActorHandle, Error> {
    let mut std_cmd = std::process::Command::new(shell);
    let piped_out = std::process::Stdio::piped();
    let piped_err = std::process::Stdio::piped();
    let stdin = std::process::Stdio::null();
    std_cmd
        .arg("-c")
        .arg(command)
        .stdin(stdin)
        .stdout(piped_out)
        .stderr(piped_err)
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
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

    let actor_name = name.to_string();
    let channels = actor::ActorChannels { cmd_rx, event_tx };
    let pid = spawned.id().ok_or(Error::MissingPid)?;
    let pid = i32::try_from(pid)?;
    let group = nix::unistd::Pid::from_raw(pid);
    let driver = actor::Driver::Exec { group };
    let actor_future = actor::actor_loop(actor_name, spawned, channels, drain, driver);
    let handle = tokio::spawn(actor_future);

    Ok(message::ActorHandle {
        cmd_tx,
        join: handle,
        name: name.to_string(),
    })
}

pub fn spawn_child(
    cmd: &mut tokio::process::Command,
) -> Result<tokio::process::Child, std::io::Error> {
    cmd.spawn()
}
