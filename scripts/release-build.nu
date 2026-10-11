use log.nu

const BUILD_IMAGE = 'ghcr.io/rust-cross/cargo-zigbuild:0.23.4@sha256:d8313491ec5798de0633fdc1c5753761bff79967bea69076020dc78121b2cca8'
const TARGETS = [
    {os: unknown-linux-musl, arch: x86_64}
    {os: unknown-linux-musl, arch: aarch64}
    {os: apple-darwin, arch: x86_64}
    {os: apple-darwin, arch: aarch64}
]


def select-targets [target: string]: nothing -> list<record> {
    let selected = $TARGETS | where $target == all or $"($it.arch)-($it.os)" == $target
    if ($selected | is-empty) {
        let expected = $TARGETS
            | each {|item| $"($item.arch)-($item.os)" }
            | prepend all
            | str join ', '
        error make {msg: $"unknown release target: ($target)\nexpected: ($expected)"}
    }
    $selected
}


def build-command [build: record]: nothing -> string {
    let triples = $build.targets | each {|target| $"($target.arch)-($target.os)" }
    let install_targets = $triples | str join ','
    let build_targets = $triples | each {|triple| $"--target ($triple)" } | str join ' '
    let copies = $build.targets | each {|target|
        let source = $"/build-cache/($target.arch)-($target.os)/release/($build.name)"
        let destination = $"($build.name)-($target.os)-($target.arch)"
        $"cp ($source) ($destination)"
    }
    [
        'set -euo pipefail'
        $"rustup toolchain install ($build.toolchain) --profile minimal --no-self-update --target ($install_targets)"
        $"cargo zigbuild --locked --release ($build_targets)"
        ...$copies
    ] | str join (char newline)
}


def project-cache-prefix []: nothing -> string {
    let remote = ^git remote get-url origin
    let name = $remote | path basename | str replace --regex '\.git$' ''
    $"($name)-release"
}


def run-build [build: record]: nothing -> nothing {
    log info $"building ($build.name) for ($build.targets | length) release targets"
    let cache_prefix = project-cache-prefix
    let image_key = $BUILD_IMAGE | hash sha256 | str substring 0..11
    let target_volume = $"($cache_prefix)-target-($build.toolchain)-($image_key)"
    (^podman run
        --rm
        --volume $"(pwd):/workspace"
        --volume $"($cache_prefix)-rustup:/usr/local/rustup:nocopy"
        --volume $"($cache_prefix)-registry:/usr/local/cargo/registry:nocopy"
        --volume $"($cache_prefix)-git:/usr/local/cargo/git:nocopy"
        --volume $"($target_volume):/build-cache:nocopy"
        --workdir /workspace
        --env REVISION
        --env $"RUSTUP_TOOLCHAIN=($build.toolchain)"
        --env CARGO_TARGET_DIR=/build-cache
        --env XDG_CACHE_HOME=/build-cache/cache
        $BUILD_IMAGE
        bash -c (build-command $build))
}


def main [target: string]: nothing -> nothing {
    try {
        let build = {
            name: (open Cargo.toml | get package.name)
            toolchain: (open rust-toolchain.toml | get toolchain.channel)
            targets: (select-targets $target)
        }
        run-build $build
        for target in $build.targets {
            log ok $"($build.name)-($target.os)-($target.arch)"
        }
    } catch {|err|
        log err $err.msg
        exit 1
    }
}
