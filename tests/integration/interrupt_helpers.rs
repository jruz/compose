use super::helpers;

#[test]
fn startup_exit_reports_captured_logs() {
    let command = "exec sh -c 'echo startup-error; exit 42'";
    let script = helpers::build_interrupt_script(command, "false", "echo readiness-reached", 60);
    let started = std::time::Instant::now();
    let output = helpers::run_shell_script(&script);
    let elapsed = started.elapsed();
    let deadline = std::time::Duration::from_secs(10);

    helpers::assert_exit_code(&output, 1);
    helpers::assert_stdout_contains(&output, "startup-error");
    let stderr = helpers::stderr_of(&output);
    assert!(
        stderr.contains("compose exited before readiness"),
        "{stderr}"
    );
    assert!(elapsed < deadline, "early exit took {elapsed:?}");
}

#[test]
fn startup_success_without_readiness_is_failure() {
    let script =
        helpers::build_interrupt_script("exec true", "false", "echo readiness-reached", 60);
    let output = helpers::run_shell_script(&script);

    helpers::assert_exit_code(&output, 1);
    let stderr = helpers::stderr_of(&output);
    assert!(
        stderr.contains("compose exited before readiness"),
        "{stderr}"
    );
}

#[test]
fn startup_timeout_stops_process_and_removes_temporary_files() {
    let command = "COMPOSE_TEST_DIR=\"$COMPOSE_TEST_DIR\" exec sh -c 'echo test-dir=$COMPOSE_TEST_DIR; echo startup-stalled; exec sleep 30'";
    let script = helpers::build_interrupt_script(command, "false", "echo readiness-reached", 1);
    let started = std::time::Instant::now();
    let output = helpers::run_shell_script(&script);
    let elapsed = started.elapsed();
    let deadline = std::time::Duration::from_secs(10);

    helpers::assert_exit_code(&output, 124);
    helpers::assert_stdout_contains(&output, "startup-stalled");
    let stdout = helpers::stdout_of(&output);
    let stderr = helpers::stderr_of(&output);
    assert!(stderr.contains("compose startup timed out"), "{stderr}");
    assert!(!stdout.contains("readiness-reached"), "{stdout}");
    assert!(elapsed < deadline, "startup timeout took {elapsed:?}");
    assert_interrupt_artifacts_removed(&stdout);
}

fn assert_interrupt_artifacts_removed(stdout: &str) {
    let process_removed = stdout
        .lines()
        .filter_map(|line| line.strip_prefix("compose-pid="))
        .map(|pid| {
            let path = format!("/proc/{pid}");
            std::path::PathBuf::from(path)
        })
        .any(|path| !path.exists());
    assert!(
        process_removed,
        "missing PID or surviving process: {stdout}"
    );

    let directory_removed = stdout
        .lines()
        .filter_map(|line| line.strip_prefix("test-dir="))
        .map(std::path::Path::new)
        .any(|path| !path.exists());
    assert!(
        directory_removed,
        "missing temporary directory or surviving files: {stdout}"
    );
}
