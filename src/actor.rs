use std::os::unix::process::ExitStatusExt as _;
use std::time::Duration;

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio::sync::{mpsc, oneshot};

use crate::config;
use crate::docker;
use crate::message;

const GRACEFUL_SHUTDOWN_SECS: u64 = 2;
const STDERR_BUFFER_LINES: usize = 20;

pub enum HealthCommand {
    Exec(Vec<String>),
    Shell(String),
}

pub struct HealthConfig {
    pub command: HealthCommand,
    pub interval: Duration,
    pub timeout: Duration,
}

enum HealthOutcome {
    CommandFailed(String),
    Healthy,
    Killed,
    ProcessExited(Result<std::process::ExitStatus, std::io::Error>),
    TimedOut(String),
}

pub enum Driver {
    Docker {
        binary: String,
        container: String,
        stop_timeout_secs: Option<u64>,
    },
    Exec,
}

pub struct ActorChannels {
    pub cmd_rx: mpsc::UnboundedReceiver<message::Command>,
    pub event_tx: mpsc::UnboundedSender<message::Event>,
}

pub struct OutputDrain {
    stderr_done: oneshot::Receiver<()>,
    stderr_rx: mpsc::UnboundedReceiver<String>,
    stdout_done: oneshot::Receiver<()>,
}

pub fn parse_health_config(hc: &config::HealthCheck) -> HealthConfig {
    let command = match (&hc.command, &hc.exec) {
        (Some(shell_cmd), _) => HealthCommand::Shell(shell_cmd.clone()),
        (_, Some(exec)) => HealthCommand::Exec(exec.clone()),
        (None, None) => HealthCommand::Shell(String::new()),
    };
    HealthConfig {
        command,
        interval: Duration::from_secs(hc.interval_secs),
        timeout: Duration::from_secs(hc.timeout_secs),
    }
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
            stream_lines(&proc_name, BufReader::new(stdout), &tx).await;
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
            let mut lines = BufReader::new(stderr).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        match stderr_cap_tx.send(line.clone()) {
                            Ok(()) | Err(_) => {}
                        }
                        if let Err(_send_err) =
                            display_tx.send(message::DisplayMsg::Output(message::OutputMsg::Line {
                                name: proc_name.clone(),
                                line,
                            }))
                        {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        if let Err(_send_err) =
                            display_tx.send(message::DisplayMsg::Output(message::OutputMsg::Line {
                                name: proc_name.clone(),
                                line: format!("[pipe error: {err}]"),
                            }))
                        {}
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
        stderr_rx: stderr_cap_rx,
        stderr_done: stderr_done_rx,
        stdout_done: stdout_done_rx,
    }
}

pub async fn actor_loop(
    name: String,
    mut child: tokio::process::Child,
    channels: ActorChannels,
    health: Option<HealthConfig>,
    drain: OutputDrain,
    driver: Driver,
) {
    let ActorChannels {
        mut cmd_rx,
        event_tx,
    } = channels;

    let OutputDrain {
        mut stderr_rx,
        stdout_done,
        stderr_done,
    } = drain;

    if let Some(hc) = health {
        match run_health_check(&hc, &mut child, &mut cmd_rx, &driver).await {
            HealthOutcome::Healthy => {}
            HealthOutcome::TimedOut(last_stderr) => {
                kill_process(&driver, &mut child).await;
                if let Err(_send_err) = event_tx.send(message::Event::HealthFailed {
                    name: name.clone(),
                    reason: format!("health check timed out, last stderr: {last_stderr}"),
                }) {}
                return;
            }
            HealthOutcome::ProcessExited(_status) => {
                if let Err(_recv_err) = stdout_done.await {}
                if let Err(_recv_err) = stderr_done.await {}
                let stderr = drain_stderr(&mut stderr_rx).await;
                let reason = if stderr.is_empty() {
                    "exited before becoming healthy".to_string()
                } else {
                    format!(
                        "exited before becoming healthy, last stderr: {}",
                        stderr.join("; ")
                    )
                };
                if let Err(_send_err) = event_tx.send(message::Event::HealthFailed {
                    name: name.clone(),
                    reason,
                }) {}
                return;
            }
            HealthOutcome::Killed => {
                kill_process(&driver, &mut child).await;
                return;
            }
            HealthOutcome::CommandFailed(err) => {
                if let Err(_send_err) = event_tx.send(message::Event::HealthFailed {
                    name: name.clone(),
                    reason: format!("health check command failed: {err}"),
                }) {}
                return;
            }
        }
    }

    if let Err(_send_err) = event_tx.send(message::Event::Healthy { name: name.clone() }) {
        return;
    }

    let status = tokio::select! {
        Some(message::Command::Kill) = cmd_rx.recv() => {
            kill_process(&driver, &mut child).await;
            return;
        }
        result = child.wait() => {
            match result {
                Ok(status) => status,
                Err(_) => return,
            }
        }
    };

    if let Err(_recv_err) = stdout_done.await {}
    if let Err(_recv_err) = stderr_done.await {}

    let code = exit_code_from_status(status);
    let stderr_lines = drain_stderr(&mut stderr_rx).await;

    if let Err(_send_err) = event_tx.send(message::Event::Exited {
        name: name.clone(),
        code,
        stderr_lines,
    }) {}
}

