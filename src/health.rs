use std::collections::BTreeMap;
use std::time::Duration;

use crate::config;
use crate::docker;
use crate::env;

pub struct HealthConfig {
    pub interval: Duration,
    pub test: Vec<String>,
    pub timeout: Duration,
}

pub enum HealthDriver {
    Docker { binary: String, container: String },
    Exec,
}

pub fn resolve_config(hc: &config::HealthCheck, env: &BTreeMap<String, String>) -> HealthConfig {
    HealthConfig {
        interval: Duration::from_secs(hc.interval_secs),
        test: hc
            .test
            .iter()
            .map(|arg| env::expand_vars(arg, env))
            .collect(),
        timeout: Duration::from_secs(hc.timeout_secs),
    }
}

pub async fn check(driver: &HealthDriver, test: &[String]) -> Result<std::process::Output, String> {
    match driver {
        HealthDriver::Exec => run_exec(test).await.map_err(|err| err.to_string()),
        HealthDriver::Docker { binary, container } => docker::exec_check(binary, container, test)
            .await
            .map_err(|err| err.to_string()),
    }
}

async fn run_exec(test: &[String]) -> std::io::Result<std::process::Output> {
    let (program, args) = test.split_first().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "healthcheck.test is empty",
        )
    })?;
    tokio::process::Command::new(program)
        .args(args)
        .output()
        .await
}
