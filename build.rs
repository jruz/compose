fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::process::Command::new("date")
        .arg("-u")
        .arg("+%Y-%m-%d %H:%M:%S UTC")
        .output()?;
    let build_time = String::from_utf8_lossy(&output.stdout).trim().to_string();

    let revision = std::env::var("REVISION")?;

    println!("cargo:rustc-env=BUILD_TIME={build_time}");
    println!("cargo:rustc-env=REVISION={revision}");

    Ok(())
}
