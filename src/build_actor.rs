use std::collections::BTreeMap;

use tokio::io::BufReader;
use tokio::sync::mpsc;

use crate::actor;
use crate::config;
use crate::docker;
use crate::env;
use crate::message;
use crate::process;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("display channel closed: {0}")]
    DisplayChannelClosed(mpsc::error::SendError<message::DisplayMsg>),

    #[error(transparent)]
    Docker(#[from] docker::Error),

    #[error(transparent)]
    Env(#[from] env::Error),
}

pub async fn build(
    binary: &str,
    config: &config::Config,
    start_order: &[String],
    network: &str,
    global_env: &BTreeMap<String, String>,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<(), Error> {
    for name in start_order {
        let Some(proc_def) = config.processes.get(name) else {
            continue;
        };
        let Some(docker_config) = &proc_def.docker else {
            continue;
        };
        let Some(build_context) = &docker_config.build else {
            continue;
        };

        let building_msg =
            message::DisplayMsg::Docker(message::DockerMsg::Building { name: name.clone() });
        send_display(display_tx, building_msg)?;

        let image = docker_config.image.as_deref();
        let tag = docker::image_tag(network, name, image);
        let build_args = resolve_build_args(docker_config, global_env, proc_def)?;

        let dockerfile = docker_config.dockerfile.as_deref();
        let mut build_cmd =
            docker::build_command(binary, &tag, build_context, dockerfile, &build_args);
        let mut child =
            process::spawn_child(&mut build_cmd).map_err(|source| docker::Error::Build {
                context: build_context.clone(),
                source,
            })?;

        let stdout_handle = child.stdout.take().map(|stdout| {
            let tx = display_tx.clone();
            let proc_name = name.clone();
            tokio::spawn(async move {
                let buffered_stdout = BufReader::new(stdout);
                actor::stream_lines(&proc_name, buffered_stdout, &tx).await;
            })
        });

        let stderr_handle = child.stderr.take().map(|stderr| {
            let tx = display_tx.clone();
            let proc_name = name.clone();
            tokio::spawn(async move {
                let buffered_stderr = BufReader::new(stderr);
                actor::stream_lines(&proc_name, buffered_stderr, &tx).await;
            })
        });

        let status = child.wait().await.map_err(|source| docker::Error::Build {
            context: build_context.clone(),
            source,
        })?;

        if let Some(handle) = stdout_handle {
            drop(handle.await);
        }
        if let Some(handle) = stderr_handle {
            drop(handle.await);
        }

        if !status.success() {
            let Some(code) = status.code() else {
                let err = docker::Error::BuildKilled {
                    context: build_context.clone(),
                };
                let err: Error = err.into();
                return Err(err);
            };
            let err = docker::Error::BuildFailed {
                context: build_context.clone(),
                code,
            };
            let err: Error = err.into();
            return Err(err);
        }

        let built_msg =
            message::DisplayMsg::Docker(message::DockerMsg::Built { name: name.clone() });
        send_display(display_tx, built_msg)?;
    }

    Ok(())
}

fn resolve_build_args(
    docker_config: &config::DockerConfig,
    global_env: &BTreeMap<String, String>,
    proc_def: &config::ProcessDef,
) -> Result<BTreeMap<String, String>, Error> {
    if let Some(args) = &docker_config.build_args {
        let cloned = args.clone();
        Ok(cloned)
    } else {
        let resolved = env::resolve(global_env, proc_def)?;
        Ok(resolved)
    }
}

fn send_display(
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    msg: message::DisplayMsg,
) -> Result<(), Error> {
    display_tx.send(msg).map_err(Error::DisplayChannelClosed)
}
