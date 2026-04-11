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
        let key = key.clone();
        let value = value.clone();
        env.insert(key, value);
    }
}

fn interpolate(env: &mut BTreeMap<String, String>) -> Result<(), Error> {
    detect_interpolation_cycles(env)?;
    loop {
        let snapshot = env.clone();
        let keys: Vec<String> = env.keys().cloned().collect();
        for key in &keys {
            if let Some(value) = env.get_mut(key) {
                *value = interpolate_value(key, value, &snapshot);
            }
        }
        if *env == snapshot {
            break;
        }
    }
    Ok(())
}

pub fn expand_vars(value: &str, env: &BTreeMap<String, String>) -> String {
    let capacity = value.len();
    let mut result = String::with_capacity(capacity);
    let chars_iter = value.chars();
    let mut chars = chars_iter.peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'{') {
            chars.next();
            let var_name: String = chars
                .by_ref()
                .take_while(|&closing| closing != '}')
                .collect();
            if let Some(resolved) = env.get(&var_name) {
                result.push_str(resolved);
            } else if let Ok(host_val) = std::env::var(&var_name) {
                result.push_str(&host_val);
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

fn interpolate_value(key: &str, value: &str, env: &BTreeMap<String, String>) -> String {
    let capacity = value.len();
    let mut result = String::with_capacity(capacity);
    let chars_iter = value.chars();
    let mut chars = chars_iter.peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'{') {
            chars.next();
            let var_name: String = chars
                .by_ref()
                .take_while(|&closing| closing != '}')
                .collect();
            if var_name == key {
                if let Ok(host_val) = std::env::var(&var_name) {
                    result.push_str(&host_val);
                } else {
                    result.push_str("${");
                    result.push_str(&var_name);
                    result.push('}');
                }
            } else if let Some(resolved) = env.get(&var_name) {
                result.push_str(resolved);
            } else if let Ok(host_val) = std::env::var(&var_name) {
                result.push_str(&host_val);
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
        let key_str = key.as_str();
        if !visited.contains(key_str) {
            detect_ref_cycle(key, &refs, &mut visited, &mut in_stack, &mut path)?;
        }
    }

    Ok(())
}

fn build_ref_graph(env: &BTreeMap<String, String>) -> BTreeMap<&str, Vec<String>> {
    env.iter()
        .map(|(key, value)| {
            let refs = parse_var_refs(value);
            let deps: Vec<String> = refs
                .into_iter()
                .filter(|var| {
                    let var_str = var.as_str();
                    var != key && env.contains_key(var_str)
                })
                .collect();
            let key_str = key.as_str();
            (key_str, deps)
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
            let dep_str = dep.as_str();
            if in_stack.contains(dep_str) {
                let cycle = format_cycle(path, dep);
                return Err(Error::CircularInterpolation { cycle });
            }
            if !visited.contains(dep_str) {
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
    let chars_iter = value.chars();
    let mut chars = chars_iter.peekable();

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
    if let Some(path) = path {
        let env = config::parse_env_file(path)?;
        Ok(env)
    } else {
        let empty = BTreeMap::new();
        Ok(empty)
    }
}

fn load_process_env_file(
    env: &mut BTreeMap<String, String>,
    proc_def: &config::ProcessDef,
) -> Result<(), Error> {
    if let Some(path) = &proc_def.env_file {
        let env_path = Path::new(path);
        let file_env = config::parse_env_file(env_path)?;
        for (key, value) in file_env {
            env.insert(key, value);
        }
    }
    Ok(())
}
