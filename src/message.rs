#[derive(Debug)]
pub enum ActorEvent {
    Exited {
        code: u8,
        name: String,
        stderr_lines: Vec<String>,
    },
    HealthFailed {
        name: String,
        reason: String,
    },
    Healthy {
        name: String,
    },
}

#[derive(Debug)]
pub enum Command {
    Kill,
}

#[derive(Debug)]
pub enum DockerMsg {
    Building { name: String },
    Built { name: String },
    CleanupError { name: String, reason: String },
    CreatingNetwork { name: String },
    RemovingContainers { name: String },
    RemovingNetwork { name: String },
}

#[derive(Debug)]
pub enum LifecycleMsg {
    Completed {
        name: String,
    },
    Crashed {
        code: u8,
        name: String,
    },
    Exited {
        code: u8,
        name: String,
    },
    HealthFailed {
        name: String,
        reason: String,
    },
    Healthy {
        name: String,
    },
    Killing {
        name: String,
        reason: String,
    },
    Starting {
        command: String,
        name: String,
    },
    Summary {
        name: String,
        stderr_lines: Vec<String>,
    },
    WaitingCompletion {
        name: String,
    },
    WaitingHealth {
        name: String,
    },
}

#[derive(Debug)]
pub enum OutputMsg {
    Line { line: String, name: String },
}

#[derive(Debug)]
pub enum DisplayMsg {
    Docker(DockerMsg),
    Lifecycle(LifecycleMsg),
    Output(OutputMsg),
}

pub struct ActorHandle {
    pub cmd_tx: tokio::sync::mpsc::UnboundedSender<Command>,
    pub join: tokio::task::JoinHandle<()>,
    pub name: String,
}

impl ActorHandle {
    pub fn kill(&self) {
        drop(self.cmd_tx.send(Command::Kill));
    }
}

pub struct ExitResult {
    pub code: u8,
    pub failed_name: Option<String>,
}

pub fn display_msg_name(msg: &DisplayMsg) -> &str {
    match msg {
        DisplayMsg::Docker(docker) => match docker {
            DockerMsg::Building { name }
            | DockerMsg::Built { name }
            | DockerMsg::CleanupError { name, .. }
            | DockerMsg::CreatingNetwork { name }
            | DockerMsg::RemovingContainers { name }
            | DockerMsg::RemovingNetwork { name } => name,
        },
        DisplayMsg::Lifecycle(lifecycle) => match lifecycle {
            LifecycleMsg::Completed { name }
            | LifecycleMsg::Crashed { name, .. }
            | LifecycleMsg::Exited { name, .. }
            | LifecycleMsg::HealthFailed { name, .. }
            | LifecycleMsg::Healthy { name }
            | LifecycleMsg::Killing { name, .. }
            | LifecycleMsg::Starting { name, .. }
            | LifecycleMsg::Summary { name, .. }
            | LifecycleMsg::WaitingCompletion { name }
            | LifecycleMsg::WaitingHealth { name } => name,
        },
        DisplayMsg::Output(OutputMsg::Line { name, .. }) => name,
    }
}
