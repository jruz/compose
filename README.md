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
inputs.composeSrc.url = "git+https://codeberg.org/jruz/compose.git?ref=main";
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

### `run` — run a task with its depending services

- Starts all processes. When the last declared process exits, all others are killed.
- Last process exit code is returned.
- If a support process crashes first, all are killed and exit code 1 is returned.
- Oneshot processes run to completion during startup before dependents start.

```
compose run --config compose.yaml
```

Example config — run a migration with a database:

```yaml
processes:
  db:
    command: "postgres -D /tmp/pgdata"
    health:
      command: "pg_isready"
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
processes:
  redis:
    command: "redis-server"
    health:
      command: "redis-cli ping"
      interval_secs: 1
      timeout_secs: 5

  web:
    command: "node server.js"
    depends_on: [redis]
    env:
      PORT: "3000"

  worker:
    command: "node worker.js"
    depends_on: [redis]
```

### Docker mode

Instead of running shell commands, processes can run as docker containers.

- Each run creates an isolated docker network
- Services are reachable by their process name (e.g. process `db` is reachable as `db`)
- Health checks run inside the container via `docker exec` (`command` uses `sh -c`, `exec` runs directly)
- All containers and the network are cleaned up on exit

```
compose up --config compose.yaml
```

Example config — postgres, redis, and nginx with `${VAR}` interpolation:

```yaml
processes:
  postgres:
    docker:
      image: postgres:16-alpine
      ports:
        - "${PG_PORT}:5432"
    env:
      POSTGRES_PASSWORD: "${PG_PASSWORD}"
      POSTGRES_DB: "${PG_DB}"
    health:
      command: "pg_isready -U postgres"
      interval_secs: 1
      timeout_secs: 10

  redis:
    docker:
      image: redis:7-alpine
      ports:
        - "${REDIS_PORT}:6379"
    health:
      command: "redis-cli ping"
      interval_secs: 1
      timeout_secs: 5

  api:
    docker:
      image: nginx:alpine
      ports:
        - "${API_PORT}:80"
    depends_on: [postgres, redis]
```

With a sops-encrypted env file:

```
compose up --config compose.yaml --env-file <(sops --decrypt secrets.env)
```

Build from a Dockerfile instead of pulling an image:

```yaml
processes:
  app:
    docker:
      build: ./app
      dockerfile: Dockerfile.dev
      ports:
        - "3000:3000"
      volumes:
        - "/absolute/path/to/src:/app/src"
```

### Environment variables

Inline per-process:

```yaml
processes:
  app:
    command: "node server.js"
    env:
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

Variable interpolation with `${VAR}` in env values:

```yaml
processes:
  app:
    command: "node server.js"
    env:
      DATABASE_URL: "postgres://user:${DB_PASSWORD}@localhost:5432/${DB_NAME}"
```

Precedence: process env > env file > host environment.

## Config schema

```yaml
shell: <string> # shell for exec processes (default: sh)
processes:
  <name>:
    type: <service|oneshot> # process type (default: service)
    command: <string> # shell command (exec) or container CMD override (docker)
    exec: [<string>, ...] # container CMD override, exec form (docker only)
    docker: # presence of this section makes it a docker process
      image: <string> # docker image (required if no build)
      build: <string> # build context path (required if no image)
      dockerfile: <string> # dockerfile path (optional)
      ports: [<host:container>] # port mappings
      volumes: [<host:container>] # volume mounts
      stop_timeout_secs: <int> # seconds before force kill (default: 10)
    depends_on: [<string>, ...] # processes that must be healthy first
    env:
      KEY: "VALUE"
    env_file: <path> # KEY=VALUE format, applied to this process
    health: # health check (one of command or exec)
      command: <string> # shell command (exit 0 = healthy)
      exec: [<string>, ...] # direct execution without shell
      interval_secs: <int> # seconds between retries
      timeout_secs: <int> # seconds before giving up
```

### Process types

- **service** (default) — expected to stay running. Exiting is treated as a crash.
- **oneshot** — runs to completion during startup. Exit 0 = success, dependents start. Exit non-0 = failure. Cannot have health checks — successful exit is the readiness signal.

Example config — database with migrations and seeding before server:

```yaml
processes:
  db:
    command: "postgres -D /tmp/pgdata"
    health:
      command: "pg_isready"
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

[AGPL-3.0](LICENSE)
