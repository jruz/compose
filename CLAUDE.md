# compose

Lightweight process orchestrator. Reads a YAML config, spawns processes with dependency ordering, health checks, and colored output.

## Commands

- `just check` - full lint suite (fmt, clippy, build, test)
- `just test` - run tests with cargo nextest
- `just fix` - auto-fix fmt and clippy

## Architecture

- `cli.rs` - clap derive CLI: `run` (exit on last process) and `up` (exit on any process)
- `config.rs` - YAML loading, validation (circular deps, unknown deps, empty health), env file parsing
- `actor.rs` - shared actor loop: output streaming, health check polling, graceful kill (exec: SIGTERM+SIGKILL, docker: docker stop)
- `process.rs` - spawns exec processes via `sh -c`, delegates to actor loop
- `docker_actor.rs` - spawns docker containers via `docker run`, delegates to actor loop
- `docker.rs` - docker CLI wrappers (build, run, exec, stop, rm, network)
- `supervisor.rs` - starts processes in dependency order, manages lifecycle, handles shutdown signals
- `env.rs` - env resolution: global env file, process env file, inline env, `${VAR}` interpolation with cycle detection
- `formatter.rs` - colored tagged output, 6-color cycle
- `message.rs` - event and display message enums for actor-supervisor communication

## Testing

- integration tests only, in `tests/integration.rs`
- fixtures in `tests/fixtures/*.yaml`
- tests run the compiled binary via `env!("CARGO_BIN_EXE_compose")`

## Config Schema

```yaml
processes:
  <name>:
    command: <string>             # exec driver
    docker:                       # docker driver (alternative to command)
      image: <string>             # required if no build
      build: <path>               # build context
      dockerfile: <path>          # optional, custom Dockerfile path
      ports: [<host:container>]
      volumes: [<host:container>]
      stop_timeout_secs: <int>    # optional, omit for docker default (10s)
    type: service | oneshot       # optional, default: service
    depends_on: [<name>, ...]     # optional
    env:                          # optional
      KEY: "VALUE"
    env_file: <path>              # optional, KEY=VALUE format
    health:                       # optional (not allowed on oneshot)
      command: <string>           # shell health check
      exec: [<string>, ...]       # exec health check (alternative to command)
      interval_secs: <int>
      timeout_secs: <int>
```
