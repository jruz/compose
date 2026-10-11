# compose

Lightweight process orchestrator. Reads a YAML config, spawns processes with dependency ordering, health checks, and colored output.

## Commands

- `just check` - lint and check
- `just test` - integration tests
- `just fix` - auto-fix fmt and clippy
- `just install` - install the binary to `~/.cargo/bin`
- `just generate-screenshots` - regenerate README gifs from `examples/vhs/*.tape`
- `just sandbox-check` - verify sandbox isolation

## Pointers

- `justfile` - all recipes
- `flake.nix` - dev shell, `sandbox` and `sandbox-docker` wrappers
- `src/` - one module per concern, entry point `src/main.rs`
- `tests/integration/` - integration tests
- `tests/fixtures/` - test configs

## Notes

- `sandbox-docker` mounts the Docker socket, so `just test` and all dependency code it runs can bypass filesystem isolation
- the Docker opt-in is specific to this project; don't copy it into skills
- tests run the compiled binary via `env!("CARGO_BIN_EXE_compose")`, not library calls

## Documentation

When adding or changing config fields or features, update these places:

- `src/cli.rs` - `long_about` schema in `--help`
- `README.md` - config schema and usage examples
- `../agent-config/skills/compose/SKILL.md` - global skill docs and examples
