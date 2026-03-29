# compose

Lightweight process orchestrator. Reads a YAML config, spawns processes with dependency ordering, health checks, and colored output.

## Commands

- `just check` - full lint suite (fmt, clippy, build, test)
- `just test` - run tests with cargo nextest
- `just fix` - auto-fix fmt and clippy
- `just compose-check` - run `just check` inside a podman container via compose

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
- `build_actor.rs` - builds docker images for processes with build contexts in dependency order
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
- `CLAUDE.md` - config schema below
- `~/dev/.claude/skills/compose/SKILL.md` - global skill docs and examples

## Config Schema

```yaml
runtime: docker | podman         # optional, default: docker
shell: <string>                  # optional, default: sh
processes:
  <name>:
    command: <string>             # shell command (exec) or CMD override (docker), supports ${VAR}
    exec: [<string>, ...]         # CMD override exec form (docker only), supports ${VAR}
    docker:                       # container driver (alternative to command)
      image: <string>             # required if no build
      build: <path>               # build context (required if no image)
      dockerfile: <path>          # optional, custom Dockerfile path
      ports: [<host:container>]     # supports ${VAR} interpolation
      volumes: [<host:container>]   # supports ${VAR} interpolation
      stop_timeout_secs: <int>    # optional, omit for docker default (10s)
    type: service | oneshot       # optional, default: service
    depends_on: [<name>, ...]     # optional
    env:                          # optional
      KEY: "VALUE"
    env_file: <path>              # optional, KEY=VALUE format
    health:                       # optional (not allowed on oneshot)
      command: <string>           # shell health check, supports ${VAR} interpolation
      exec: [<string>, ...]       # exec health check (no shell), supports ${VAR} interpolation
      interval_secs: <int>
      timeout_secs: <int>
```

### Variable Interpolation

All string values support `${VAR}` interpolation from resolved env vars (inline env, env_file, --env-file) and host environment. Applies to: `command`, `exec`, `ports`, `volumes`, `health.command`, `health.exec`. Undefined variables stay literal.