async fn run_health_check(
    hc: &HealthConfig,
    child: &mut tokio::process::Child,
    cmd_rx: &mut mpsc::UnboundedReceiver<message::Command>,
    driver: &Driver,
) -> HealthOutcome {
    let deadline = tokio::time::Instant::now() + hc.timeout;

    loop {
        let check_result = tokio::select! {
            result = check_health(driver, &hc.command) => result,
            status = child.wait() => {
                return HealthOutcome::ProcessExited(status);
            }
            Some(message::Command::Kill) = cmd_rx.recv() => {
                return HealthOutcome::Killed;
            }
        };

        match check_result {
            Ok(output) if output.status.success() => return HealthOutcome::Healthy,
            Ok(output) => {
                if tokio::time::Instant::now() >= deadline {
                    let last_stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    return HealthOutcome::TimedOut(last_stderr);
                }
            }
            Err(err) => {
                return HealthOutcome::CommandFailed(err);
            }
        }

        tokio::select! {
            () = tokio::time::sleep(hc.interval) => {}
            status = child.wait() => {
                return HealthOutcome::ProcessExited(status);
            }
            Some(message::Command::Kill) = cmd_rx.recv() => {
                return HealthOutcome::Killed;
            }
        }
    }
}

async fn kill_process(driver: &Driver, child: &mut tokio::process::Child) {
    match driver {
        Driver::Exec => graceful_kill(child).await,
        Driver::Docker {
            binary,
            container,
            stop_timeout_secs,
        } => if let Err(_stop_err) = docker::stop(binary, container, *stop_timeout_secs).await {},
    }
}

async fn check_health(
    driver: &Driver,
    cmd: &HealthCommand,
) -> Result<std::process::Output, String> {
    match driver {
        Driver::Exec => run_exec_health(cmd).await.map_err(|err| err.to_string()),
        Driver::Docker {
            binary, container, ..
        } => run_docker_health(binary, container, cmd)
            .await
            .map_err(|err| err.to_string()),
    }
}

async fn run_exec_health(cmd: &HealthCommand) -> std::io::Result<std::process::Output> {
    match cmd {
        HealthCommand::Shell(shell_cmd) => {
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(shell_cmd)
                .output()
                .await
        }
        HealthCommand::Exec(args) => {
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

async fn run_docker_health(
    binary: &str,
    container: &str,
    cmd: &HealthCommand,
) -> Result<std::process::Output, docker::Error> {
    match cmd {
        HealthCommand::Shell(shell_cmd) => docker::exec_shell(binary, container, shell_cmd).await,
        HealthCommand::Exec(args) => docker::exec_check(binary, container, args).await,
    }
}

pub async fn graceful_kill(child: &mut tokio::process::Child) {
    let Some(pid) = child.id() else {
        return;
    };
    let Ok(pid) = i32::try_from(pid) else {
        return;
    };
    let group = Pid::from_raw(pid);
    if let Err(_signal_err) = signal::killpg(group, Signal::SIGTERM) {}
    tokio::select! {
        _status = child.wait() => {}
        () = tokio::time::sleep(Duration::from_secs(GRACEFUL_SHUTDOWN_SECS)) => {
            if let Err(_signal_err) = signal::killpg(group, Signal::SIGKILL) {}
            let _wait = child.wait().await;
        }
    }
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

pub async fn stream_lines<R: tokio::io::AsyncRead + Unpin>(
    name: &str,
    reader: BufReader<R>,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) {
    let mut lines = reader.lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                if let Err(_send_err) =
                    display_tx.send(message::DisplayMsg::Output(message::OutputMsg::Line {
                        name: name.to_string(),
                        line,
                    }))
                {
                    break;
                }
            }
            Ok(None) => break,
            Err(err) => {
                if let Err(_send_err) =
                    display_tx.send(message::DisplayMsg::Output(message::OutputMsg::Line {
                        name: name.to_string(),
                        line: format!("[pipe error: {err}]"),
                    }))
                {}
                break;
            }
        }
    }
}
