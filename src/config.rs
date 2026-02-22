use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::Deserialize;

use crate::error::Error;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub process: Vec<ProcessDef>,
}

#[derive(Debug, Deserialize)]
pub struct ProcessDef {
    pub command: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub health: Option<String>,
    pub name: String,
}

pub fn load(path: &Path) -> Result<Config, Error> {
    let content = std::fs::read_to_string(path).map_err(|source| Error::FileRead {
        path: path.to_path_buf(),
        source,
    })?;

    let config: Config = toml::from_str(&content).map_err(|source| Error::TomlParse {
        path: path.to_path_buf(),
        source,
    })?;

    validate(&config)?;

    Ok(config)
}

fn validate(config: &Config) -> Result<(), Error> {
    if config.process.is_empty() {
        return Err(Error::NoProcesses);
    }

    check_duplicate_names(config)?;
    check_unknown_dependencies(config)?;
    check_circular_dependencies(config)?;

    Ok(())
}

fn check_duplicate_names(config: &Config) -> Result<(), Error> {
    let mut seen = HashSet::new();
    for proc in &config.process {
        if !seen.insert(&proc.name) {
            return Err(Error::DuplicateName(proc.name.clone()));
        }
    }
    Ok(())
}

fn check_unknown_dependencies(config: &Config) -> Result<(), Error> {
    let names: HashSet<&str> = config
        .process
        .iter()
        .map(|proc| proc.name.as_str())
        .collect();
    for proc in &config.process {
        for dep in &proc.depends_on {
            if !names.contains(dep.as_str()) {
                return Err(Error::UnknownDependency {
                    process: proc.name.clone(),
                    dependency: dep.clone(),
                });
            }
        }
    }
    Ok(())
}

fn check_circular_dependencies(config: &Config) -> Result<(), Error> {
    let adj: HashMap<&str, Vec<&str>> = config
        .process
        .iter()
        .map(|proc| {
            (
                proc.name.as_str(),
                proc.depends_on.iter().map(String::as_str).collect(),
            )
        })
        .collect();

    let mut visited = HashSet::new();
    let mut in_stack = HashSet::new();

    for proc in &config.process {
        if !visited.contains(proc.name.as_str()) {
            detect_cycle(proc.name.as_str(), &adj, &mut visited, &mut in_stack)?;
        }
    }

    Ok(())
}

fn detect_cycle<'graph>(
    node: &'graph str,
    adj: &HashMap<&'graph str, Vec<&'graph str>>,
    visited: &mut HashSet<&'graph str>,
    in_stack: &mut HashSet<&'graph str>,
) -> Result<(), Error> {
    visited.insert(node);
    in_stack.insert(node);

    if let Some(deps) = adj.get(node) {
        for dep in deps {
            if in_stack.contains(dep) {
                return Err(Error::CircularDependency(node.to_string()));
            }
            if !visited.contains(dep) {
                detect_cycle(dep, adj, visited, in_stack)?;
            }
        }
    }

    in_stack.remove(node);
    Ok(())
}
