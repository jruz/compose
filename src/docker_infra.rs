use std::collections::BTreeMap;

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
    global_env: &BTreeMap<String, String>,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) -> Result<String, Error> {
    let network = generate_network_name(project_name);
    let msg = message::DisplayMsg::Docker(message::DockerMsg::CreatingNetwork {
        name: binary.to_string(),
    });
    send_display(display_tx, msg)?;
    remove_containers(binary, config, start_order, &network, display_tx).await;
    docker::network_rm(binary, &network).await?;
    docker::network_create(binary, &network).await?;
    build_actor::build(
        binary,
        config,
        start_order,
        &network,
        global_env,
        display_tx,
    )
    .await?;
    Ok(network)
}

pub async fn cleanup(
    binary: &str,
    config: &config::Config,
    start_order: &[String],
    network: &str,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) {
    remove_containers(binary, config, start_order, network, display_tx).await;

    let removing_msg = message::DisplayMsg::Docker(message::DockerMsg::RemovingNetwork {
        name: binary.to_string(),
    });
    if let Err(_send_err) = send_display(display_tx, removing_msg) {}
    if let Err(rm_err) = docker::network_rm(binary, network).await {
        let error_msg = message::DisplayMsg::Docker(message::DockerMsg::CleanupError {
            name: binary.to_string(),
            reason: rm_err.to_string(),
        });
        if let Err(_send_err) = send_display(display_tx, error_msg) {}
    }
}

async fn remove_containers(
    binary: &str,
    config: &config::Config,
    start_order: &[String],
    network: &str,
    display_tx: &mpsc::UnboundedSender<message::DisplayMsg>,
) {
    let docker_names: Vec<String> = start_order
        .iter()
        .filter(|name| {
            let key = name.as_str();
            config
                .processes
                .get(key)
                .is_some_and(|proc| proc.docker.is_some())
        })
        .cloned()
        .collect();

    if docker_names.is_empty() {
        return;
    }

    let removing_msg = message::DisplayMsg::Docker(message::DockerMsg::RemovingContainers {
        name: binary.to_string(),
    });
    if let Err(_send_err) = send_display(display_tx, removing_msg) {}
    for name in &docker_names {
        let container = docker::container_name(network, name);
        if let Err(rm_err) = docker::rm(binary, &container).await {
            let error_msg = message::DisplayMsg::Docker(message::DockerMsg::CleanupError {
                name: binary.to_string(),
                reason: rm_err.to_string(),
            });
            if let Err(_send_err) = send_display(display_tx, error_msg) {}
        }
    }
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
