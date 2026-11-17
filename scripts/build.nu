use lib/log.nu
use lib/targets.nu [TARGETS]

const BUILD_IMAGE = 'docker.io/messense/cargo-zigbuild:0.24.1'
const DIST_DIR = 'target/dist'


def select-targets [target: string]: nothing -> list<record> {
    $TARGETS | where $target == all or $"($it.arch)-($it.os)" == $target
}


def build-command [build: record]: nothing -> string {
    let triples = $build.targets | each {|target| $"($target.arch)-($target.os)" }
    let install_targets = $triples | str join ','
    let build_targets = $triples | each {|triple| $"--target ($triple)" } | str join ' '
    let copies = $build.targets | each {|target|
        let source = $"/build-cache/($target.arch)-($target.os)/($build.profile)/($build.name)"
        let destination = $"($DIST_DIR)/($build.name)-($target.os)-($target.arch)"
        $"install --mode=755 ($source) ($destination)"
    }
    [
        'set -euo pipefail'
        $"mkdir --parents ($DIST_DIR)"
        $"rustup toolchain install ($build.toolchain) --profile minimal --no-self-update --target ($install_targets)"
        $"cargo zigbuild --locked --profile ($build.profile) ($build_targets)"
        ...$copies
    ] | str join (char newline)
}


def project-cache-prefix []: nothing -> string {
    let remote = (^git remote get-url origin)
    let name = $remote | str trim | path basename | str replace --regex '\.git$' ''
    $"($name)-release"
}


def run-build [build: record]: nothing -> nothing {
    log info $"building ($build.name) for ($build.targets | length) targets with profile ($build.profile)"
    let cache_prefix = project-cache-prefix
    let image_key = $BUILD_IMAGE | hash sha256 | str substring 0..11
    let target_volume = $"($cache_prefix)-target-($build.toolchain)-($image_key)"
    let uid = (^id --user | str trim)
    let gid = (^id --group | str trim)
    (^podman run
        --rm
        --userns keep-id
        --user $"($uid):($gid)"
        --read-only
        --cap-drop ALL
        --security-opt no-new-privileges
        --tmpfs /tmp:exec
        --tmpfs $env.HOME
        --volume $"(pwd):/app"
        --volume $"($cache_prefix)-rustup:/usr/local/rustup:nocopy"
        --volume $"($cache_prefix)-registry:/usr/local/cargo/registry:nocopy"
        --volume $"($cache_prefix)-git:/usr/local/cargo/git:nocopy"
        --volume $"($target_volume):/build-cache:nocopy"
        --workdir /app
        --env REVISION
        --env $"HOME=($env.HOME)"
        --env TMPDIR=/tmp
        --env $"RUSTUP_TOOLCHAIN=($build.toolchain)"
        --env CARGO_TARGET_DIR=/build-cache
        --env XDG_CACHE_HOME=/build-cache/cache
        $BUILD_IMAGE
        bash -c (build-command $build))
}


# Cross-compile binaries in a container
def main [
    target: string # target triple such as x86_64-unknown-linux-musl, or all
    profile: string # cargo profile such as release or fast
]: nothing -> nothing {
    let build = {
        name: (open Cargo.toml | get package.name)
        toolchain: (open rust-toolchain.toml | get toolchain.channel)
        profile: $profile
        targets: (select-targets $target)
    }
    if ($build.targets | is-empty) {
        let expected = $TARGETS
            | each {|item| $"($item.arch)-($item.os)" }
            | prepend all
            | str join ', '
        log error $"unknown target: ($target)\nexpected: ($expected)"
        exit 1
    }
    run-build $build
    for target in $build.targets {
        log ok $"($DIST_DIR)/($build.name)-($target.os)-($target.arch)"
    }
}
