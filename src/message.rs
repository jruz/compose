#[derive(Debug)]
pub enum Event {
    Exited {
        name: String,
        code: u8,
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
        name: String,
        code: u8,
    },
    Exited {
        name: String,
        code: u8,
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
        name: String,
        command: String,
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
    Line { name: String, line: String },
}

#[derive(Debug)]
pub enum DisplayMsg {
    Docker(DockerMsg),
    Lifecycle(LifecycleMsg),
    Output(OutputMsg),
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
