set dotenv-load

# ─── general ──────────────────────────────────────────────────────────────────

check:
    nix develop --command just project-check

project-check:
    just nix-check
    just woodpecker-check
    just rust-check

check-ci:
    just check
    just test

test:
    just rust-test

install:
    sandbox cargo install \
        --path . \
        --locked \
        --force \
        --root target/dist
    install \
        --directory \
        --mode=755 \
        "$HOME/.cargo/bin"
    install \
        --mode=755 \
        target/dist/bin/* \
        "$HOME/.cargo/bin"

fix:
    just nix-fix
    just rust-fix

run *ARGS='':
    just rust-run {{ARGS}}

watch:
    just rust-watch

# ─── sandbox ─────────────────────────────────────────────────────────────────

[positional-arguments]
sandbox-check *EXTRA_ALLOWED:
    just sandbox-isolation-check
    just sandbox-allowlist-check "$@"
    just sandbox-tmp-check
    just sandbox-args-check
    just sandbox-docker-check

sandbox-isolation-check:
    sandbox bun scripts/sandbox-isolation-check.ts

[positional-arguments]
sandbox-allowlist-check *EXTRA_ALLOWED:
    sandbox nu scripts/sandbox-isolation-check.nu "$@"

sandbox-tmp-check:
    nu scripts/sandbox-tmp-check.nu

sandbox-args-check:
    nu scripts/sandbox-args-check.nu sandbox
    nu scripts/sandbox-args-check.nu sandbox-docker

sandbox-docker-check:
    nu scripts/sandbox-docker-check.nu
    sandbox-docker nu scripts/sandbox-isolation-check.nu /var/run/docker.sock

# ─── nix ──────────────────────────────────────────────────────────────────────

nix-check:
    fd \
        --extension nix \
        --exec nixfmt --check
    statix check .
    deadnix --fail .

nix-fix:
    fd \
        --extension nix \
        --exec nixfmt
    statix fix .
    deadnix --edit .

# ─── rust ─────────────────────────────────────────────────────────────────────

rust-build:
    sandbox cargo build --all-targets

rust-cargo-check:
    sandbox cargo check --all-targets

rust-check:
    just rust-cargo-check
    just rust-fmt-check
    just rust-clippy-check

rust-clippy-check *ARGS='':
    sandbox cargo clippy --all-targets {{ARGS}}

rust-clippy-fix *ARGS='':
    sandbox cargo clippy \
        --fix \
        --all-targets \
        --allow-dirty \
        {{ARGS}}

rust-fix:
    just rust-fmt-fix
    just rust-clippy-fix

rust-fmt-check:
    sandbox cargo fmt --all -- --check

rust-fmt-fix:
    sandbox cargo fmt --all

rust-outdated:
    sandbox cargo outdated

rust-test *FILTER='':
    sandbox-docker cargo nextest run \
        --no-tests=pass \
        {{FILTER}}

rust-upgrade:
    sandbox cargo upgrade --incompatible
    sandbox cargo update

rust-run *ARGS='':
    sandbox cargo run -- {{ARGS}}

rust-watch:
    sandbox cargo watch --exec run

# ─── woodpecker ───────────────────────────────────────────────────────────────

woodpecker-check:
    sandbox woodpecker-cli lint .woodpecker/

# ─── examples ─────────────────────────────────────────────────────────────────

example-run:
    sandbox cargo build --release
    ./target/release/compose run --config examples/compose.run.yaml

example-up:
    sandbox cargo build --release
    ./target/release/compose up --config examples/compose.up.yaml

example-fail:
    sandbox cargo build --release
    ./target/release/compose run --config examples/compose.fail.yaml

example-docker-run:
    sandbox cargo build --release
    ./target/release/compose run --config examples/compose.docker-run.yaml

example-docker-fail:
    sandbox cargo build --release
    ./target/release/compose run --config examples/compose.docker-fail.yaml

example-invalid-fields:
    sandbox cargo build --release
    ./target/release/compose run --config examples/compose.invalid-fields.yaml

generate-screenshots:
    sandbox cargo build --release
    vhs examples/vhs/hero.tape
    vhs examples/vhs/run.tape
    vhs examples/vhs/up.tape
    vhs examples/vhs/fail.tape

# ─── release ──────────────────────────────────────────────────────────────────

release-build TARGET:
    nu scripts/release-build.nu {{ TARGET }}

release-build-all:
    nu scripts/release-build.nu all

release-bump VERSION:
    nu scripts/release-bump.nu {{ VERSION }}

release-upload TAG:
    sandbox bun scripts/release-upload.ts {{ TAG }}

release-delete TAG:
    git tag --delete {{ TAG }}
    sandbox bun scripts/release-delete.ts {{ TAG }}
