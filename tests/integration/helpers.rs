use std::path::PathBuf;
use std::process::Output;

pub fn binary_path() -> PathBuf {
    env!("CARGO_BIN_EXE_compose").into()
}

pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

pub fn run_fixture(fixture: &str, subcommand: &str) -> Output {
    let bin = binary_path();
    let config = fixture_path(fixture);
    match std::process::Command::new(bin)
        .arg(subcommand)
        .arg("--config")
        .arg(config)
        .output()
    {
        Ok(output) => output,
        Err(err) => panic!("failed to execute binary: {err}"),
    }
}

pub fn run_with_env_file(fixture: &str, subcommand: &str, env_file: &str) -> Output {
    let bin = binary_path();
    let config = fixture_path(fixture);
    let env = fixture_path(env_file);
    match std::process::Command::new(bin)
        .arg(subcommand)
        .arg("--config")
        .arg(config)
        .arg("--env-file")
        .arg(env)
        .output()
    {
        Ok(output) => output,
        Err(err) => panic!("failed to execute binary: {err}"),
    }
}

pub fn run_docker_fixture(fixture_dir: &str, subcommand: &str) -> Output {
    let dir = fixture_path(fixture_dir);
    let bin = binary_path();
    match std::process::Command::new(bin)
        .arg(subcommand)
        .arg("--config")
        .arg("compose.yaml")
        .current_dir(&dir)
        .output()
    {
        Ok(output) => output,
        Err(err) => panic!("failed to execute binary: {err}"),
    }
}

pub fn run_fixture_with_host_env(
    fixture: &str,
    subcommand: &str,
    env_vars: &[(&str, &str)],
) -> Output {
    let bin = binary_path();
    let config = fixture_path(fixture);
    let mut cmd = std::process::Command::new(bin);
    cmd.arg(subcommand).arg("--config").arg(config);
    for (key, value) in env_vars {
        cmd.env(key, value);
    }
    match cmd.output() {
        Ok(output) => output,
        Err(err) => panic!("failed to execute binary: {err}"),
    }
}

pub fn run_docker_fixture_with_host_env(
    fixture_dir: &str,
    subcommand: &str,
    env_vars: &[(&str, &str)],
) -> Output {
    let dir = fixture_path(fixture_dir);
    let bin = binary_path();
    let mut cmd = std::process::Command::new(bin);
    cmd.arg(subcommand)
        .arg("--config")
        .arg("compose.yaml")
        .current_dir(&dir);
    for (key, value) in env_vars {
        cmd.env(key, value);
    }
    match cmd.output() {
        Ok(output) => output,
        Err(err) => panic!("failed to execute binary: {err}"),
    }
}

pub fn run_shell_script(script: &str) -> Output {
    match std::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .output()
    {
        Ok(output) => output,
        Err(err) => panic!("failed to run shell script: {err}"),
    }
}

pub fn run_shell_script_in_dir(script: &str, dir: &std::path::Path) -> Output {
    match std::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .output()
    {
        Ok(output) => output,
        Err(err) => panic!("failed to run shell script: {err}"),
    }
}

pub fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

pub fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

pub fn assert_stdout_contains(output: &Output, needle: &str) {
    let stdout = stdout_of(output);
    assert!(
        stdout.contains(needle),
        "expected {needle:?} in stdout\nstdout: {stdout}\nstderr: {}",
        stderr_of(output)
    );
}

pub fn assert_stdout_order(output: &Output, items: &[&str]) {
    let stdout = stdout_of(output);
    let mut prev_pos = 0;
    let mut prev_item = "";
    for item in items {
        let Some(pos) = stdout.find(item) else {
            panic!("stdout-order: {item:?} not found in stdout\nstdout: {stdout}")
        };
        assert!(
            prev_item.is_empty() || pos > prev_pos,
            "stdout-order: {item:?} appeared before {prev_item:?}\nstdout: {stdout}"
        );
        prev_pos = pos;
        prev_item = item;
    }
}

pub fn assert_exit_code(output: &Output, expected: i32) {
    let stdout = stdout_of(output);
    assert_eq!(
        output.status.code(),
        Some(expected),
        "expected exit code {expected}\nstdout: {stdout}\nstderr: {}",
        stderr_of(output)
    );
}
