use tokio::io::BufReader;
use tokio::sync::mpsc;

use crate::actor;
use crate::config;
use crate::docker;
use crate::message;
use crate::process;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("display channel closed: {0}")]
    DisplayChannelClosed(mpsc::error::SendError<message::DisplayMsg>),

    #[error(transparent)]
    Docker(#[from] docker::Error),
}

pub async fn build(
    config: &config::Config,
    start_order: &[String],
    network: &str,
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

        send_display(
            display_tx,
            message::DisplayMsg::Docker(message::DockerMsg::Building { name: name.clone() }),
        )?;

        let tag = docker::image_tag(network, name, docker_config.image.as_deref());

        let mut child = process::spawn_child(&mut docker::build_command(
            &tag,
            build_context,
            docker_config.dockerfile.as_deref(),
        ))
        .map_err(|source| docker::Error::Build {
            context: build_context.clone(),
            source,
        })?;

        let stdout_handle = child.stdout.take().map(|stdout| {
            let tx = display_tx.clone();
            let proc_name = name.clone();
            tokio::spawn(async move {
                actor::stream_lines(&proc_name, BufReader::new(stdout), &tx).await;
            })
        });

        let stderr_handle = child.stderr.take().map(|stderr| {
            let tx = display_tx.clone();
            let proc_name = name.clone();
            tokio::spawn(async move {
                actor::stream_lines(&proc_name, BufReader::new(stderr), &tx).await;
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
                return Err(docker::Error::BuildKilled {
                    context: build_context.clone(),
                }
                .into());
            };
            return Err(docker::Error::BuildFailed {
                context: build_context.clone(),
                code,
            }
            .into());
        }

        send_display(
            display_tx,
            message::DisplayMsg::Docker(message::DockerMsg::Built { name: name.clone() }),
        )?;
    }

    Ok(())
}

fn send_display(
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    msg: message::DisplayMsg,
) -> Result<(), Error> {
    display_tx.send(msg).map_err(Error::DisplayChannelClosed)
}
