use tokio::sync::mpsc;

use crate::build_actor;
use crate::config;
use crate::docker;
use crate::message;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Build(#[from] build_actor::Error),

    #[error("display channel closed: {0}")]
    DisplayChannelClosed(mpsc::error::SendError<message::DisplayMsg>),

    #[error(transparent)]
    Docker(#[from] docker::Error),
}

pub async fn setup(
    binary: &str,
    config: &config::Config,
    start_order: &[String],
    project_name: Option<&str>,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<String, Error> {
    let network = generate_network_name(project_name);
    send_display(
        display_tx,
        message::DisplayMsg::Docker(message::DockerMsg::CreatingNetwork {
            name: binary.to_string(),
        }),
    )?;
    docker::network_create(binary, &network).await?;
    build_actor::build(binary, config, start_order, &network, display_tx).await?;
    Ok(network)
}

pub async fn cleanup(
    binary: &str,
    config: &config::Config,
    start_order: &[String],
    network: &str,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) {
    let docker_names: Vec<String> = start_order
        .iter()
        .filter(|name| {
            config
                .processes
                .get(name.as_str())
                .is_some_and(|proc| proc.docker.is_some())
        })
        .cloned()
        .collect();

    if !docker_names.is_empty() {
        if let Err(_send_err) = send_display(
            display_tx,
            message::DisplayMsg::Docker(message::DockerMsg::RemovingContainers {
                name: binary.to_string(),
            }),
        ) {}
        for name in &docker_names {
            let container = docker::container_name(network, name);
            if let Err(rm_err) = docker::rm(binary, &container).await
                && let Err(_send_err) = send_display(
                    display_tx,
                    message::DisplayMsg::Docker(message::DockerMsg::CleanupError {
                        name: binary.to_string(),
                        reason: rm_err.to_string(),
                    }),
                )
            {}
        }
    }

    if let Err(_send_err) = send_display(
        display_tx,
        message::DisplayMsg::Docker(message::DockerMsg::RemovingNetwork {
            name: binary.to_string(),
        }),
    ) {}
    if let Err(rm_err) = docker::network_rm(binary, network).await
        && let Err(_send_err) = send_display(
            display_tx,
            message::DisplayMsg::Docker(message::DockerMsg::CleanupError {
                name: binary.to_string(),
                reason: rm_err.to_string(),
            }),
        )
    {}
}

fn generate_network_name(project_name: Option<&str>) -> String {
    match project_name {
        Some(name) => format!("compose-{name}"),
        None => format!("compose-{}", std::process::id()),
    }
}

fn send_display(
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
    msg: message::DisplayMsg,
) -> Result<(), Error> {
    display_tx.send(msg).map_err(Error::DisplayChannelClosed)
}
