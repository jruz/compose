# compose

Lightweight process orchestrator. Reads a YAML config, spawns processes with dependency ordering, health checks, and colored output.

## Commands

- `just check` - full lint suite (fmt, clippy, build, test) inside sandbox
- `just check-ci` - full lint suite + tests inside sandbox
- `just test` - run tests with cargo nextest
- `just fix` - auto-fix fmt and clippy
- `just sandbox-check` - verify sandbox filesystem isolation and argument forwarding

## Sandbox

- `flake.nix` defines a `sandbox` wrapper using bubblewrap (`bwrap`)
- Unshares all namespaces except network (`--unshare-all --share-net`)
- `$HOME` is a tmpfs — only `$HOME/.cargo` and `$HOME/.gitconfig` are bind-mounted
- Current working directory is read-write, `/nix` and system paths are read-only
- PATH is restricted to an explicit set of tools defined in `sandboxPath`
- `just check` and `just check-ci` run inside the sandbox
- Two devShells: `default` (minimal: just, nushell, sandbox) and `sandbox` (full tooling for use inside the sandbox)

## Architecture

- `main.rs` - entry point: parses CLI, loads config, runs supervisor
- `cli.rs` - clap derive CLI: `run` (exit on last process) and `up` (exit on any process)
- `config.rs` - YAML loading, validation (circular deps, unknown deps, empty health), env file parsing
- `env.rs` - env resolution: global env file, process env file, inline env, `${VAR}` interpolation with cycle detection
- `supervisor.rs` - coordinates docker infrastructure, process startup, lifecycle management, and cleanup
- `startup.rs` - resolves dependency order, spawns processes, waits for health checks
- `lifecycle.rs` - manages process lifecycle strategies and signal handling
- `shutdown.rs` - sends kill signals to actors and shutdown notifications
- `actor.rs` - async loop: watches process, reports exit via ActorExited, responds to kill command, graceful shutdown (SIGTERM+SIGKILL for exec, docker stop for containers)
- `process.rs` - spawns exec processes via `sh -c`, delegates to actor loop
- `docker_actor.rs` - spawns docker containers via `docker run`, delegates to actor loop
- `build_actor.rs` - builds docker images for processes with build contexts in dependency order, passes build_args or resolved env as --build-arg
- `docker.rs` - container CLI wrappers (build, run, exec, stop, rm, network), runtime-agnostic via binary parameter
- `docker_infra.rs` - manages docker network creation, image building, and cleanup
- `health.rs` - health check types, config parsing, and execution (shell command or exec form), called from startup.rs
- `formatter.rs` - colored tagged output, 6-color cycle
- `message.rs` - ActorExited struct (actor→supervisor), DisplayMsg enum (anyone→formatter), ActorHandle struct, Command enum
- `version.rs` - version string from build environment variables

## Testing

- integration tests only, in `tests/integration.rs`
- fixtures in `tests/fixtures/*.yaml`
- tests run the compiled binary via `env!("CARGO_BIN_EXE_compose")`

## Documentation

When adding or changing config fields or features, update all four places:

- `src/cli.rs` - `long_about` schema in `--help`
- `README.md` - config schema and usage examples
- `CLAUDE.md` - config
- `~/dev/projects/agent-config/skills/compose/SKILL.md` - global skill docs and examples
