#[cfg(test)]
mod helpers {
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
        match std::process::Command::new(binary_path())
            .arg(subcommand)
            .arg("--config")
            .arg(fixture_path(fixture))
            .output()
        {
            Ok(output) => output,
            Err(err) => panic!("failed to execute binary: {err}"),
        }
    }

    pub fn run_with_env_file(fixture: &str, subcommand: &str, env_file: &str) -> Output {
        match std::process::Command::new(binary_path())
            .arg(subcommand)
            .arg("--config")
            .arg(fixture_path(fixture))
            .arg("--env-file")
            .arg(fixture_path(env_file))
            .output()
        {
            Ok(output) => output,
            Err(err) => panic!("failed to execute binary: {err}"),
        }
    }

    pub fn run_docker_fixture(fixture_dir: &str, subcommand: &str) -> Output {
        let dir = fixture_path(fixture_dir);
        match std::process::Command::new(binary_path())
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
        let mut cmd = std::process::Command::new(binary_path());
        cmd.arg(subcommand)
            .arg("--config")
            .arg(fixture_path(fixture));
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
}

#[cfg(test)]
mod cli {
    use super::helpers::*;

    #[test]
    fn version_flag() {
        let output = match std::process::Command::new(binary_path())
            .arg("--version")
            .output()
        {
            Ok(output) => output,
            Err(err) => panic!("failed to execute binary: {err}"),
        };
        assert_eq!(output.status.code(), Some(0_i32));
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        let Some(version_line) = stdout.lines().find(|line| line.contains("version: ")) else {
            panic!("missing version line in stdout: {stdout}")
        };
        let Some(version) = version_line.split("version: ").nth(1) else {
            panic!("missing version value in line: {version_line}")
        };
        assert!(!version.trim().is_empty(), "version should not be empty");

        let Some(build_time_line) = stdout.lines().find(|line| line.starts_with("build-time: "))
        else {
            panic!("missing build-time line in stdout: {stdout}")
        };
        let Some(build_time) = build_time_line.strip_prefix("build-time: ") else {
            panic!("missing build-time value in line: {build_time_line}")
        };
        assert!(
            build_time.contains('T') && build_time.contains('Z'),
            "build-time should be ISO 8601 format, got: {build_time}"
        );

        let Some(revision_line) = stdout.lines().find(|line| line.starts_with("revision: ")) else {
            panic!("missing revision line in stdout: {stdout}")
        };
        let Some(revision) = revision_line.strip_prefix("revision: ") else {
            panic!("missing revision value in line: {revision_line}")
        };
        assert!(!revision.trim().is_empty(), "revision should not be empty");
    }
}

#[cfg(test)]
mod exec {
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
    fn graceful_shutdown() {
        let output = run_fixture("graceful_shutdown.yaml", "run");
        assert_exit_code(&output, 0);
        assert_stdout_contains(&output, "trapper-started");
        assert_stdout_contains(&output, "[trapper] sigterm-received");
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
}

#[cfg(test)]
mod env {
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
    fn env_self_referencing_cycle() {
        let output = run_fixture("env_self_ref.yaml", "run");
        assert_ne!(output.status.code(), Some(0_i32));
        assert_stdout_contains(&output, "circular variable interpolation: A -> A");
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
}

#[cfg(test)]
mod oneshot {
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
}

#[cfg(test)]
mod validation {
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
        let output = match std::process::Command::new(binary_path())
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
    fn nonexistent_shell_binary() {
        let output = run_fixture("shell_nonexistent.yaml", "run");
        assert_ne!(output.status.code(), Some(0_i32));
        assert_stdout_contains(&output, "failed to spawn");
    }
}

#[cfg(test)]
mod docker {
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
}
