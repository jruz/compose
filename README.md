# compose

- Process orchestrator for shell commands and docker containers
- Colored, prefixed output per process
- Dependency ordering and health checks
- Dependency failure stops the entire workflow
- Exit code of the last process is forwarded

<img src="examples/vhs/hero.gif" width="600">

## Install

Nix flake input:

```nix
inputs.composeSrc.url = "git+https://github.com/jruz/compose.git?ref=main";
```

Then add to your devshell:

```nix
let
  compose = composeSrc.packages.${system}.default;
in
pkgs.mkShell {
  buildInputs = [ compose ];
}
```

From source:

```
cargo install --path .
```

## Usage

`compose version`, `compose --version`, and `compose -V` show the version, revision, and build time without loading a config.

### `run` — run a task with its depending services

- Starts all processes and stops the others when the last declared process exits.
- Last process exit code is returned.
- If a support process crashes first, all are killed and exit code 1 is returned.
- Oneshot processes run to completion during startup before dependents start.

```
compose run --config compose.yaml
```

Example config — run a migration with a database:

```yaml
services:
  db:
    command: "postgres -D /tmp/pgdata"
    healthcheck:
      test: [sh, -c, "pg_isready"]
      interval_secs: 1
      timeout_secs: 10

  migrate:
    command: "dbmate up"
    depends_on: [db]
```

### `up` — keep all processes running

- Starts all processes and keeps them running.
- If any process exits, all others are killed and exit code 1 is returned.

```
compose up --config compose.yaml
```

Example config — run a web app with a background worker:

```yaml
services:
  redis:
    command: "redis-server"
    healthcheck:
      test: [sh, -c, "redis-cli ping"]
      interval_secs: 1
      timeout_secs: 5

  web:
    command: "node server.js"
    depends_on: [redis]
    environment:
      PORT: "3000"

  worker:
    command: "node worker.js"
    depends_on: [redis]
```

### Commands

A host `command` string requires Bash on `PATH` and runs with `bash -c`. A container `command` string is split on whitespace and passed as a CMD override without a shell.

Use a list to pass exact arguments on either runtime:

```yaml
services:
  greet:
    command: [printf, '%s\n', "${GREETING}"]
    environment:
      GREETING: hello world
```

Compose expands `${VAR}` in each list item without splitting it or interpreting shell syntax. Containers without a `command` use the image's default command.

### Container mode

Instead of running shell commands, processes can run as containers via docker or podman.

- Set `runtime: podman` at the top level to use podman (default: `docker`)
- Each run creates an isolated container network
- Services are reachable by their process name (e.g. process `db` is reachable as `db`)
- Health checks run inside the container.
- All containers and the network are cleaned up on exit
- Set `docker.run.init: true` to add `--init` for file watchers that need signal forwarding and child reaping.

```
compose up --config compose.yaml
```

Example config — postgres, redis, and nginx with `${VAR}` interpolation:

```yaml
services:
  postgres:
    docker:
      image: postgres:16-alpine
      run:
        publish:
          - "${PG_PORT}:5432"
    environment:
      POSTGRES_PASSWORD: "${PG_PASSWORD}"
      POSTGRES_DB: "${PG_DB}"
    healthcheck:
      test: [sh, -c, "pg_isready -U postgres"]
      interval_secs: 1
      timeout_secs: 10

  redis:
    docker:
      image: redis:7-alpine
      run:
        publish:
          - "${REDIS_PORT}:6379"
    healthcheck:
      test: [sh, -c, "redis-cli ping"]
      interval_secs: 1
      timeout_secs: 5

  api:
    docker:
      image: nginx:alpine
      run:
        publish:
          - "${API_PORT}:80"
    depends_on: [postgres, redis]
```

With a sops-encrypted env file:

```
compose up --config compose.yaml --env-file <(sops --decrypt secrets.env)
```

Build from a Dockerfile instead of pulling an image:

```yaml
services:
  app:
    docker:
      build:
        context: ./app
        dockerfile: Dockerfile.dev
      run:
        init: true
        publish:
          - "3000:3000"
        volume:
          - "/absolute/path/to/src:/app/src"
```

Pass explicit build args to `docker build`:

```yaml
services:
  app:
    docker:
      build:
        context: ./app
        args:
          NODE_VERSION: "${NODE_VERSION}"
          ENV: "development"
```

