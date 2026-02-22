#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process::Output;

    fn binary_path() -> PathBuf {
        env!("CARGO_BIN_EXE_simple-compose").into()
    }

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn run_fixture(fixture: &str, subcommand: &str) -> Output {
        let Ok(output) = std::process::Command::new(binary_path())
            .arg("--file")
            .arg(fixture_path(fixture))
            .arg(subcommand)
            .output()
        else {
            panic!("failed to execute binary with fixture {fixture}")
        };
        output
    }

    fn stdout_text(output: &Output) -> String {
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    fn stderr_text(output: &Output) -> String {
        String::from_utf8_lossy(&output.stderr).to_string()
    }

    #[test]
    fn run_main_exits_zero() {
        let output = run_fixture("run_exit_zero.toml", "run");
        assert_eq!(
            output.status.code(),
            Some(0_i32),
            "stdout: {}\nstderr: {}",
            stdout_text(&output),
            stderr_text(&output)
        );
        assert!(stdout_text(&output).contains("hello"));
    }

    #[test]
    fn run_main_exits_nonzero() {
        let output = run_fixture("run_exit_nonzero.toml", "run");
        assert_eq!(
            output.status.code(),
            Some(42_i32),
            "stdout: {}\nstderr: {}",
            stdout_text(&output),
            stderr_text(&output)
        );
    }

    #[test]
    fn run_support_crashes_exits_one() {
        let output = run_fixture("run_support_crashes.toml", "run");
        assert_eq!(
            output.status.code(),
            Some(1_i32),
            "stdout: {}\nstderr: {}",
            stdout_text(&output),
            stderr_text(&output)
        );
        assert!(stdout_text(&output).contains("crashed"));
    }

    #[test]
    fn up_any_exit_triggers_shutdown() {
        let output = run_fixture("up_any_exit.toml", "up");
        assert_eq!(
            output.status.code(),
            Some(1_i32),
            "stdout: {}\nstderr: {}",
            stdout_text(&output),
            stderr_text(&output)
        );
        assert!(stdout_text(&output).contains("crashed"));
    }

    #[test]
    fn dependency_ordering() {
        let output = run_fixture("dependency_order.toml", "run");
        let stdout = stdout_text(&output);
        let db_pos = stdout.find("db-started");
        let app_pos = stdout.find("app-started");
        assert!(
            db_pos.is_some() && app_pos.is_some(),
            "expected both db-started and app-started in output\nstdout: {stdout}"
        );
        assert!(
            db_pos < app_pos,
            "db should start before app\nstdout: {stdout}"
        );
    }

    #[test]
    fn health_check_gates_dependent() {
        let output = run_fixture("health_check.toml", "run");
        let stdout = stdout_text(&output);
        assert!(
            stdout.contains("healthy"),
            "expected healthy message\nstdout: {stdout}"
        );
        let healthy_pos = stdout.find("healthy");
        let consumer_pos = stdout.find("consumer-started");
        assert!(
            healthy_pos.is_some() && consumer_pos.is_some(),
            "expected both healthy and consumer-started\nstdout: {stdout}"
        );
        assert!(
            healthy_pos < consumer_pos,
            "healthy should appear before consumer-started\nstdout: {stdout}"
        );
    }

    #[test]
    fn validation_duplicate_names() {
        let output = run_fixture("duplicate_names.toml", "run");
        assert_ne!(output.status.code(), Some(0_i32));
        assert!(stderr_text(&output).contains("duplicate"));
    }

    #[test]
    fn validation_unknown_dependency() {
        let output = run_fixture("unknown_dependency.toml", "run");
        assert_ne!(output.status.code(), Some(0_i32));
        assert!(stderr_text(&output).contains("unknown"));
    }

    #[test]
    fn validation_circular_dependency() {
        let output = run_fixture("circular_dependency.toml", "run");
        assert_ne!(output.status.code(), Some(0_i32));
        assert!(stderr_text(&output).contains("circular"));
    }

    #[test]
    fn validation_no_processes() {
        let output = run_fixture("no_processes.toml", "run");
        assert_ne!(output.status.code(), Some(0_i32));
        let stderr = stderr_text(&output);
        assert!(
            stderr.contains("no processes") || stderr.contains("missing field"),
            "expected 'no processes' or parse error\nstderr: {stderr}"
        );
    }

    #[test]
    fn missing_file() {
        let Ok(output) = std::process::Command::new(binary_path())
            .arg("--file")
            .arg("/tmp/nonexistent_compose_file_12345.toml")
            .arg("run")
            .output()
        else {
            panic!("failed to execute binary")
        };

        assert_ne!(output.status.code(), Some(0_i32));
        assert!(!stderr_text(&output).is_empty());
    }
}
