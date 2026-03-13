install:
    cargo install --path . --force

check:
    just woodpecker-check
    just rust-check

check-ci:
    just check
    just rust-test

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

example-docker-fail:
    cargo build --release
    ./target/release/compose run --config examples/compose.docker-fail.yaml

example-invalid-fields:
    cargo build --release
    ./target/release/compose run --config examples/compose.invalid-fields.yaml

generate-screenshots:
    cargo build --release
    vhs examples/vhs/hero.tape
    vhs examples/vhs/run.tape
    vhs examples/vhs/up.tape
    vhs examples/vhs/fail.tape

release-build-linux:
    scripts/release-build.sh unknown-linux-gnu x86_64
    scripts/release-build.sh unknown-linux-gnu aarch64

release-build-macos:
    scripts/release-build.sh apple-darwin x86_64
    scripts/release-build.sh apple-darwin aarch64

release-upload TAG:
    bun scripts/release-upload.ts {{ TAG }}

release-delete TAG:
    bun scripts/release-delete.ts {{ TAG }}

woodpecker-check:
    #!/usr/bin/env nu
    glob .woodpecker/*.yml | each {|f|
        print $"Linting ($f)..."
        woodpecker-cli lint $f
    }
    print $"(ansi green)OK: All woodpecker configs valid(ansi reset)"
