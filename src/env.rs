use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::config;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("circular variable interpolation: {cycle}")]
    CircularInterpolation { cycle: String },

    #[error(transparent)]
    Config(#[from] config::Error),

    #[error("could not run sops for {path}: {source}")]
    SopsCommand {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("sops could not decrypt {path} ({status}); check the file and decryption credentials")]
    SopsDecrypt {
        path: PathBuf,
        status: std::process::ExitStatus,
    },

    #[error("sops output for {path} is not valid dotenv")]
    SopsOutput { path: PathBuf },
}

pub struct Global {
    pub build: BTreeMap<String, String>,
    pub runtime: BTreeMap<String, String>,
}

pub struct Runtime {
    pub image: Option<String>,
    pub vars: BTreeMap<String, String>,
}

pub fn resolve(
    global_env: &BTreeMap<String, String>,
    proc_def: &config::ProcessDef,
) -> Result<BTreeMap<String, String>, Error> {
    let path = proc_def.env_file.as_deref().map(Path::new);
    let dotenv = load_dotenv(path)?;
    let vars = merge(global_env, dotenv);
    resolve_inline(vars, proc_def)
}

pub async fn resolve_runtime(
    global: &Global,
    proc_def: &config::ProcessDef,
) -> Result<Runtime, Error> {
    let dotenv_path = proc_def.env_file.as_deref().map(Path::new);
    let dotenv = load_dotenv(dotenv_path)?;
    let sops_path = proc_def.sops_file.as_deref();
    let sops = load_sops(sops_path).await?;
    let image = match proc_def
        .docker
        .as_ref()
        .and_then(|docker| docker.image.as_ref())
    {
        Some(image) => {
            let build_dotenv = dotenv.clone();
            let build_vars = merge(&global.build, build_dotenv);
            let plain = resolve_inline(build_vars, proc_def)?;
            let image = expand_vars(image, &plain);
            Some(image)
        }
        None => None,
    };
    let mut vars = merge(&global.runtime, dotenv);
    vars.extend(sops);
    let vars = resolve_inline(vars, proc_def)?;
    Ok(Runtime { image, vars })
}

fn resolve_inline(
    mut vars: BTreeMap<String, String>,
    proc_def: &config::ProcessDef,
) -> Result<BTreeMap<String, String>, Error> {
    let inline = proc_def.environment.clone();
    vars.extend(inline);
    interpolate(&mut vars)?;
    Ok(vars)
}

fn merge(
    base: &BTreeMap<String, String>,
    overrides: BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut vars = base.clone();
    vars.extend(overrides);
    vars
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

pub async fn load_global(
    config: &config::Config,
    cli_path: Option<&Path>,
) -> Result<Global, Error> {
    let shared_path = config.env_file.as_deref();
    let shared = load_dotenv(shared_path)?;
    let cli = load_dotenv(cli_path)?;
    let build_cli = cli.clone();
    let build = merge(&shared, build_cli);
    let sops_path = config.sops_file.as_deref();
    let sops = load_sops(sops_path).await?;
    let mut runtime = merge(&shared, sops);
    runtime.extend(cli);
    Ok(Global { build, runtime })
}

fn load_dotenv(path: Option<&Path>) -> Result<BTreeMap<String, String>, Error> {
    let Some(path) = path else {
        let vars = BTreeMap::new();
        return Ok(vars);
    };
    let vars = config::parse_env_file(path)?;
    Ok(vars)
}

async fn load_sops(path: Option<&Path>) -> Result<BTreeMap<String, String>, Error> {
    let Some(path) = path else {
        let vars = BTreeMap::new();
        return Ok(vars);
    };
    let stdin = Stdio::null();
    let output = tokio::process::Command::new("sops")
        .args(["--decrypt", "--output-type", "dotenv", "--"])
        .arg(path)
        .stdin(stdin)
        .kill_on_drop(true)
        .output()
        .await;
    let output = match output {
        Ok(output) => output,
        Err(source) => {
            return Err(Error::SopsCommand {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if !output.status.success() {
        return Err(Error::SopsDecrypt {
            path: path.to_path_buf(),
            status: output.status,
        });
    }
    let content = match std::str::from_utf8(&output.stdout) {
        Ok(content) if !content.contains('\0') => content,
        _ => {
            return Err(Error::SopsOutput {
                path: path.to_path_buf(),
            });
        }
    };
    let vars = config::parse_dotenv(path, content)?;
    if vars.contains_key("") {
        return Err(Error::SopsOutput {
            path: path.to_path_buf(),
        });
    }
    Ok(vars)
}
