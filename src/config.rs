use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("circular dependency detected: {0}")]
    CircularDependency(String),

    #[error("{name}: docker process cannot have both command and exec")]
    DockerBothCommandAndExec { name: String },

    #[error("{name}: docker requires image or build")]
    DockerNoImageOrBuild { name: String },

    #[error("malformed env file {path} at line {line_number}: {line}")]
    EnvFileMalformed {
        path: PathBuf,
        line_number: usize,
        line: String,
    },

    #[error("{name}: exec driver requires command")]
    ExecNoCommand { name: String },

    #[error("ERROR: failed to read {path}: {source}")]
    FileRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("{name}: health check must specify either command or exec, not both")]
    HealthBoth { name: String },

    #[error("{name}: health check must specify command or exec")]
    HealthEmpty { name: String },

    #[error("last process {name} is oneshot — run mode requires a service as the final process")]
    LastProcessOneshot { name: String },

    #[error("no processes defined")]
    NoProcesses,

    #[error("{name}: oneshot processes cannot have health checks")]
    OneshotHealth { name: String },

    #[error("process {process} depends on unknown process: {dependency}")]
    UnknownDependency { dependency: String, process: String },

    #[error("failed to parse YAML from {path}: {source}")]
    YamlParse {
        path: PathBuf,
        source: serde_yml::Error,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub processes: IndexMap<String, ProcessDef>,
    #[serde(default)]
    pub runtime: Runtime,
    pub shell: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    #[default]
    Docker,
    Podman,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerConfig {
    pub build: Option<String>,
    #[serde(default)]
    pub build_args: Option<BTreeMap<String, String>>,
    pub dockerfile: Option<String>,
    pub image: Option<String>,
    #[serde(default)]
    pub ports: Vec<String>,
    pub stop_timeout_secs: Option<u64>,
    #[serde(default)]
    pub volumes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthCheck {
    pub command: Option<String>,
    pub exec: Option<Vec<String>>,
    pub interval_secs: u64,
    pub timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessDef {
    pub command: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub docker: Option<DockerConfig>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub env_file: Option<String>,
    pub exec: Option<Vec<String>>,
    pub health: Option<HealthCheck>,
    #[serde(rename = "type", default = "default_process_type")]
    pub process_type: ProcessType,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcessType {
    Oneshot,
    Service,
}

fn default_process_type() -> ProcessType {
    ProcessType::Service
}

pub fn runtime_binary(runtime: &Runtime) -> &str {
    match runtime {
        Runtime::Docker => "docker",
        Runtime::Podman => "podman",
    }
}

pub fn load(path: &Path) -> Result<Config, Error> {
    let content = std::fs::read_to_string(path).map_err(|source| Error::FileRead {
        path: path.to_path_buf(),
        source,
    })?;

    let config: Config = serde_yml::from_str(&content).map_err(|source| Error::YamlParse {
        path: path.to_path_buf(),
        source,
    })?;

    validate(&config)?;

    Ok(config)
}

pub fn parse_env_file(path: &Path) -> Result<BTreeMap<String, String>, Error> {
    let content = std::fs::read_to_string(path).map_err(|source| Error::FileRead {
        path: path.to_path_buf(),
        source,
    })?;

    let lines = content.lines().enumerate();
    let init = BTreeMap::new();
    lines
        .filter(|(_i, line)| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .try_fold(init, |mut acc, (i, line)| {
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| Error::EnvFileMalformed {
                    path: path.to_path_buf(),
                    line_number: i + 1,
                    line: line.to_string(),
                })?;
            let key_str = key.trim().to_string();
            let value_str = value.trim().to_string();
            acc.insert(key_str, value_str);
            Ok(acc)
        })
}

fn validate(config: &Config) -> Result<(), Error> {
    if config.processes.is_empty() {
        return Err(Error::NoProcesses);
    }

    for (name, proc_def) in &config.processes {
        check_driver(name, proc_def)?;
        check_health(name, proc_def)?;
    }

    check_unknown_dependencies(config)?;
    check_circular_dependencies(config)?;

    Ok(())
}

fn check_circular_dependencies(config: &Config) -> Result<(), Error> {
    let adj: HashMap<&str, Vec<&str>> = config
        .processes
        .iter()
        .map(|(name, proc_def)| {
            (
                name.as_str(),
                proc_def.depends_on.iter().map(String::as_str).collect(),
            )
        })
        .collect();

    let mut visited = HashSet::new();
    let mut in_stack = HashSet::new();
    let mut path: Vec<&str> = Vec::new();

    for name in config.processes.keys() {
        let name_str = name.as_str();
        if !visited.contains(name_str) {
            detect_cycle(name_str, &adj, &mut visited, &mut in_stack, &mut path)?;
        }
    }

    Ok(())
}

fn check_driver(name: &str, proc_def: &ProcessDef) -> Result<(), Error> {
    match &proc_def.docker {
        Some(docker) if docker.image.is_none() && docker.build.is_none() => {
            Err(Error::DockerNoImageOrBuild {
                name: name.to_string(),
            })
        }
        Some(_) if proc_def.command.is_some() && proc_def.exec.is_some() => {
            Err(Error::DockerBothCommandAndExec {
                name: name.to_string(),
            })
        }
        None if proc_def.command.is_none() => Err(Error::ExecNoCommand {
            name: name.to_string(),
        }),
        Some(_) | None => Ok(()),
    }
}

fn check_health(name: &str, proc_def: &ProcessDef) -> Result<(), Error> {
    match &proc_def.health {
        None => Ok(()),
        Some(_) if proc_def.process_type == ProcessType::Oneshot => Err(Error::OneshotHealth {
            name: name.to_string(),
        }),
        Some(health) => check_health_fields(name, health),
    }
}

fn check_health_fields(name: &str, health: &HealthCheck) -> Result<(), Error> {
    let has_command = health.command.as_ref().is_some_and(|cmd| !cmd.is_empty());
    let has_exec = health.exec.as_ref().is_some_and(|exec| !exec.is_empty());

    match (has_command, has_exec) {
        (false, false) => Err(Error::HealthEmpty {
            name: name.to_string(),
        }),
        (true, true) => Err(Error::HealthBoth {
            name: name.to_string(),
        }),
        _ => Ok(()),
    }
}

fn check_unknown_dependencies(config: &Config) -> Result<(), Error> {
    let names: HashSet<&str> = config.processes.keys().map(String::as_str).collect();
    for (name, proc_def) in &config.processes {
        for dep in &proc_def.depends_on {
            let dep_str = dep.as_str();
            if !names.contains(dep_str) {
                return Err(Error::UnknownDependency {
                    process: name.clone(),
                    dependency: dep.clone(),
                });
            }
        }
    }
    Ok(())
}

fn detect_cycle<'graph>(
    node: &'graph str,
    adj: &HashMap<&'graph str, Vec<&'graph str>>,
    visited: &mut HashSet<&'graph str>,
    in_stack: &mut HashSet<&'graph str>,
    path: &mut Vec<&'graph str>,
) -> Result<(), Error> {
    visited.insert(node);
    in_stack.insert(node);
    path.push(node);

    if let Some(deps) = adj.get(node) {
        for dep in deps {
            if in_stack.contains(dep) {
                let cycle = format_cycle(path, dep);
                let err = Error::CircularDependency(cycle);
                return Err(err);
            }
            if !visited.contains(dep) {
                detect_cycle(dep, adj, visited, in_stack, path)?;
            }
        }
    }

    in_stack.remove(node);
    path.pop();
    Ok(())
}

fn format_cycle(path: &[&str], back_edge: &str) -> String {
    let cycle_start = path.iter().position(|&nm| nm == back_edge);
    match cycle_start.and_then(|idx| path.get(idx..)) {
        Some(cycle_slice) => {
            let mut chain: Vec<&str> = cycle_slice.to_vec();
            chain.push(back_edge);
            chain.join(" -> ")
        }
        None => format!("{} -> {back_edge}", path.last().map_or("?", |nm| nm)),
    }
}
