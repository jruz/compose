use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("circular dependency detected: {0}")]
    CircularDependency(String),

    #[error("{name}: {source}")]
    DockerFlags {
        name: String,
        source: crate::docker_flags::Error,
    },

    #[error("{name}: docker requires image or build")]
    DockerNoImageOrBuild { name: String },

    #[error("malformed env file {path} at line {line_number}: expected KEY=VALUE")]
    EnvFileMalformed { path: PathBuf, line_number: usize },

    #[error("{name}: exec driver requires command")]
    ExecNoCommand { name: String },

    #[error("ERROR: failed to read {path}: {source}")]
    FileRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("{name}: healthcheck.test must start with a non-empty executable name")]
    HealthTest { name: String },

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
    pub env_file: Option<PathBuf>,
    #[serde(default)]
    pub runtime: Runtime,
    pub services: IndexMap<String, ProcessDef>,
    pub sops_file: Option<PathBuf>,
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
    pub build: Option<crate::docker_flags::Build>,
    pub image: Option<String>,
    #[serde(default)]
    pub run: crate::docker_flags::Flags,
    pub stop_timeout_secs: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthCheck {
    pub interval_secs: u64,
    pub test: Vec<String>,
    pub timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessDef {
    pub command: Option<Command>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub docker: Option<DockerConfig>,
    pub env_file: Option<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    pub healthcheck: Option<HealthCheck>,
    #[serde(rename = "type", default = "default_process_type")]
    pub process_type: ProcessType,
    pub sops_file: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Command {
    List(Vec<String>),
    String(String),
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

    parse_dotenv(path, &content)
}

pub fn parse_dotenv(path: &Path, content: &str) -> Result<BTreeMap<String, String>, Error> {
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
                })?;
            let key_str = key.trim().to_string();
            let value_str = value.trim().to_string();
            acc.insert(key_str, value_str);
            Ok(acc)
        })
}

fn validate(config: &Config) -> Result<(), Error> {
    if config.services.is_empty() {
        return Err(Error::NoProcesses);
    }

    for (name, proc_def) in &config.services {
        check_driver(name, proc_def)?;
        check_health(name, proc_def)?;
    }

    check_unknown_dependencies(config)?;
    check_circular_dependencies(config)?;

    Ok(())
}

fn check_circular_dependencies(config: &Config) -> Result<(), Error> {
    let adj: HashMap<&str, Vec<&str>> = config
        .services
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

    for name in config.services.keys() {
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
        None if proc_def.command.is_none() => Err(Error::ExecNoCommand {
            name: name.to_string(),
        }),
        Some(docker) => match crate::docker_flags::validate(docker) {
            Ok(()) => Ok(()),
            Err(source) => Err(Error::DockerFlags {
                name: name.to_string(),
                source,
            }),
        },
        None => Ok(()),
    }
}

fn check_health(name: &str, proc_def: &ProcessDef) -> Result<(), Error> {
    match &proc_def.healthcheck {
        None => Ok(()),
        Some(_) if proc_def.process_type == ProcessType::Oneshot => Err(Error::OneshotHealth {
            name: name.to_string(),
        }),
        Some(health)
            if health
                .test
                .first()
                .is_some_and(|program| !program.is_empty()) =>
        {
            Ok(())
        }
        Some(_) => Err(Error::HealthTest {
            name: name.to_string(),
        }),
    }
}

fn check_unknown_dependencies(config: &Config) -> Result<(), Error> {
    let names: HashSet<&str> = config.services.keys().map(String::as_str).collect();
    for (name, proc_def) in &config.services {
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
