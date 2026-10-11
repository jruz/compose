use std::ffi::OsString;
use std::process::Command;

type BuildResult<T> = Result<T, Box<dyn std::error::Error>>;

struct Revision {
    value: String,
    watched_paths: Vec<String>,
}

fn run(program: &str, args: &[&str]) -> BuildResult<String> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        let command = args.join(" ");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr.trim();
        let message = format!("{program} {command}: {reason}");
        let error: Box<dyn std::error::Error> = message.into();
        return Err(error);
    }
    let stdout = String::from_utf8(output.stdout)?;
    let trimmed = stdout.trim().to_string();
    Ok(trimmed)
}

fn git(args: &[&str]) -> BuildResult<String> {
    run("git", args)
}

fn git_head_paths() -> BuildResult<Vec<String>> {
    let head_path = git(&["rev-parse", "--git-path", "HEAD"])?;
    let reference = git(&["rev-parse", "--symbolic-full-name", "HEAD"])?;
    let reference_path = git(&["rev-parse", "--git-path", &reference])?;
    Ok(vec![head_path, reference_path])
}

fn git_revision() -> BuildResult<Revision> {
    let value = git(&["rev-parse", "--short", "HEAD"])?;
    let watched_paths = git_head_paths()?;
    Ok(Revision {
        value,
        watched_paths,
    })
}

fn env_revision(raw: OsString) -> BuildResult<Revision> {
    let Ok(value) = raw.into_string() else {
        let error: Box<dyn std::error::Error> = "REVISION is not valid UTF-8".into();
        return Err(error);
    };
    Ok(Revision {
        value,
        watched_paths: Vec::new(),
    })
}

fn revision() -> BuildResult<Revision> {
    match std::env::var_os("REVISION") {
        Some(raw) => env_revision(raw),
        None => git_revision(),
    }
}

fn build_time() -> BuildResult<String> {
    run("date", &["-u", "+%Y-%m-%d %H:%M:%S UTC"])
}

fn main() -> BuildResult<()> {
    let build_time = build_time()?;
    let revision = revision()?;

    println!("cargo:rustc-env=BUILD_TIME={build_time}");
    println!("cargo:rustc-env=REVISION={}", revision.value);
    println!("cargo:rerun-if-env-changed=REVISION");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");
    revision
        .watched_paths
        .iter()
        .for_each(|path| println!("cargo:rerun-if-changed={path}"));

    Ok(())
}
