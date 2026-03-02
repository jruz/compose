install:
    cargo install --path . --force

check:
    just rust-check

test:
    just rust-test

run *ARGS='':
    cargo run {{ARGS}}

watch:
    just rust-watch

fix:
    just rust-fix

rust-build:
    cargo build --all-targets

rust-cargo-check:
    cargo check --all-targets

rust-check:
    just rust-cargo-check
    just rust-fmt-check
    just rust-clippy

rust-clippy *ARGS='':
    cargo clippy --all-targets {{ARGS}}

rust-clippy-fix *ARGS='':
    cargo clippy \
        --fix \
        --all-targets \
        --allow-dirty \
        {{ARGS}}

rust-fix:
    just rust-fmt-fix
    just rust-clippy-fix

rust-fmt-check:
    cargo fmt --all -- --check

rust-fmt-fix:
    cargo fmt --all

rust-outdated:
    cargo outdated

rust-test *FILTER='':
    cargo nextest run \
        --no-tests=pass \
        {{FILTER}}

rust-watch:
    cargo watch --exec run

rust-upgrade:
    cargo upgrade --incompatible
    cargo update

example-run:
    cargo build --release
    ./target/release/compose run --config examples/compose.run.yaml

example-up:
    cargo build --release
    ./target/release/compose up --config examples/compose.up.yaml

example-fail:
    cargo build --release
    ./target/release/compose run --config examples/compose.fail.yaml

generate-screenshots:
    cargo build --release
    vhs examples/vhs/hero.tape
    vhs examples/vhs/run.tape
    vhs examples/vhs/up.tape
    vhs examples/vhs/fail.tape

release-build:
    #!/usr/bin/env nu
    let targets = [
        {target: "x86_64-unknown-linux-gnu", suffix: "linux-x86_64"}
        {target: "aarch64-unknown-linux-gnu", suffix: "linux-arm64"}
        {target: "x86_64-apple-darwin", suffix: "macos-x86_64"}
        {target: "aarch64-apple-darwin", suffix: "macos-arm64"}
    ]
    for t in $targets {
        let output = $"compose-($t.suffix)"
        print $"Building ($output)..."
        ^cargo build --release --target $t.target
        cp $"target/($t.target)/release/compose" $output
        print $"(ansi green)OK: ($output)(ansi reset)"
    }

release-upload tag:
    bun scripts/release-upload.ts {{ tag }}
