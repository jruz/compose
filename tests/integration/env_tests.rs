use super::helpers::*;

#[test]
fn env_inline() {
    let output = run_fixture("env_inline.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "hello-from-env");
}

#[test]
fn env_file() {
    let output = run_with_env_file("env_file.yaml", "run", "test.env");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "loaded-from-env-file");
}

#[test]
fn env_file_override() {
    let output = run_with_env_file("env_file_override.yaml", "run", "test.env");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "inline-wins");
}

#[test]
fn env_file_process_substitution() {
    let bin = binary_path().display().to_string();
    let config = fixture_path("env_file.yaml").display().to_string();
    let env_file = fixture_path("test.env").display().to_string();
    let script = format!("{bin} run --config {config} --env-file <(cat {env_file})");
    let output = run_shell_script(&format!("bash -c '{script}'"));
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "loaded-from-env-file");
}

#[test]
fn env_interpolation() {
    let output = run_fixture("env_interpolation.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "postgres://admin:secret@db:5432/mydb");
}

#[test]
fn env_chained_interpolation() {
    let output = run_fixture("env_chained_interpolation.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "A=value");
    assert_stdout_contains(&output, "B=value");
    assert_stdout_contains(&output, "C=value");
}

#[test]
fn env_host_interpolation() {
    let output = run_fixture_with_host_env(
        "env_host_interpolation.yaml",
        "run",
        &[("HOST_TEST_VAR", "from-host")],
    );
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "RESULT=from-host");
}

#[test]
fn env_passthrough() {
    let output = run_fixture_with_host_env(
        "env_passthrough.yaml",
        "run",
        &[("MY_VAR", "passed-through")],
    );
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "MY_VAR=passed-through");
}

#[test]
fn env_undefined_var_stays_literal() {
    let output = run_fixture("env_undefined_var.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "${NONEXISTENT}");
}

#[test]
fn env_empty_value_interpolation() {
    let output = run_fixture("env_empty_value.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "OTHER=prefix--suffix");
}

#[test]
fn env_mixed_host_and_compose_vars() {
    let output = run_fixture_with_host_env(
        "env_mixed_host_compose.yaml",
        "run",
        &[("HOST_MIX_VAR", "host-val")],
    );
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "RESULT=compose-val:host-val");
}

#[test]
fn env_self_ref_resolves_from_host() {
    let output = run_fixture_with_host_env("env_self_ref.yaml", "run", &[("A", "host-value")]);
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "A=prefix-host-value");
}

#[test]
fn env_circular_interpolation() {
    let output = run_fixture("env_circular.yaml", "run");
    assert_ne!(output.status.code(), Some(0_i32));
    assert_stdout_contains(&output, "circular variable interpolation");
}

#[test]
fn env_dollar_without_braces_stays_literal() {
    let output = run_fixture("env_dollar_no_braces.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "RESULT=$FOO is literal");
}

#[test]
fn env_unclosed_brace_heals() {
    let output = run_fixture("env_unclosed_brace.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "RESULT=before-${FOO}");
}

#[test]
fn env_empty_ref_stays_literal() {
    let output = run_fixture("env_empty_ref.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "RESULT=before-${}-after");
}

#[test]
fn env_file_with_equals_in_value() {
    let output = run_fixture("env_file_equals.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(
        &output,
        "CONNECTION=postgres://user:pass=word@host:5432/db=name",
    );
}

#[test]
fn env_file_skips_empty_and_whitespace_lines() {
    let output = run_fixture("env_file_whitespace.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "FIRST=one");
    assert_stdout_contains(&output, "SECOND=two");
}

#[test]
fn env_file_duplicate_keys_last_wins() {
    let output = run_fixture("env_file_duplicate.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "KEY=second");
}

#[test]
fn env_file_skips_comment_lines() {
    let output = run_fixture("env_file_comments.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "KEY=value");
    assert_stdout_contains(&output, "OTHER=data");
}

#[test]
fn env_precedence_chain() {
    let output = run_with_env_file("env_precedence.yaml", "run", "env_precedence_global.env");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "SHARED=from-inline");
    assert_stdout_contains(&output, "GLOBAL_ONLY=global-val");
    assert_stdout_contains(&output, "PROC_ONLY=proc-val");
    assert_stdout_contains(&output, "INLINE_ONLY=inline-val");
}

#[test]
fn health_command_interpolation() {
    let output = run_fixture_with_temporary_marker("health_command_interpolation.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "healthy");
    assert_stdout_contains(&output, "consumer-started");
}

#[test]
fn health_exec_interpolation() {
    let output = run_fixture("health_exec_interpolation.yaml", "run");
    assert_exit_code(&output, 0);
    assert_stdout_contains(&output, "healthy");
    assert_stdout_contains(&output, "consumer-started");
}
