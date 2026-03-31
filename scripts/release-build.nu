use log.nu

def main [os: string, arch: string] {
    let target = $"($arch)-($os)"
    let name = (open Cargo.toml | get package.name)

    log info $"Building ($target)"

    (^podman run
        --rm
        --volume $"(pwd):/workspace"
        --workdir /workspace
        --env $"REVISION=($env.REVISION)"
        ghcr.io/rust-cross/cargo-zigbuild:0.21.4
        bash -c $"rustup target add ($target) && cargo zigbuild --release --target ($target)")

    cp $"target/($target)/release/($name)" $"($name)-($os)-($arch)"
    log ok $"Built ($name)-($os)-($arch)"
}
