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
