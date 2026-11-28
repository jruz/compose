mod rust '.just/rust.justfile'
mod just '.just/just.justfile'
mod oxfmt '.just/oxfmt.justfile'
mod git '.just/git.justfile'
mod tools '.just/tools.justfile'

release_bin := "target/docker/" + arch() + "-unknown-linux-musl/release/compose"

check:
    just just::fmt-check
    just oxfmt::fmt-check
    just rust::check

check-ci:
    just check
    just test

fix:
    just just::fmt-fix
    just oxfmt::fmt-fix
    just rust::fix

setup:
    direnv allow
    just git::hooks-setup

install:
    just rust::install

test *FILTER='':
    nu scripts/rust-test.nu {{ FILTER }}

run *ARGS='':
    just rust::run {{ ARGS }}

watch:
    just rust::watch

example-run:
    sandbox cargo build --release --target {{ arch() }}-unknown-linux-musl
    ./{{ release_bin }} run --config examples/compose.run.yaml

example-up:
    sandbox cargo build --release --target {{ arch() }}-unknown-linux-musl
    ./{{ release_bin }} up --config examples/compose.up.yaml

example-fail:
    sandbox cargo build --release --target {{ arch() }}-unknown-linux-musl
    ./{{ release_bin }} run --config examples/compose.fail.yaml

example-docker-run:
    sandbox cargo build --release --target {{ arch() }}-unknown-linux-musl
    ./{{ release_bin }} run --config examples/compose.docker-run.yaml

example-docker-fail:
    sandbox cargo build --release --target {{ arch() }}-unknown-linux-musl
    ./{{ release_bin }} run --config examples/compose.docker-fail.yaml

example-invalid-fields:
    sandbox cargo build --release --target {{ arch() }}-unknown-linux-musl
    ./{{ release_bin }} run --config examples/compose.invalid-fields.yaml

generate-screenshots:
    sandbox cargo build --release --target {{ arch() }}-unknown-linux-musl
    vhs examples/vhs/hero.tape
    vhs examples/vhs/run.tape
    vhs examples/vhs/up.tape
    vhs examples/vhs/fail.tape

[positional-arguments]
release +ARGS:
    nu scripts/release.nu "$@"
