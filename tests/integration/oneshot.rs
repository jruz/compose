use super::helpers::*;

#[test]
fn oneshot_run() {
    let output = run_fixture("oneshot_run.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "waiting for completion");
    assert_stdout_contains(&output, "completed");
    assert_stdout_order(&output, &["completed", "main-started"]);
}

#[test]
fn oneshot_chain() {
    let output = run_fixture("oneshot_chain.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "reset] completed");
    assert_stdout_contains(&output, "seed] completed");
    assert_stdout_order(
        &output,
        &["reset] completed", "seed] completed", "server-started"],
    );
}

#[test]
fn oneshot_depends_healthy() {
    let output = run_fixture("oneshot_depends_healthy.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "healthy");
    assert_stdout_contains(&output, "init] completed");
    assert_stdout_order(&output, &["healthy", "completed", "main-started"]);
}

#[test]
fn oneshot_env() {
    let output = run_fixture("oneshot_env.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "HELLO=world");
}

#[test]
fn oneshot_explicit_service() {
    let output = run_fixture("oneshot_explicit_service.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "main-done");
}

#[test]
fn oneshot_diamond() {
    let output = run_fixture("oneshot_diamond.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "base] completed");
    assert_stdout_contains(&output, "left] completed");
    assert_stdout_contains(&output, "right] completed");
    assert_stdout_order(
        &output,
        &["base] completed", "left] completed", "main-started"],
    );
    assert_stdout_order(
        &output,
        &["base] completed", "right] completed", "main-started"],
    );
}

#[test]
fn oneshot_up_mode() {
    let output = run_fixture("oneshot_up_mode.yaml", "up");
    assert_exit_code(&output, 1);
    assert_stdout_contains(&output, "init] completed");
    assert_stdout_contains(&output, "server-started");
    assert_stdout_contains(&output, "crashed");
    assert_stdout_order(&output, &["completed", "server-started", "crashed"]);
}

#[test]
fn oneshot_fail() {
    let output = run_fixture("oneshot_fail.yaml", "run");
    assert_exit_code(&output, 1);
    assert_stdout_contains(&output, "exited with code 1");
}

#[test]
fn oneshot_fail_no_stderr() {
    let output = run_fixture("oneshot_fail_no_stderr.yaml", "run");
    assert_exit_code(&output, 1);
    assert_stdout_contains(&output, "exited with code 1");
    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("--- last stderr ---"),
        "no stderr summary expected when process has no stderr\nstdout: {stdout}"
    );
}
