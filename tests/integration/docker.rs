use super::helpers::*;

#[test]
fn docker_build() {
    let output = run_docker_fixture("docker_build", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "building");
    assert_stdout_contains(&output, "built");
    assert_stdout_contains(&output, "build works");
}

#[test]
fn docker_cleanup() {
    let output = run_docker_fixture("docker_cleanup", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "creating network");
    assert_stdout_contains(&output, "removing containers");
    assert_stdout_contains(&output, "removing network");
}

#[test]
fn docker_crash_stderr() {
    let output = run_docker_fixture("docker_crash_stderr", "run");
    assert_exit_code(&output, 1);
    assert_stdout_contains(&output, "crashed");
    assert_stdout_contains(&output, "--- last stderr ---");
    assert_stdout_contains(&output, "FATAL: something broke");
}

#[test]
fn docker_deps_diamond() {
    let output = run_docker_fixture("docker_deps_diamond", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "db-ready");
    assert_stdout_contains(&output, "cache-ready");
    assert_stdout_contains(&output, "app-done");
    assert_stdout_order(&output, &["db-ready", "app-done"]);
    assert_stdout_order(&output, &["cache-ready", "app-done"]);
}

#[test]
fn docker_dns() {
    let output = run_docker_fixture("docker_dns", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "dns-resolved");
}

#[test]
fn docker_env() {
    let output = run_docker_fixture("docker_env", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "MY_VAR=hello-from-env");
    assert_stdout_contains(&output, "OTHER_VAR=42");
}

#[test]
fn docker_env_no_host_leak() {
    let output = run_docker_fixture("docker_env_no_host_leak", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(
        &output,
        "PATH=/custom/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    );
}

#[test]
fn docker_health_check() {
    let output = run_docker_fixture("docker_health_check", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "waiting for health check");
    assert_stdout_contains(&output, "healthy");
    assert_stdout_contains(&output, "consumer ready");
    assert_stdout_order(&output, &["healthy", "consumer ready"]);
}

#[test]
fn docker_health_check_exec() {
    let output = run_docker_fixture("docker_health_check_exec", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "waiting for health check");
    assert_stdout_contains(&output, "healthy");
    assert_stdout_contains(&output, "consumer ready");
    assert_stdout_order(&output, &["healthy", "consumer ready"]);
}

#[test]
fn docker_health_check_shell() {
    let output = run_docker_fixture("docker_health_check_shell", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "waiting for health check");
    assert_stdout_contains(&output, "healthy");
    assert_stdout_contains(&output, "consumer ready");
    assert_stdout_order(&output, &["healthy", "consumer ready"]);
}

fn run_background_with_sigint(
    fixture_dir: &str,
    subcommand: &str,
    wait_secs: u32,
) -> std::process::Output {
    let dir = fixture_path(fixture_dir);
    let bin = binary_path();
    let script = format!(
        "{bin} {subcommand} --config compose.yaml &\nPID=$!\nsleep {wait_secs}\nkill -INT $PID\nwait $PID\nexit 0",
        bin = bin.display(),
    );
    run_shell_script_in_dir(&script, &dir)
}

#[test]
fn docker_ports() {
    let dir = fixture_path("docker_ports");
    let bin = binary_path();
    let script = format!(
        "{bin} up --config compose.yaml &\nPID=$!\nsleep 5\ncurl -s http://localhost:18793/\nkill -INT $PID\nwait $PID\nexit 0",
        bin = bin.display(),
    );
    let output = run_shell_script_in_dir(&script, &dir);

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("port-works"),
        "expected port-works in stdout\nstdout: {stdout}"
    );
}

#[test]
fn docker_services() {
    let output = run_docker_fixture("docker_services", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "hello from svc");
    assert_stdout_contains(&output, "creating network");
    assert_stdout_contains(&output, "removing containers");
    assert_stdout_contains(&output, "removing network");
}

#[test]
fn docker_sigterm() {
    let output = run_background_with_sigint("docker_sigterm", "up", 5);
    let stdout = stdout_of(&output);

    assert!(
        stdout.contains("removing containers"),
        "expected 'removing containers' in stdout\nstdout: {stdout}"
    );
    assert!(
        stdout.contains("removing network"),
        "expected 'removing network' in stdout\nstdout: {stdout}"
    );
}

#[test]
fn docker_volumes() {
    let dir = fixture_path("docker_volumes");
    let data_dir = dir.join("data");
    match std::fs::create_dir_all(&data_dir) {
        Ok(()) => {}
        Err(err) => panic!("failed to create data dir: {err}"),
    }
    match std::fs::write(data_dir.join("file.txt"), "volume-content-here\n") {
        Ok(()) => {}
        Err(err) => panic!("failed to write test file: {err}"),
    }

    let volume_mount = format!("{}:/mnt/data", data_dir.display());
    let compose_content = format!(
        "processes:\n  vol-main:\n    docker:\n      build: ./app\n      volumes:\n        - \"{volume_mount}\"\n"
    );
    match std::fs::write(dir.join("compose.yaml"), compose_content) {
        Ok(()) => {}
        Err(err) => panic!("failed to write compose.yaml: {err}"),
    }

    let output = match std::process::Command::new(binary_path())
        .arg("run")
        .arg("--config")
        .arg("compose.yaml")
        .current_dir(&dir)
        .output()
    {
        Ok(output) => output,
        Err(err) => panic!("failed to execute binary: {err}"),
    };

    match std::fs::remove_dir_all(&data_dir) {
        Ok(()) => {}
        Err(err) => panic!("failed to clean up test data dir: {err}"),
    }

    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "volume-content-here");
}

#[test]
fn docker_dockerfile() {
    let output = run_docker_fixture("docker_dockerfile", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "custom-dockerfile-works");
}

#[test]
fn docker_command() {
    let output = run_docker_fixture("docker_command", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "hello from command");
}

#[test]
fn docker_exec() {
    let output = run_docker_fixture("docker_exec", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "hello from exec");
}

#[test]
fn docker_env_host_interpolation() {
    let output = run_docker_fixture_with_host_env(
        "docker_env_host_interpolation",
        "run",
        &[("DOCKER_HOST_TEST_VAR", "host-resolved")],
    );
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "RESULT=host-resolved");
}

#[test]
fn docker_env_empty_value() {
    let output = run_docker_fixture("docker_env_empty", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "EMPTY_VAR=[]");
    assert_stdout_contains(&output, "SET_VAR=[has-value]");
}

#[test]
fn docker_env_passthrough() {
    let output = run_docker_fixture_with_host_env(
        "docker_env_passthrough",
        "run",
        &[("MY_VAR", "passed-through")],
    );
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "MY_VAR=passed-through");
}

#[test]
fn docker_stop_timeout() {
    let output = run_docker_fixture("docker_stop_timeout", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "trigger-done");
    assert_stdout_contains(&output, "removing containers");
}

#[test]
fn docker_build_failure() {
    let output = run_docker_fixture("docker_build_failure", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("ERROR") || stdout.contains("error"),
        "expected error in stdout\nstdout: {stdout}\nstderr: {}",
        stderr_of(&output)
    );
}
