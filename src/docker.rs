use std::collections::BTreeMap;
use std::os::unix::process::CommandExt as _;

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

    #[error("docker network create failed with exit code {code}: {stderr}")]
    NetworkCreateFailed { code: i32, stderr: String },

    #[error("docker network create failed (killed by signal)")]
    NetworkCreateKilled,

    #[error("docker network rm failed: {source}")]
    NetworkRm { source: std::io::Error },

    #[error("docker network rm failed with exit code {code}: {stderr}")]
    NetworkRmFailed { code: i32, stderr: String },

    #[error("docker network rm failed (killed by signal)")]
    NetworkRmKilled,

    #[error("docker rm failed: {source}")]
    Rm { source: std::io::Error },

    #[error("docker rm failed with exit code {code}: {stderr}")]
    RmFailed { code: i32, stderr: String },

    #[error("docker rm failed (killed by signal)")]
    RmKilled,

    #[error("docker stop failed for {name}: {source}")]
    Stop {
        name: String,
        source: std::io::Error,
    },

    #[error("docker stop failed for {name} with exit code {code}: {stderr}")]
    StopFailed {
        name: String,
        code: i32,
        stderr: String,
    },

    #[error("docker stop failed for {name} (killed by signal)")]
    StopKilled { name: String },
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

pub fn image_tag(network: &str, process_name: &str, image: Option<&str>) -> String {
    match image {
        Some(explicit) => explicit.to_string(),
        None => format!("{network}-{process_name}"),
    }
}

pub fn build_command(
    binary: &str,
    tag: &str,
    context: &str,
    dockerfile: Option<&str>,
    build_args: &BTreeMap<String, String>,
) -> process::Command {
    let mut cmd = process::Command::new(binary);
    cmd.arg("build")
        .arg("--progress=plain")
        .arg("--tag")
        .arg(tag);
    if let Some(file) = dockerfile {
        cmd.arg("--file").arg(file);
    }
    for (key, value) in build_args {
        cmd.arg("--build-arg").arg(format!("{key}={value}"));
    }
    cmd.arg(context);
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    cmd
}

pub fn run_command(binary: &str, run: &RunConfig<'_>) -> std::process::Command {
    let mut cmd = std::process::Command::new(binary);
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
        .process_group(0);

    cmd
}

pub async fn exec_check(
    binary: &str,
    container: &str,
    args: &[String],
) -> Result<std::process::Output, Error> {
    let mut cmd = process::Command::new(binary);
    cmd.arg("exec").arg(container);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.output().await.map_err(|source| Error::Exec { source })
}

pub async fn exec_shell(
    binary: &str,
    container: &str,
    shell_cmd: &str,
) -> Result<std::process::Output, Error> {
    process::Command::new(binary)
        .arg("exec")
        .arg(container)
        .arg("sh")
        .arg("-c")
        .arg(shell_cmd)
        .output()
        .await
        .map_err(|source| Error::Exec { source })
}

pub async fn stop(binary: &str, name: &str, timeout_secs: Option<u64>) -> Result<(), Error> {
    let mut cmd = process::Command::new(binary);
    cmd.arg("stop");
    if let Some(secs) = timeout_secs {
        cmd.arg("--time").arg(secs.to_string());
    }
    cmd.arg(name);
    let output = cmd.output().await.map_err(|source| Error::Stop {
        name: name.to_string(),
        source,
    })?;

    if !output.status.success() {
        let Some(code) = output.status.code() else {
            return Err(Error::StopKilled {
                name: name.to_string(),
            });
        };
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(Error::StopFailed {
            name: name.to_string(),
            code,
            stderr,
        });
    }

    Ok(())
}

pub async fn rm(binary: &str, name: &str) -> Result<(), Error> {
    let output = process::Command::new(binary)
        .arg("rm")
        .arg("--force")
        .arg(name)
        .output()
        .await
        .map_err(|source| Error::Rm { source })?;

    if !output.status.success() {
        let Some(code) = output.status.code() else {
            return Err(Error::RmKilled);
        };
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(Error::RmFailed { code, stderr });
    }

    Ok(())
}

pub async fn network_create(binary: &str, name: &str) -> Result<(), Error> {
    let output = process::Command::new(binary)
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
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(Error::NetworkCreateFailed { code, stderr });
    }

    Ok(())
}

pub async fn network_rm(binary: &str, name: &str) -> Result<(), Error> {
    let output = process::Command::new(binary)
        .arg("network")
        .arg("rm")
        .arg(name)
        .output()
        .await
        .map_err(|source| Error::NetworkRm { source })?;

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let is_real_failure = !output.status.success() && !stderr.contains("not found");

    if is_real_failure {
        match output.status.code() {
            Some(code) => Err(Error::NetworkRmFailed { code, stderr }),
            None => Err(Error::NetworkRmKilled),
        }
    } else {
        Ok(())
    }
}
