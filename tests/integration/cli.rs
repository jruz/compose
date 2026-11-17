use super::helpers::*;

fn version_output(argument: &str) -> std::process::Output {
    let binary = binary_path();
    let directory = std::env::temp_dir();
    let output = match std::process::Command::new(binary)
        .arg(argument)
        .current_dir(directory)
        .output()
    {
        Ok(output) => output,
        Err(err) => panic!("failed to execute binary: {err}"),
    };
    assert_eq!(output.status.code(), Some(0_i32));
    assert_eq!(output.stderr, b"");
    output
}

#[test]
fn version_subcommand_matches_flag_without_config() {
    let flag = version_output("--version");
    let subcommand = version_output("version");
    assert_eq!(subcommand.stdout, flag.stdout);
}

#[test]
fn short_version_flag_matches_long_flag() {
    let long = version_output("--version");
    let short = version_output("-V");
    assert_eq!(short.stdout, long.stdout);
}

#[test]
fn version_flag() {
    let output = version_output("--version");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    let Some(version_line) = stdout.lines().find(|line| line.contains("version: ")) else {
        panic!("missing version line in stdout: {stdout}")
    };
    let Some(version) = version_line.split("version: ").nth(1) else {
        panic!("missing version value in line: {version_line}")
    };
    assert!(!version.trim().is_empty(), "version should not be empty");

    let Some(build_time_line) = stdout.lines().find(|line| line.starts_with("build-time: ")) else {
        panic!("missing build-time line in stdout: {stdout}")
    };
    let Some(build_time) = build_time_line.strip_prefix("build-time: ") else {
        panic!("missing build-time value in line: {build_time_line}")
    };
    assert!(
        build_time.contains("UTC"),
        "build-time should contain UTC, got: {build_time}"
    );

    let Some(revision_line) = stdout.lines().find(|line| line.starts_with("revision: ")) else {
        panic!("missing revision line in stdout: {stdout}")
    };
    let Some(revision) = revision_line.strip_prefix("revision: ") else {
        panic!("missing revision value in line: {revision_line}")
    };
    assert!(!revision.trim().is_empty(), "revision should not be empty");
}
