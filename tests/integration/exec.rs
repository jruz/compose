use super::helpers::*;

#[test]
fn run_main_exits_zero() {
    let output = run_fixture("run_exit_zero.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "hello");
}

#[test]
fn run_main_exits_nonzero() {
    let output = run_fixture("run_exit_nonzero.yaml", "run");
    assert_exit_code(&output, 42);
}

#[test]
fn run_support_crashes() {
    let output = run_fixture("run_support_crashes.yaml", "run");
    assert_exit_code(&output, 1);
    assert_stdout_contains(&output, "crashed");
    assert_stdout_contains(&output, "killing (crasher crashed)");
}

#[test]
fn crash_stderr_summary() {
    let output = run_fixture("run_stderr_summary.yaml", "run");
    assert_exit_code(&output, 1);
    assert_stdout_contains(&output, "--- last stderr ---");
    assert_stdout_contains(&output, "FATAL: cannot continue");
}

#[test]
fn up_any_exit_triggers_shutdown() {
    let output = run_fixture("up_any_exit.yaml", "up");
    assert_exit_code(&output, 1);
    assert_stdout_contains(&output, "crashed");
    assert_stdout_contains(&output, "killing (short crashed)");
}

#[test]
fn up_main_exit() {
    let output = run_fixture("up_main_exit.yaml", "up");
    assert_exit_code(&output, 1);
    assert_stdout_contains(&output, "crashed");
}

#[test]
fn dependency_ordering() {
    let output = run_fixture("dependency_order.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_order(&output, &["db-started", "app-started"]);
}

#[test]
fn dependency_multi() {
    let output = run_fixture("dependency_multi.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "db-started");
    assert_stdout_contains(&output, "cache-started");
    assert_stdout_contains(&output, "app-started");
    assert_stdout_order(&output, &["db-started", "app-started"]);
    assert_stdout_order(&output, &["cache-started", "app-started"]);
}

#[test]
fn dependency_diamond() {
    let output = run_fixture("dependency_diamond.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_order(&output, &["infra-started", "svcA-started"]);
    assert_stdout_order(&output, &["infra-started", "svcB-started"]);
    assert_stdout_order(&output, &["svcA-started", "app-started"]);
    assert_stdout_order(&output, &["svcB-started", "app-started"]);
}

#[test]
fn dependency_deep_chain() {
    let output = run_fixture("dependency_deep_chain.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_order(&output, &["c-started", "b-started", "a-started"]);
}

#[test]
fn health_check_gates_dependent() {
    let output = run_fixture("health_check.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_order(&output, &["healthy", "consumer-started"]);
}

#[test]
fn health_check_exec() {
    let output = run_fixture("health_check_exec.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_order(&output, &["healthy", "consumer-started"]);
}

#[test]
fn health_check_timeout() {
    let output = run_fixture("health_check_timeout.yaml", "run");
    assert_exit_code(&output, 1);
    assert_stdout_contains(&output, "health check timed out");
}

#[test]
fn health_crash_during_check() {
    let output = run_fixture("health_crash_during_check.yaml", "run");
    assert_exit_code(&output, 1);
    assert_stdout_contains(&output, "exited before becoming healthy");
}

#[test]
fn interrupt_during_health_check_cleans_up_process() {
    let output = interrupt_fixture_after_process_starts("interrupt_health_check.yaml", "up");
    let service_pid = output
        .stdout
        .lines()
        .find(|line| line.starts_with("service-pid="))
        .and_then(|line| line.split_once("service-pid=").map(|(_, pid)| pid.trim()));
    let Some(service_pid) = service_pid else {
        panic!(
            "service PID missing from stdout: {}\nstderr: {}",
            output.stdout, output.stderr
        );
    };
    let service_pid = match service_pid.parse::<i32>() {
        Ok(pid) => pid,
        Err(err) => panic!("invalid service PID {service_pid:?}: {err}"),
    };
    let process = nix::unistd::Pid::from_raw(service_pid);
    let process_path = std::path::PathBuf::from(format!("/proc/{service_pid}"));
    let process_alive = process_path.exists();
    if process_alive
        && let Err(err) = nix::sys::signal::killpg(process, nix::sys::signal::Signal::SIGKILL)
    {
        panic!("failed to clean up leaked process: {err}");
    }

    let expected_code: i32 = 130;
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "stdout: {}\nstderr: {}",
        output.stdout,
        output.stderr
    );
    assert!(!process_alive, "startup service survived compose shutdown");
}

#[test]
fn exec_services_do_not_inherit_stdin() {
    let bin = binary_path();
    let fixture = fixture_path("exec_stdin.yaml");
    let script = format!(
        "printf 'synthetic-input\\n' | \"{bin}\" run --config \"{fixture}\"",
        bin = bin.display(),
        fixture = fixture.display(),
    );
    let output = run_shell_script(&script);
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "[service] stdin-closed");
}

#[test]
fn signal_shutdown_sigint_waits_for_direct_process() {
    assert_signal_shutdown("exec_signal_shutdown.yaml", "INT");
}

#[test]
fn signal_shutdown_sigterm_waits_for_direct_process() {
    assert_signal_shutdown("exec_signal_shutdown.yaml", "TERM");
}

#[test]
fn signal_shutdown_sigint_waits_for_descendant() {
    assert_signal_shutdown("exec_descendant_shutdown.yaml", "INT");
}

#[test]
fn signal_shutdown_sigterm_waits_for_descendant() {
    assert_signal_shutdown("exec_descendant_shutdown.yaml", "TERM");
}

fn assert_signal_shutdown(fixture: &str, signal: &str) {
    let output = signal_shutdown_fixture(fixture, signal);
    let expected_code: i32 = 130;
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "stdout: {}\nstderr: {}",
        output.stdout,
        output.stderr
    );
    assert!(
        output.stdout.contains("marker-at-compose-exit=present"),
        "compose exited before TERM cleanup finished\nstdout: {}\nstderr: {}",
        output.stdout,
        output.stderr
    );
}

#[test]
fn shutdown_completion_on_main_exit() {
    assert_shutdown_completion(
        "run",
        "  main:\n    depends_on: [wrapper]\n    command: exit 42\n",
        42,
    );
}

#[test]
fn shutdown_completion_on_sibling_crash() {
    assert_shutdown_completion(
        "run",
        "  crasher:\n    depends_on: [wrapper]\n    command: exit 7\n  main:\n    depends_on: [crasher]\n    command: exec sleep 30\n",
        1,
    );
}

#[test]
fn shutdown_completion_on_oneshot_failure() {
    assert_shutdown_completion(
        "up",
        "  task:\n    depends_on: [wrapper]\n    type: oneshot\n    command: exit 7\n",
        1,
    );
}

#[test]
fn shutdown_completion_on_health_failure() {
    assert_shutdown_completion(
        "up",
        "  unhealthy:\n    depends_on: [wrapper]\n    command: exec sleep 30\n    health:\n      command: 'false'\n      interval_secs: 1\n      timeout_secs: 1\n",
        1,
    );
}

#[test]
fn shutdown_completion_on_startup_error() {
    assert_shutdown_completion(
        "up",
        "  broken:\n    depends_on: [wrapper]\n    command: exec sleep 30\n    env_file: /compose-test-missing-env-file\n",
        1,
    );
}

#[test]
fn signal_shutdown_during_startup_waits_for_descendant() {
    for signal in ["INT", "TERM"] {
        let action = format!("kill -{signal} \"$COMPOSE_PID\"");
        let output = shutdown_fixture(
            "exec_descendant_shutdown.yaml",
            "up",
            &action,
            "    health:\n      command: 'false'\n      interval_secs: 1\n      timeout_secs: 30\n",
        );
        assert_shutdown_marker(&output, 130);
    }
}

#[test]
fn signal_shutdown_escalates_term_ignoring_descendants() {
    let output = escalation_shutdown_fixture();
    let expected_code: i32 = 130;
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "stdout: {}\nstderr: {}",
        output.stdout,
        output.stderr
    );
    assert!(output.stdout.contains("descendant-stopped"));
}

#[test]
fn shutdown_completion_after_wrapper_exits_naturally() {
    let output = shutdown_fixture("exec_orphan_shutdown.yaml", "run", ":", "");
    assert_shutdown_marker(&output, 23);
}

fn assert_shutdown_completion(subcommand: &str, trigger: &str, expected_code: i32) {
    let extra = format!(
        "    health:\n      command: 'test -s \"$COMPOSE_TEST_DIR/ready\"'\n      interval_secs: 1\n      timeout_secs: 10\n{trigger}"
    );
    let output = shutdown_fixture("exec_descendant_shutdown.yaml", subcommand, ":", &extra);
    assert_shutdown_marker(&output, expected_code);
}

fn assert_shutdown_marker(output: &InterruptedOutput, expected_code: i32) {
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "stdout: {}\nstderr: {}",
        output.stdout,
        output.stderr
    );
    assert!(
        output.stdout.contains("marker-at-compose-exit=present"),
        "cleanup incomplete at compose exit\nstdout: {}\nstderr: {}",
        output.stdout,
        output.stderr
    );
}

#[test]
fn graceful_shutdown() {
    let output = run_fixture("graceful_shutdown.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "trapper-started");
    assert_stdout_contains(&output, "[trapper] sigterm-received");
}

#[test]
fn health_check_retry_then_healthy() {
    let output = run_fixture_with_temporary_marker("health_check_retry.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "service-started");
    assert_stdout_contains(&output, "healthy");
    assert_stdout_contains(&output, "consumer-started");
    assert_stdout_order(&output, &["service-started", "healthy", "consumer-started"]);
}

#[test]
fn health_timeout_kills_process() {
    let output = run_fixture("health_timeout_kills_process.yaml", "run");
    assert_exit_code(&output, 1);
    assert_stdout_contains(&output, "health check timed out");
    assert_stdout_contains(&output, "[service] sigterm-on-timeout");
}

#[test]
fn health_failure_shuts_down_siblings() {
    let bin = binary_path();
    let fixture = fixture_path("health_crash_sibling.yaml");
    let script = format!(
        "{bin} run --config {fixture} &\nPID=$!\n( sleep 10 && kill -9 $PID ) >/dev/null 2>&1 &\nTIMER=$!\nwait $PID 2>/dev/null\nSTATUS=$?\nkill -9 $TIMER 2>/dev/null\nif [ $STATUS -eq 137 ]; then exit 124; fi\nexit $STATUS",
        bin = bin.display(),
        fixture = fixture.display(),
    );
    let output = run_shell_script(&script);

    let stdout = stdout_of(&output);
    assert_ne!(
        output.status.code(),
        Some(124_i32),
        "BUG: compose hung — health failure did not trigger shutdown within 10s\nstdout: {stdout}"
    );
    assert!(
        !output.status.success(),
        "expected non-zero exit\nstdout: {stdout}"
    );
}

#[test]
fn stderr_overflow_shows_last_20() {
    let output = run_fixture("run_stderr_overflow.yaml", "run");
    assert_exit_code(&output, 1);
    assert_stdout_contains(&output, "--- last stderr ---");
    assert_stdout_contains(&output, "stderr-line-30");
    assert_stdout_contains(&output, "stderr-line-11");
    let stdout = stdout_of(&output);
    let summary_section = stdout
        .split_once("--- last stderr ---")
        .expect("no summary")
        .1;
    assert!(
        !summary_section.contains("stderr-line-1\n"),
        "stderr-line-1 should have been truncated from the 20-line buffer\nstdout: {stdout}"
    );
}

#[test]
fn main_nonzero_no_stderr_summary() {
    let output = run_fixture("run_main_nonzero_stderr.yaml", "run");
    assert_exit_code(&output, 1);
    assert_stdout_contains(&output, "FATAL: main process error");
    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("--- last stderr ---"),
        "main process exit path should not show stderr summary\nstdout: {stdout}"
    );
}

#[test]
fn sigterm_ignored_gets_sigkill() {
    let output = run_fixture("run_sigterm_ignored.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "stubborn-started");
    assert_stdout_contains(&output, "main-done");
}

#[test]
fn service_with_child_then_oneshot() {
    let bin = binary_path();
    let fixture = fixture_path("service_with_child_then_oneshot.yaml");
    let script = format!(
        "{bin} run --config {fixture} &\nPID=$!\n( sleep 10 && kill -9 $PID ) >/dev/null 2>&1 &\nTIMER=$!\nwait $PID 2>/dev/null\nSTATUS=$?\nkill -9 $TIMER 2>/dev/null\nif [ $STATUS -eq 137 ]; then exit 124; fi\nexit $STATUS",
        bin = bin.display(),
        fixture = fixture.display(),
    );
    let output = run_shell_script(&script);
    let stdout = stdout_of(&output);
    assert_ne!(
        output.status.code(),
        Some(124_i32),
        "BUG: compose hung — child processes not cleaned up within 10s\nstdout: {stdout}"
    );
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "db-ready");
    assert_stdout_contains(&output, "main-done");
}

#[test]
fn process_group_cleanup() {
    let bin = binary_path();
    let fixture = fixture_path("process_group_cleanup.yaml");
    let script = format!(
        "{bin} run --config {fixture} &\nPID=$!\n( sleep 10 && kill -9 $PID ) >/dev/null 2>&1 &\nTIMER=$!\nwait $PID 2>/dev/null\nSTATUS=$?\nkill -9 $TIMER 2>/dev/null\nif [ $STATUS -eq 137 ]; then exit 124; fi\nexit $STATUS",
        bin = bin.display(),
        fixture = fixture.display(),
    );
    let output = run_shell_script(&script);
    let stdout = stdout_of(&output);
    assert_ne!(
        output.status.code(),
        Some(124_i32),
        "BUG: compose hung — child processes not cleaned up within 10s\nstdout: {stdout}"
    );
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "bg-started");
    assert_stdout_contains(&output, "main-done");
}

#[test]
fn shell_bash() {
    let output = run_fixture("shell_bash.yaml", "run");
    assert_exit_code(&output, 0);
    let stdout = stdout_of(&output);
    let version_line = stdout
        .lines()
        .find(|line| line.contains("[main]") && !line.contains("starting"))
        .and_then(|line| line.split("[main]").nth(1))
        .map(str::trim)
        .expect("expected version output from bash");
    assert!(
        version_line
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_digit()),
        "expected bash version digit, got: {version_line}"
    );
}
