use super::helpers::*;

#[test]
fn unknown_dependency() {
    let output = run_fixture("unknown_dependency.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "unknown");
}

#[test]
fn circular_dependency() {
    let output = run_fixture("circular_dependency.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "circular");
}

#[test]
fn no_processes() {
    let output = run_fixture("no_processes.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "no processes");
}

#[test]
fn health_empty() {
    let output = run_fixture("validation_health_empty.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "must specify command or exec");
}

#[test]
fn health_both() {
    let output = run_fixture("validation_health_both.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "must specify either command or exec, not both");
}

#[test]
fn unknown_type() {
    let output = run_fixture("validation_unknown_type.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "unknown variant");
}

#[test]
fn docker_no_image() {
    let output = run_fixture("validation_docker_no_image.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "requires image or build");
}

#[test]
fn docker_both_command_exec() {
    let output = run_fixture("validation_docker_both_command_exec.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "cannot have both command and exec");
}

#[test]
fn exec_no_command() {
    let output = run_fixture("validation_exec_no_command.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "exec driver requires command");
}

#[test]
fn oneshot_health() {
    let output = run_fixture("validation_oneshot_health.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "oneshot processes cannot have health checks");
}

#[test]
fn missing_file() {
    let bin = binary_path();
    let output = match std::process::Command::new(bin)
        .arg("run")
        .arg("--config")
        .arg("/tmp/nonexistent_compose_file_12345.yaml")
        .output()
    {
        Ok(output) => output,
        Err(err) => panic!("failed to execute binary: {err}"),
    };
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "ERROR");
}

#[test]
fn health_empty_command_string() {
    let output = run_fixture("validation_health_empty_command.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "must specify command or exec");
}

#[test]
fn health_empty_exec_array() {
    let output = run_fixture("validation_health_empty_exec.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "must specify command or exec");
}

#[test]
fn health_zero_intervals_accepted() {
    let output = run_fixture("validation_health_zero_intervals.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "healthy");
}

#[test]
fn env_file_line_without_equals() {
    let output = run_fixture("validation_env_file_no_equals.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "malformed env file");
}

#[test]
fn env_file_nonexistent_path() {
    let output = run_fixture("validation_env_file_nonexistent.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "failed to read");
}

#[test]
fn global_env_file_nonexistent() {
    let output = run_with_env_file(
        "run_exit_zero.yaml",
        "run",
        "/tmp/nonexistent_global_env_98765.env",
    );
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "failed to read");
}

#[test]
fn unknown_yaml_field_rejected() {
    let output = run_fixture("validation_unknown_field.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "unknown field");
}

#[test]
fn last_process_oneshot() {
    let output = run_fixture("validation_last_oneshot.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "oneshot");
    assert_stdout_contains(&output, "run mode requires a service");
}

#[test]
fn nonexistent_shell_binary() {
    let output = run_fixture("shell_nonexistent.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "failed to spawn");
}