When `docker.build.args` is omitted, resolved env vars (`environment`, `env_file`, global `--env-file`) are forwarded as build args. Set `args: {}` to forward none. Use explicit build args for non-secret inputs.

Use Docker or Podman long flag names without `--`. Scalars become `--flag=value`, and lists repeat the flag.

`docker.build.args` uses `--build-arg`. Maps for `args` and `label` produce `--flag=KEY=value`. Quote version numbers to preserve their text.

Compose reserves build `tag`, `progress`, and `help`, and run `attach`, `detach`, `help`, `log-driver`, `name`, `net`, `network`, `network-alias`, `restart`, `rm`, and `stop-timeout`. Use `docker.stop_timeout_secs` for shutdown timing.

`docker.build.dockerfile` passes the Dockerfile path to `--file`. `docker.run` contains extra CLI flags; `image` and `stop_timeout_secs` stay directly under `docker`.

### Environment variables

Inline per-process:

```yaml
services:
  app:
    command: "node server.js"
    environment:
      PORT: "3000"
      NODE_ENV: "development"
```

From a `.env` applied to all processes:

```
compose run --config compose.yaml --env-file .env
```

With sops-encrypted secrets via process substitution:

```
compose run --config compose.yaml --env-file <(sops --decrypt secrets.env)
```

Variable interpolation with `${VAR}` uses resolved env vars and host environment:

```yaml
services:
  app:
    docker:
      build:
        context: .
      run:
        publish:
          - "${APP_PORT}:${APP_PORT}"
    command: "node server.js --port ${APP_PORT}"
    environment:
      APP_PORT: "3000"
      DATABASE_URL: "postgres://user:${DB_PASSWORD}@localhost:5432/${DB_NAME}"
    healthcheck:
      test: ["curl", "--fail", "http://localhost:${APP_PORT}/health"]
      interval_secs: 1
      timeout_secs: 10
```

Applies to: `command` strings and list items, `docker.image`, `docker.build.context`, `docker.build.dockerfile`, build and run flag values, arguments in `healthcheck.test`, and `environment` values.
Undefined variables stay literal: `${UNKNOWN}` remains `${UNKNOWN}`.

Precedence: service `environment` > service `env_file` > CLI `--env-file` > launching environment.

## Config schema

Use `services` for host processes, containers, and one-shot tasks.

```yaml
runtime: <docker|podman> # container runtime (default: docker)
services:
  <name>:
    type: <service|oneshot> # process type (default: service)
    command: <string|list> # host Bash command or container CMD override; lists pass arguments directly
    docker: # presence of this section makes it a docker process
      image: <string> # docker image (required if no build)
      build: # required if no image
        context: <string> # build context path
        dockerfile: <string> # optional Dockerfile path passed to --file
        <flag>: <value> # Docker or Podman build flag
      run:
        <flag>: <value> # Docker or Podman run flag
      stop_timeout_secs: <int> # seconds before force kill (default: 10)
    depends_on: [<string>, ...] # processes that must be healthy first
    environment:
      KEY: "VALUE"
    env_file: <path> # KEY=VALUE format, applied to this process
    healthcheck: # startup probe (exit 0 = ready)
      test: [<executable>, ...] # runs directly; use [sh, -c, <script>] for shell syntax
      interval_secs: <int> # seconds between retries
      timeout_secs: <int> # seconds before giving up
```

### Process types

- **service** (default) — expected to stay running until shutdown.
- **oneshot** — must exit successfully before dependents start.

One-shot tasks cannot have health checks.

Example config — database with migrations and seeding before server:

```yaml
services:
  db:
    command: "postgres -D /tmp/pgdata"
    healthcheck:
      test: [sh, -c, "pg_isready"]
      interval_secs: 1
      timeout_secs: 10

  migrate:
    type: oneshot
    command: "dbmate up"
    depends_on: [db]

  seed:
    type: oneshot
    command: "psql -f seed.sql"
    depends_on: [migrate]

  server:
    command: "node server.js"
    depends_on: [seed]
```

## Output

Run

<img src="examples/vhs/run.gif" width="600">

Up

<img src="examples/vhs/up.gif" width="600">

Failure

<img src="examples/vhs/fail.gif" width="600">

## License

Licensed under the [EUPL](LICENSE).
