use super::helpers::*;

#[test]
fn docker_build() {
    let output = run_docker_fixture("docker_build", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "[docker] creating network");
    assert_stdout_contains(&output, "building");
    assert_stdout_contains(&output, "built");
    assert_stdout_contains(&output, "build works");
}

#[test]
fn docker_build_args() {
    let output = run_docker_fixture("docker_build_args", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "baked: hello-from-build-arg");
}

#[test]
fn docker_cleanup() {
    let output = run_docker_fixture("docker_cleanup", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "[docker] creating network");
    assert_stdout_contains(&output, "[docker] removing containers");
    assert_stdout_contains(&output, "[docker] removing network");
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

#[test]
fn docker_ports() -> Result<(), super::helpers::Error> {
    let output = run_docker_http_fixture("docker_ports", 18793, &[])?;
    check_http_response(&output, "port-works")
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
fn interrupt_after_docker_becomes_healthy_cleans_up_resources() {
    let fixture = "docker_sigterm";
    let process_name = "sig-server";
    let output = interrupt_docker_fixture_after_healthy(fixture, "up", process_name);
    assert_docker_shutdown_cleans_up_resources(&output, fixture, process_name);

    let healthy = output.stdout.find("[sig-server] healthy");
    let killing = output.stdout.find("[sig-server] killing");
    let healthy_before_shutdown = healthy
        .zip(killing)
        .is_some_and(|(ready, stopped)| ready < stopped);
    assert!(
        healthy_before_shutdown,
        "expected health before shutdown\nstdout: {}\nstderr: {}",
        output.stdout, output.stderr
    );
    assert!(
        output.stdout.contains("removing containers"),
        "expected 'removing containers' in stdout\nstdout: {}",
        output.stdout
    );
    assert!(
        output.stdout.contains("removing network"),
        "expected 'removing network' in stdout\nstdout: {}",
        output.stdout
    );
}

#[test]
fn interrupt_during_docker_health_check_cleans_up_resources() {
    let fixture = "docker_interrupt_health";
    let process_name = "startup-service";
    let output = interrupt_docker_fixture_after_container_starts(fixture, "up", process_name);
    assert_docker_shutdown_cleans_up_resources(&output, fixture, process_name);
}

fn assert_docker_shutdown_cleans_up_resources(
    output: &InterruptedOutput,
    fixture: &str,
    process_name: &str,
) {
    let network = format!("{fixture}-{}", output.pid);
    let container = format!("{network}-{process_name}");
    let container_names =
        docker_resource_names(&["container", "ls", "--all", "--format", "{{.Names}}"]);
    let network_names = docker_resource_names(&["network", "ls", "--format", "{{.Name}}"]);
    let container_exists = container_names.lines().any(|name| name == container);
    let network_exists = network_names.lines().any(|name| name == network);
    if container_exists {
        docker_resource_cleanup(&["rm", "--force", &container]);
    }
    if network_exists {
        docker_resource_cleanup(&["network", "rm", &network]);
    }

    let expected_code: i32 = 130;
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "stdout: {}\nstderr: {}",
        output.stdout,
        output.stderr
    );
    assert!(!container_exists, "container survived compose shutdown");
    assert!(!network_exists, "network survived compose shutdown");
}

fn docker_resource_names(args: &[&str]) -> String {
    let mut command = std::process::Command::new("docker");
    command.args(args);
    let output = match command.output() {
        Ok(output) => output,
        Err(err) => panic!("failed to inspect Docker resource: {err}"),
    };
    assert_exit_code(&output, 0);
    stdout_of(&output)
}

fn docker_resource_cleanup(args: &[&str]) {
    let mut command = std::process::Command::new("docker");
    command.args(args);
    let output = match command.output() {
        Ok(output) => output,
        Err(err) => panic!("failed to clean up Docker resource: {err}"),
    };
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "failed to clean up Docker resource: {stderr}"
    );
}

fn docker_volume_create(name: &str) -> String {
    let out = match std::process::Command::new("docker")
        .args(["volume", "create", name])
        .output()
    {
        Ok(out) => out,
        Err(err) => panic!("failed to create docker volume: {err}"),
    };
    assert!(out.status.success(), "docker volume create failed");
    name.to_string()
}

fn docker_volume_populate(vol: &str, file: &str, content: &str) {
    let script = format!("echo -n '{content}' > /mnt/{file}");
    let out = match std::process::Command::new("docker")
        .args([
            "run",
            "--rm",
            "-v",
            &format!("{vol}:/mnt"),
            "alpine:3.21",
            "sh",
            "-c",
            &script,
        ])
        .output()
    {
        Ok(out) => out,
        Err(err) => panic!("failed to populate docker volume: {err}"),
    };
    assert!(out.status.success(), "docker volume populate failed");
}

fn docker_volume_rm(vol: &str) {
    let out = match std::process::Command::new("docker")
        .args(["volume", "rm", "--force", vol])
        .output()
    {
        Ok(out) => out,
        Err(err) => panic!("failed to remove docker volume: {err}"),
    };
    assert!(out.status.success(), "docker volume rm failed");
}

