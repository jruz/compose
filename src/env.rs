use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use crate::config;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("circular variable interpolation: {cycle}")]
    CircularInterpolation { cycle: String },

    #[error(transparent)]
    Config(#[from] config::Error),
}

pub fn resolve(
    global_env: &BTreeMap<String, String>,
    proc_def: &config::ProcessDef,
) -> Result<BTreeMap<String, String>, Error> {
    let mut env = global_env.clone();
    load_process_env_file(&mut env, proc_def)?;
    apply_inline_env(&mut env, proc_def);
    interpolate(&mut env)?;
    Ok(env)
}

fn apply_inline_env(env: &mut BTreeMap<String, String>, proc_def: &config::ProcessDef) {
    for (key, value) in &proc_def.env {
        env.insert(key.clone(), value.clone());
    }
}

fn interpolate(env: &mut BTreeMap<String, String>) -> Result<(), Error> {
    detect_interpolation_cycles(env)?;
    let snapshot: BTreeMap<String, String> = env.clone();
    for value in env.values_mut() {
        *value = interpolate_value(value, &snapshot);
    }
    Ok(())
}

fn interpolate_value(value: &str, env: &BTreeMap<String, String>) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'{') {
            chars.next();
            let var_name: String = chars
                .by_ref()
                .take_while(|&closing| closing != '}')
                .collect();
            if let Some(resolved) = env.get(&var_name) {
                result.push_str(resolved);
            } else {
                result.push_str("${");
                result.push_str(&var_name);
                result.push('}');
            }
        } else {
            result.push(ch);
        }
    }

    result
}

fn detect_interpolation_cycles(env: &BTreeMap<String, String>) -> Result<(), Error> {
    let refs = build_ref_graph(env);
    let mut visited: HashSet<&str> = HashSet::new();
    let mut in_stack: HashSet<&str> = HashSet::new();
    let mut path: Vec<&str> = Vec::new();

    for key in env.keys() {
        if !visited.contains(key.as_str()) {
            detect_ref_cycle(key, &refs, &mut visited, &mut in_stack, &mut path)?;
        }
    }

    Ok(())
}

fn build_ref_graph(env: &BTreeMap<String, String>) -> BTreeMap<&str, Vec<String>> {
    env.iter()
        .map(|(key, value)| {
            let deps: Vec<String> = parse_var_refs(value)
                .into_iter()
                .filter(|var| env.contains_key(var.as_str()))
                .collect();
            (key.as_str(), deps)
        })
        .collect()
}

fn detect_ref_cycle<'graph>(
    node: &'graph str,
    refs: &'graph BTreeMap<&'graph str, Vec<String>>,
    visited: &mut HashSet<&'graph str>,
    in_stack: &mut HashSet<&'graph str>,
    path: &mut Vec<&'graph str>,
) -> Result<(), Error> {
    visited.insert(node);
    in_stack.insert(node);
    path.push(node);

    if let Some(deps) = refs.get(node) {
        for dep in deps {
            if in_stack.contains(dep.as_str()) {
                let cycle = format_cycle(path, dep);
                return Err(Error::CircularInterpolation { cycle });
            }
            if !visited.contains(dep.as_str()) {
                detect_ref_cycle(dep, refs, visited, in_stack, path)?;
            }
        }
    }

    in_stack.remove(node);
    path.pop();
    Ok(())
}

fn parse_var_refs(value: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut chars = value.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'{') {
            chars.next();
            let var_name: String = chars
                .by_ref()
                .take_while(|&closing| closing != '}')
                .collect();
            if !var_name.is_empty() {
                refs.push(var_name);
            }
        }
    }

    refs
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

pub fn load_global(path: Option<&Path>) -> Result<BTreeMap<String, String>, Error> {
    match path {
        Some(path) => Ok(config::parse_env_file(path)?),
        None => Ok(BTreeMap::new()),
    }
}

fn load_process_env_file(
    env: &mut BTreeMap<String, String>,
    proc_def: &config::ProcessDef,
) -> Result<(), Error> {
    if let Some(path) = &proc_def.env_file {
        let file_env = config::parse_env_file(Path::new(path))?;
        for (key, value) in file_env {
            env.insert(key, value);
        }
    }
    Ok(())
}
