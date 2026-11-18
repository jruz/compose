use std::collections::BTreeMap;

use indexmap::IndexMap;

use crate::config;
use crate::env;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("docker.{section}.{flag}: use a long CLI flag name without leading dashes")]
    InvalidName { section: &'static str, flag: String },

    #[error("docker.{section}.{flag}: flag is managed by Compose")]
    Reserved { section: &'static str, flag: String },

    #[error(
        "docker.{section}.{flag}: use a scalar or list; maps are only supported for build-arg and label"
    )]
    UnsupportedMap { section: &'static str, flag: String },
}

#[derive(Debug, serde::Deserialize)]
pub struct Build {
    pub context: String,
    #[serde(flatten)]
    pub flags: Flags,
}

pub type Flags = IndexMap<String, Value>;

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum Value {
    List(Vec<Scalar>),
    Map(IndexMap<String, Scalar>),
    Scalar(Scalar),
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum Scalar {
    Bool(bool),
    Number(serde_yml::Number),
    Text(String),
}

pub fn validate(docker: &config::DockerConfig) -> Result<(), Error> {
    if let Some(build) = &docker.build {
        validate_flags("build", &build.flags, &["tag", "progress", "help"])?;
    }
    validate_flags(
        "run",
        &docker.run,
        &[
            "attach",
            "detach",
            "help",
            "log-driver",
            "name",
            "net",
            "network",
            "network-alias",
            "restart",
            "rm",
            "stop-timeout",
        ],
    )
}

fn validate_flags(section: &'static str, flags: &Flags, reserved: &[&str]) -> Result<(), Error> {
    for (flag, value) in flags {
        if !valid_name(flag) {
            return Err(Error::InvalidName {
                section,
                flag: flag.clone(),
            });
        }
        let flag_name = flag.as_str();
        if reserved.contains(&flag_name) {
            return Err(Error::Reserved {
                section,
                flag: flag.clone(),
            });
        }
        if matches!(value, Value::Map(_)) && !supports_map(section, flag) {
            return Err(Error::UnsupportedMap {
                section,
                flag: flag.clone(),
            });
        }
    }
    Ok(())
}

fn valid_name(flag: &str) -> bool {
    let mut chars = flag.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && flag.len() > 1
}

fn supports_map(section: &str, flag: &str) -> bool {
    flag == "label" || (section == "build" && flag == "build-arg")
}

pub fn build_args(flags: &Flags, env: &BTreeMap<String, String>) -> Vec<String> {
    let mut args = args(flags, env);
    if !flags.contains_key("build-arg") {
        let forwarded = env
            .iter()
            .map(|(key, value)| format!("--build-arg={key}={value}"));
        args.extend(forwarded);
    }
    args
}

pub fn args(flags: &Flags, env: &BTreeMap<String, String>) -> Vec<String> {
    flags
        .iter()
        .flat_map(|(name, value)| flag_args(name, value, env))
        .collect()
}

fn flag_args(name: &str, flag_value: &Value, env: &BTreeMap<String, String>) -> Vec<String> {
    match flag_value {
        Value::Scalar(value) => vec![scalar_arg(name, value, env)],
        Value::List(values) => values
            .iter()
            .map(|value| scalar_arg(name, value, env))
            .collect(),
        Value::Map(entries) => entries
            .iter()
            .map(|(key, value)| {
                let value = scalar_value(value, env);
                format!("--{name}={key}={value}")
            })
            .collect(),
    }
}

fn scalar_arg(name: &str, value: &Scalar, env: &BTreeMap<String, String>) -> String {
    let value = scalar_value(value, env);
    format!("--{name}={value}")
}

fn scalar_value(value: &Scalar, env: &BTreeMap<String, String>) -> String {
    match value {
        Scalar::Text(value) => env::expand_vars(value, env),
        Scalar::Bool(value) => value.to_string(),
        Scalar::Number(value) => value.to_string(),
    }
}
