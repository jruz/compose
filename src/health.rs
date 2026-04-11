use std::time::Duration;

use crate::config;
use crate::docker;

pub enum HealthCommand {
    Exec(Vec<String>),
    Shell(String),
}

pub struct HealthConfig {
    pub command: HealthCommand,
    pub interval: Duration,
    pub timeout: Duration,
}

pub enum HealthDriver {
    Docker { binary: String, container: String },
    Exec,
}

pub fn parse_config(hc: &config::HealthCheck) -> HealthConfig {
    let command = match (&hc.command, &hc.exec) {
        (Some(shell_cmd), _) => {
            let cloned = shell_cmd.clone();
            HealthCommand::Shell(cloned)
        }
        (_, Some(exec)) => {
            let cloned = exec.clone();
            HealthCommand::Exec(cloned)
        }
        (None, None) => {
            let empty = String::new();
            HealthCommand::Shell(empty)
        }
    };
    HealthConfig {
        command,
        interval: Duration::from_secs(hc.interval_secs),
        timeout: Duration::from_secs(hc.timeout_secs),
    }
}

pub async fn check(
    driver: &HealthDriver,
    cmd: &HealthCommand,
) -> Result<std::process::Output, String> {
    match driver {
        HealthDriver::Exec => run_exec(cmd).await.map_err(|err| err.to_string()),
        HealthDriver::Docker { binary, container } => run_docker(binary, container, cmd)
            .await
            .map_err(|err| err.to_string()),
    }
}

async fn run_docker(
    binary: &str,
    container: &str,
    cmd: &HealthCommand,
) -> Result<std::process::Output, docker::Error> {
    match cmd {
        HealthCommand::Shell(shell_cmd) => docker::exec_shell(binary, container, shell_cmd).await,
        HealthCommand::Exec(args) => docker::exec_check(binary, container, args).await,
    }
}

async fn run_exec(cmd: &HealthCommand) -> std::io::Result<std::process::Output> {
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
