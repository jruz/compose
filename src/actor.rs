use std::os::unix::process::ExitStatusExt as _;
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio::sync::{mpsc, oneshot};

use crate::docker;
use crate::exec_shutdown;
use crate::message;

const STDERR_BUFFER_LINES: usize = 20;

pub enum Driver {
    Docker {
        binary: String,
        container: String,
        stop_timeout_secs: Option<u64>,
    },
    Exec {
        group: nix::unistd::Pid,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Docker(#[from] docker::Error),

    #[error(transparent)]
    ExecShutdown(#[from] exec_shutdown::Error),

    #[error(transparent)]
    Wait(#[from] std::io::Error),
}

pub struct ActorChannels {
    pub cmd_rx: mpsc::UnboundedReceiver<message::Command>,
    pub event_tx: mpsc::UnboundedSender<message::ActorExited>,
}

pub struct OutputDrain {
    stderr_done: oneshot::Receiver<()>,
    stderr_rx: mpsc::UnboundedReceiver<String>,
    stdout_done: oneshot::Receiver<()>,
}

pub fn setup_output_drain(
    name: &str,
    child: &mut tokio::process::Child,
    display_tx: mpsc::UnboundedSender<message::DisplayMsg>,
) -> OutputDrain {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (stderr_cap_tx, stderr_cap_rx) = mpsc::unbounded_channel::<String>();

    let (stdout_done_tx, stdout_done_rx) = oneshot::channel::<()>();
    let (stderr_done_tx, stderr_done_rx) = oneshot::channel::<()>();

    if let Some(stdout) = stdout {
        let tx = display_tx.clone();
        let proc_name = name.to_string();
        tokio::spawn(async move {
            let buffered_stdout = BufReader::new(stdout);
            stream_lines(&proc_name, buffered_stdout, &tx).await;
            match stdout_done_tx.send(()) {
                Ok(()) | Err(()) => {}
            }
        });
    } else {
        match stdout_done_tx.send(()) {
            Ok(()) | Err(()) => {}
        }
    }

    if let Some(stderr) = stderr {
        let proc_name = name.to_string();
        tokio::spawn(async move {
            let buffered_stderr = BufReader::new(stderr);
            let mut lines = buffered_stderr.lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        let line_copy = line.clone();
                        match stderr_cap_tx.send(line_copy) {
                            Ok(()) | Err(_) => {}
                        }
                        let output_msg = message::DisplayMsg::Output(message::OutputMsg::Line {
                            line,
                            name: proc_name.clone(),
                        });
                        if let Err(_send_err) = display_tx.send(output_msg) {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        let error_msg = message::DisplayMsg::Output(message::OutputMsg::Line {
                            line: format!("[pipe error: {err}]"),
                            name: proc_name.clone(),
                        });
                        if let Err(_send_err) = display_tx.send(error_msg) {}
                        break;
                    }
                }
            }
            match stderr_done_tx.send(()) {
                Ok(()) | Err(()) => {}
            }
        });
    } else {
        match stderr_done_tx.send(()) {
            Ok(()) | Err(()) => {}
        }
    }

    OutputDrain {
        stderr_done: stderr_done_rx,
        stderr_rx: stderr_cap_rx,
        stdout_done: stdout_done_rx,
    }
}

pub async fn actor_loop(
    name: String,
    child: tokio::process::Child,
    channels: ActorChannels,
    drain: OutputDrain,
    driver: Driver,
) -> Result<(), Error> {
    let event_tx = channels.event_tx.clone();
    let result = run_actor(&name, child, channels, drain, driver).await;
    if let Err(error) = &result {
        let send_result = event_tx.send(message::ActorExited {
            code: 1,
            name,
            stderr_lines: vec![error.to_string()],
        });
        drop(send_result);
    }
    result
}

async fn run_actor(
    name: &str,
    mut child: tokio::process::Child,
    channels: ActorChannels,
    drain: OutputDrain,
    driver: Driver,
) -> Result<(), Error> {
    let ActorChannels {
        mut cmd_rx,
        event_tx,
    } = channels;

    let OutputDrain {
        mut stderr_rx,
        stdout_done,
        stderr_done,
    } = drain;

    let status = tokio::select! {
        Some(message::Command::Kill) = cmd_rx.recv() => {
            kill_process(&driver, &mut child).await?;
            return Ok(());
        }
        result = child.wait() => result?,
    };

    if let Driver::Exec { group } = driver {
        exec_shutdown::terminate(group, &mut child).await?;
    }

    if let Err(_recv_err) = stdout_done.await {}
    if let Err(_recv_err) = stderr_done.await {}

    let code = exit_code_from_status(status);
    let stderr_lines = drain_stderr(&mut stderr_rx).await;

    let send_result = event_tx.send(message::ActorExited {
        code,
        name: name.to_string(),
        stderr_lines,
    });
    drop(send_result);
    Ok(())
}

async fn kill_process(driver: &Driver, child: &mut tokio::process::Child) -> Result<(), Error> {
    match driver {
        Driver::Exec { group } => exec_shutdown::terminate(*group, child).await?,
        Driver::Docker {
            binary,
            container,
            stop_timeout_secs,
        } => {
            stop_container(binary, container, *stop_timeout_secs, child).await?;
        }
    }
    Ok(())
}

async fn stop_container(
    binary: &str,
    container: &str,
    timeout_secs: Option<u64>,
    child: &mut tokio::process::Child,
) -> Result<(), Error> {
    let interval = std::time::Duration::from_millis(10);
    while child.try_wait()?.is_none() {
        if !docker::is_running(binary, container).await? {
            tokio::time::sleep(interval).await;
            continue;
        }
        match docker::stop(binary, container, timeout_secs).await {
            Ok(()) | Err(docker::Error::StopNotFound { .. }) => {}
            Err(error) => {
                let error = error.into();
                return Err(error);
            }
        }
        child.wait().await?;
        break;
    }
    Ok(())
}

fn exit_code_from_status(status: std::process::ExitStatus) -> u8 {
    match (status.code(), status.signal()) {
        (Some(code), _) => {
            let Ok(byte) = u8::try_from(code) else {
                return 1;
            };
            byte
        }
        (None, Some(sig)) => {
            let code = 128_i32.saturating_add(sig);
            let Ok(byte) = u8::try_from(code) else {
                return 1;
            };
            byte
        }
        (None, None) => 1,
    }
}

async fn drain_stderr(rx: &mut mpsc::UnboundedReceiver<String>) -> Vec<String> {
    let mut lines = Vec::new();
    while let Some(line) = rx.recv().await {
        lines.push(line);
    }
    let skip = lines.len().saturating_sub(STDERR_BUFFER_LINES);
    lines.into_iter().skip(skip).collect()
}

pub async fn stream_lines<R>(
    name: &str,
    reader: BufReader<R>,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = reader.lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                let output_msg = message::DisplayMsg::Output(message::OutputMsg::Line {
                    line,
                    name: name.to_string(),
                });
                if let Err(_send_err) = display_tx.send(output_msg) {
                    break;
                }
            }
            Ok(None) => break,
            Err(err) => {
                let error_msg = message::DisplayMsg::Output(message::OutputMsg::Line {
                    line: format!("[pipe error: {err}]"),
                    name: name.to_string(),
                });
                if let Err(_send_err) = display_tx.send(error_msg) {}
                break;
            }
        }
    }
}
