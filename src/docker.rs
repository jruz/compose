use std::collections::BTreeMap;

use tokio::process;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("docker build failed for {context}: {source}")]
    Build {
        context: String,
        source: std::io::Error,
    },

    #[error("docker build failed for {context} with exit code {code}")]
    BuildFailed { context: String, code: i32 },

    #[error("docker build failed for {context} (killed by signal)")]
    BuildKilled { context: String },

    #[error("docker exec failed: {source}")]
    Exec { source: std::io::Error },

    #[error("docker network create failed: {source}")]
    NetworkCreate { source: std::io::Error },

    #[error("docker network create failed with exit code {code}")]
    NetworkCreateFailed { code: i32 },

    #[error("docker network create failed (killed by signal)")]
    NetworkCreateKilled,

    #[error("docker network rm failed: {source}")]
    NetworkRm { source: std::io::Error },

    #[error("docker rm failed: {source}")]
    Rm { source: std::io::Error },

    #[error("docker stop failed for {name}: {source}")]
    Stop {
        name: String,
        source: std::io::Error,
    },
}

pub struct RunConfig<'cfg> {
    pub command: Option<&'cfg str>,
    pub container_name: &'cfg str,
    pub env: &'cfg BTreeMap<String, String>,
    pub exec: Option<&'cfg [String]>,
    pub image: &'cfg str,
    pub network: &'cfg str,
    pub network_alias: &'cfg str,
    pub ports: &'cfg [String],
    pub volumes: &'cfg [String],
}

pub fn container_name(network: &str, process_name: &str) -> String {
    format!("{network}-{process_name}")
}

pub async fn build(tag: &str, context: &str, dockerfile: Option<&str>) -> Result<(), Error> {
    let mut cmd = process::Command::new("docker");
    cmd.arg("build").arg("--tag").arg(tag);
    if let Some(file) = dockerfile {
        cmd.arg("--file").arg(file);
    }
    cmd.arg(context);
    let output = cmd.output().await.map_err(|source| Error::Build {
        context: context.to_string(),
        source,
    })?;

    if !output.status.success() {
        let Some(code) = output.status.code() else {
            return Err(Error::BuildKilled {
                context: context.to_string(),
            });
        };
        return Err(Error::BuildFailed {
            context: context.to_string(),
            code,
        });
    }

    Ok(())
}

pub fn run_command(run: &RunConfig<'_>) -> process::Command {
    let mut cmd = process::Command::new("docker");
    cmd.arg("run")
        .arg("--rm")
        .arg("--name")
        .arg(run.container_name)
        .arg("--network")
        .arg(run.network)
        .arg("--network-alias")
        .arg(run.network_alias)
        .arg("--log-driver")
        .arg("none");

    for (key, value) in run.env {
        cmd.arg("--env").arg(format!("{key}={value}"));
    }

    for port in run.ports {
        cmd.arg("--publish").arg(port);
    }

    for volume in run.volumes {
        cmd.arg("--volume").arg(volume);
    }

    cmd.arg(run.image);

    match (run.command, run.exec) {
        (Some(shell_cmd), _) => {
            for arg in shell_cmd.split_whitespace() {
                cmd.arg(arg);
            }
        }
        (_, Some(args)) => {
            for arg in args {
                cmd.arg(arg);
            }
        }
        (None, None) => {}
    }

    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    cmd
}

pub async fn exec_check(container: &str, args: &[String]) -> Result<std::process::Output, Error> {
    let mut cmd = process::Command::new("docker");
    cmd.arg("exec").arg(container);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.output().await.map_err(|source| Error::Exec { source })
}

pub async fn exec_shell(container: &str, shell_cmd: &str) -> Result<std::process::Output, Error> {
    process::Command::new("docker")
        .arg("exec")
        .arg(container)
        .arg("sh")
        .arg("-c")
        .arg(shell_cmd)
        .output()
        .await
        .map_err(|source| Error::Exec { source })
}

pub async fn stop(name: &str, timeout_secs: Option<u64>) -> Result<(), Error> {
    let mut cmd = process::Command::new("docker");
    cmd.arg("stop");
    if let Some(secs) = timeout_secs {
        cmd.arg("--time").arg(secs.to_string());
    }
    cmd.arg(name);
    let _output = cmd.output().await.map_err(|source| Error::Stop {
        name: name.to_string(),
        source,
    })?;
    Ok(())
}

pub async fn rm(name: &str) -> Result<(), Error> {
    let _output = process::Command::new("docker")
        .arg("rm")
        .arg("--force")
        .arg(name)
        .output()
        .await
        .map_err(|source| Error::Rm { source })?;
    Ok(())
}

pub async fn network_create(name: &str) -> Result<(), Error> {
    let output = process::Command::new("docker")
        .arg("network")
        .arg("create")
        .arg(name)
        .output()
        .await
        .map_err(|source| Error::NetworkCreate { source })?;

    if !output.status.success() {
        let Some(code) = output.status.code() else {
            return Err(Error::NetworkCreateKilled);
        };
        return Err(Error::NetworkCreateFailed { code });
    }

    Ok(())
}

pub async fn network_rm(name: &str) -> Result<(), Error> {
    let _output = process::Command::new("docker")
        .arg("network")
        .arg("rm")
        .arg(name)
        .output()
        .await
        .map_err(|source| Error::NetworkRm { source })?;
    Ok(())
}
