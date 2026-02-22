install:
    cargo install --path . --force

check:
    just rust-check

test:
    just rust-test

run:
    rust-run

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
    just rust-test

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