#[test]
fn docker_volumes() {
    let vol = docker_volume_create("compose-test-vol-main");
    docker_volume_populate(&vol, "file.txt", "volume-content-here\n");

    let dir = fixture_path("docker_volumes");
    let compose_content = format!(
        "processes:\n  vol-main:\n    docker:\n      image: compose-test-vol-main\n      build: ./app\n      volumes:\n        - \"{vol}:/mnt/data\"\n"
    );
    let compose_path = dir.join("compose.yaml");
    match std::fs::write(compose_path, compose_content) {
        Ok(()) => {}
        Err(err) => panic!("failed to write compose.yaml: {err}"),
    }

    let bin = binary_path();
    let output = match std::process::Command::new(bin)
        .arg("run")
        .arg("--config")
        .arg("compose.yaml")
        .current_dir(&dir)
        .output()
    {
        Ok(output) => output,
        Err(err) => panic!("failed to execute binary: {err}"),
    };

    docker_volume_rm(&vol);

    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "volume-content-here");
}

#[test]
fn docker_volume_expansion_from_host_env() {
    let vol = docker_volume_create("compose-test-vol-host");
    docker_volume_populate(&vol, "file.txt", "expanded-host-var\n");

    let dir = fixture_path("docker_volume_expansion");
    let compose_content = "processes:\n  vol-host:\n    docker:\n      image: compose-test-vol-host\n      build: ./app\n      volumes:\n        - \"${VOLUME_HOST_DIR}:/mnt/data\"\n";
    let compose_path = dir.join("compose_host.yaml");
    match std::fs::write(compose_path, compose_content) {
        Ok(()) => {}
        Err(err) => panic!("failed to write compose yaml: {err}"),
    }

    let output = {
        let bin = binary_path();
        let mut cmd = std::process::Command::new(bin);
        cmd.arg("run")
            .arg("--config")
            .arg("compose_host.yaml")
            .env("VOLUME_HOST_DIR", &vol)
            .current_dir(&dir);
        match cmd.output() {
            Ok(output) => output,
            Err(err) => panic!("failed to execute binary: {err}"),
        }
    };

    let host_cleanup = dir.join("compose_host.yaml");
    match std::fs::remove_file(host_cleanup) {
        Ok(()) => {}
        Err(err) => panic!("failed to clean up compose yaml: {err}"),
    }
    docker_volume_rm(&vol);

    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "expanded-host-var");
}

#[test]
fn docker_volume_expansion_from_process_env() {
    let vol = docker_volume_create("compose-test-vol-proc");
    docker_volume_populate(&vol, "file.txt", "expanded-process-var\n");

    let dir = fixture_path("docker_volume_expansion");
    let compose_content = format!(
        "processes:\n  vol-proc:\n    docker:\n      image: compose-test-vol-proc\n      build: ./app\n      volumes:\n        - \"${{DATA_PATH}}:/mnt/data\"\n    env:\n      DATA_PATH: \"{vol}\"\n"
    );
    let compose_path = dir.join("compose_proc.yaml");
    match std::fs::write(compose_path, compose_content) {
        Ok(()) => {}
        Err(err) => panic!("failed to write compose yaml: {err}"),
    }

    let bin = binary_path();
    let output = match std::process::Command::new(bin)
        .arg("run")
        .arg("--config")
        .arg("compose_proc.yaml")
        .current_dir(&dir)
        .output()
    {
        Ok(output) => output,
        Err(err) => panic!("failed to execute binary: {err}"),
    };

    let proc_cleanup = dir.join("compose_proc.yaml");
    match std::fs::remove_file(proc_cleanup) {
        Ok(()) => {}
        Err(err) => panic!("failed to clean up compose yaml: {err}"),
    }
    docker_volume_rm(&vol);

    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "expanded-process-var");
}

#[test]
fn docker_volume_expansion_undefined_stays_literal() {
    let dir = fixture_path("docker_volume_expansion");
    let compose_content = "processes:\n  vol-undef:\n    docker:\n      image: compose-test-vol-undef\n      build: ./app\n      volumes:\n        - \"${UNDEFINED_VAR_XYZZY}:/mnt/data\"\n";
    let compose_path = dir.join("compose_undef.yaml");
    match std::fs::write(compose_path, compose_content) {
        Ok(()) => {}
        Err(err) => panic!("failed to write compose yaml: {err}"),
    }

    let bin = binary_path();
    let output = match std::process::Command::new(bin)
        .arg("run")
        .arg("--config")
        .arg("compose_undef.yaml")
        .current_dir(&dir)
        .output()
    {
        Ok(output) => output,
        Err(err) => panic!("failed to execute binary: {err}"),
    };

    let undef_cleanup = dir.join("compose_undef.yaml");
    match std::fs::remove_file(undef_cleanup) {
        Ok(()) => {}
        Err(err) => panic!("failed to clean up compose yaml: {err}"),
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("${UNDEFINED_VAR_XYZZY}"),
        "undefined var should stay literal, got: {combined}"
    );
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
fn docker_ports_interpolation() -> Result<(), super::helpers::Error> {
    let output = run_docker_http_fixture(
        "docker_ports_interpolation",
        18794,
        &[("HOST_PORT", "18794")],
    )?;
    check_http_response(&output, "port-interpolation-works")
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
