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
pub enum DisplayMsg {
    Building {
        name: String,
    },
    Built {
        name: String,
    },
    CleanupError {
        name: String,
        reason: String,
    },
    Completed {
        name: String,
    },
    Crashed {
        name: String,
        code: u8,
    },
    CreatingNetwork {
        name: String,
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
    Output {
        name: String,
        line: String,
    },
    RemovingContainers {
        name: String,
    },
    RemovingNetwork {
        name: String,
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

pub struct ExitResult {
    pub code: u8,
    pub failed_name: Option<String>,
}

pub fn display_msg_name(msg: &DisplayMsg) -> &str {
    match msg {
        DisplayMsg::Building { name }
        | DisplayMsg::Built { name }
        | DisplayMsg::CleanupError { name, .. }
        | DisplayMsg::Completed { name }
        | DisplayMsg::Crashed { name, .. }
        | DisplayMsg::CreatingNetwork { name }
        | DisplayMsg::Exited { name, .. }
        | DisplayMsg::HealthFailed { name, .. }
        | DisplayMsg::Healthy { name }
        | DisplayMsg::Killing { name, .. }
        | DisplayMsg::Output { name, .. }
        | DisplayMsg::RemovingContainers { name }
        | DisplayMsg::RemovingNetwork { name }
        | DisplayMsg::Starting { name, .. }
        | DisplayMsg::Summary { name, .. }
        | DisplayMsg::WaitingCompletion { name }
        | DisplayMsg::WaitingHealth { name } => name,
    }
}
