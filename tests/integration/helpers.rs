use std::path::PathBuf;
use std::process::Output;

pub struct InterruptedOutput {
    pub pid: i32,
    pub status: std::process::ExitStatus,
    pub stdout: String,
}

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

pub fn run_fixture_with_temporary_marker(fixture: &str, subcommand: &str) -> Output {
    let bin = binary_path();
    let config = fixture_path(fixture);
    let script = format!(
        "set -eu\nCOMPOSE_TEST_DIR=$(mktemp -d)\ntrap 'rm -r \"$COMPOSE_TEST_DIR\"' EXIT\nCOMPOSE_TEST_MARKER=\"$COMPOSE_TEST_DIR/ready\" {bin} {subcommand} --config {config}",
        bin = bin.display(),
        config = config.display(),
    );
    run_shell_script(&script)
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

pub fn interrupt_docker_fixture_after_container_starts(
    fixture_dir: &str,
    subcommand: &str,
    process_name: &str,
) -> InterruptedOutput {
    let dir = fixture_path(fixture_dir);
    let config = dir.join("compose.yaml");
    let bin = binary_path();
    let script = format!(
        "COMPOSE_CAPTURE=$(mktemp)\n{bin} {subcommand} --config {config} >\"$COMPOSE_CAPTURE\" 2>&1 &\nCOMPOSE_PID=$!\necho compose-pid=$COMPOSE_PID\nCOMPOSE_CONTAINER={fixture_dir}-$COMPOSE_PID-{process_name}\nuntil docker container inspect \"$COMPOSE_CONTAINER\" >/dev/null 2>&1; do sleep 0.01; done\nkill -INT $COMPOSE_PID\nwait $COMPOSE_PID\nCOMPOSE_STATUS=$?\ncat \"$COMPOSE_CAPTURE\"\nrm \"$COMPOSE_CAPTURE\"\nexit $COMPOSE_STATUS",
        bin = bin.display(),
        config = config.display(),
    );
    let dir_path = dir.as_path();
    let current_dir = Some(dir_path);
    run_interrupt_script(&script, current_dir)
}

pub fn interrupt_fixture_after_process_starts(
    fixture: &str,
    subcommand: &str,
) -> InterruptedOutput {
    let config = fixture_path(fixture);
    let bin = binary_path();
    let script = format!(
        "COMPOSE_CAPTURE=$(mktemp)\nCOMPOSE_MARKER=$(mktemp)\nrm \"$COMPOSE_MARKER\"\nCOMPOSE_TEST_MARKER=\"$COMPOSE_MARKER\" {bin} {subcommand} --config {config} >\"$COMPOSE_CAPTURE\" 2>&1 &\nCOMPOSE_PID=$!\necho compose-pid=$COMPOSE_PID\nuntil test -s \"$COMPOSE_MARKER\"; do sleep 0.01; done\nCOMPOSE_SERVICE_PID=$(cat \"$COMPOSE_MARKER\")\necho service-pid=$COMPOSE_SERVICE_PID\nkill -INT $COMPOSE_PID\nwait $COMPOSE_PID\nCOMPOSE_STATUS=$?\ncat \"$COMPOSE_CAPTURE\"\nrm \"$COMPOSE_CAPTURE\"\nrm \"$COMPOSE_MARKER\"\nexit $COMPOSE_STATUS",
        bin = bin.display(),
        config = config.display(),
    );
    run_interrupt_script(&script, None)
}

fn run_interrupt_script(script: &str, current_dir: Option<&std::path::Path>) -> InterruptedOutput {
    let mut command = std::process::Command::new("sh");
    command.arg("-c").arg(script);
    if let Some(dir) = current_dir {
        command.current_dir(dir);
    }
    let output = match command.output() {
        Ok(output) => output,
        Err(err) => panic!("failed to execute binary: {err}"),
    };
    let stdout_bytes = output.stdout.as_slice();
    let stdout = String::from_utf8_lossy(stdout_bytes).to_string();
    let pid = stdout
        .lines()
        .find(|line| line.starts_with("compose-pid="))
        .and_then(|line| line.split_once('=').map(|(_, pid)| pid));
    let Some(pid) = pid else {
        panic!("compose PID missing from stdout: {stdout}");
    };
    let pid = match pid.parse::<i32>() {
        Ok(pid) => pid,
        Err(err) => panic!("invalid compose PID {pid:?}: {err}"),
    };
    InterruptedOutput {
        pid,
        status: output.status,
        stdout,
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
