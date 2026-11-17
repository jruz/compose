# compose

Lightweight process orchestrator. Reads a YAML config, spawns processes with dependency ordering, health checks, and colored output.

## Commands

- `just check` - lint and check
- `just test` - integration tests
- `just fix` - auto-fix fmt and clippy
- `just install` - install the binary to `~/.cargo/bin`
- `just generate-screenshots` - regenerate README gifs from `examples/vhs/*.tape`

## Pointers

- `justfile` - all recipes
- `flake.nix` - dev shell and Docker sandbox wrapper
- `src/` - one module per concern, entry point `src/main.rs`
- `tests/integration/` - integration tests
- `tests/fixtures/` - test configs

## Notes

- `scripts/rust-test.nu` opts into host Docker access for integration tests
- Docker-enabled tests can bypass filesystem isolation through the host daemon
- Keep the test runner's Docker access options specific to this project
- tests run the compiled binary via `env!("CARGO_BIN_EXE_compose")`, not library calls

## Release

- `just release bump <patch|minor|major>` starts a Woodpecker release from `origin/main`.

## Documentation

When adding or changing config fields or features, update these places:

- `src/cli.rs` - `long_about` schema in `--help`
- `README.md` - config schema and usage examples
- `../agent-config/skills/compose/SKILL.md` - global skill docs and examples
