set working-directory := '..'

package-name := `nu --commands 'open Cargo.toml | get package.name'`
host-os := if os() == 'macos' { 'apple-darwin' } else { 'unknown-linux-musl' }
host-binary := 'target/dist/' + package-name + '-' + host-os + '-' + arch()

architecture-check:
    nu scripts/architecture-check.nu src

build TARGET PROFILE:
    nu scripts/build.nu {{ TARGET }} {{ PROFILE }}

build-host:
    just rust::build {{ arch() }}-{{ host-os }} fast

cargo-check:
    sandbox cargo check --all-targets

check:
    just rust::cargo-check
    just rust::fmt-check
    just rust::clippy-check
    just rust::machete
    just rust::architecture-check

clippy-check *ARGS='':
    sandbox cargo clippy --all-targets {{ ARGS }}

clippy-fix *ARGS='':
    sandbox cargo clippy \
        --fix \
        --all-targets \
        --allow-dirty \
        {{ ARGS }}

fix:
    just rust::fmt-fix
    just rust::clippy-fix

fmt-check:
    sandbox cargo fmt --all -- --check

fmt-fix:
    sandbox cargo fmt --all

install:
    just rust::build-host
    install \
        --directory \
        --mode=755 \
        "$HOME/.cargo/bin"
    install \
        --mode=755 \
        {{ host-binary }} \
        "$HOME/.cargo/bin/{{ package-name }}"

machete:
    sandbox cargo machete

outdated:
    sandbox cargo outdated

run *ARGS='':
    just rust::build-host
    {{ host-binary }} {{ ARGS }}

test *FILTER='':
    sandbox cargo nextest run \
        --no-tests=pass \
        {{ FILTER }}

upgrade:
    sandbox cargo upgrade --incompatible
    sandbox cargo update

watch:
    sandbox cargo watch --exec run
