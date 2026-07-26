# AGENTS.md

## Cursor Cloud specific instructions

`agm` is a single Rust CLI binary (Agent Manager). There are no services, servers, or databases — everything is run locally through `cargo`.

### Toolchain caveat (important)

`Cargo.toml` sets `edition = "2024"`, which requires Rust **1.85+**. The base VM image ships an older default toolchain (1.83.0) that cannot parse the manifest (`feature edition2024 is required`). The startup update script installs and defaults to the `stable` toolchain, so a fresh session should already be on a compatible `rustc`. If you ever see the `edition2024` error, run `rustup default stable`.

### Common commands

Standard Cargo workflow (dependencies are declared in `Cargo.toml` / `Cargo.lock`):

- Build (dev): `cargo build`
- Run: `cargo run -- <args>` (e.g. `cargo run -- --help`, `cargo run -- init`)
- Lint: `cargo clippy --all-targets`
- Format check: `cargo fmt --check`
- Test: `cargo test`

### Notes

- `cargo run -- init` writes an `agm.json` config file into the current working directory. It is not git-ignored, so delete it after manual testing to keep the tree clean.
- `cargo clippy` currently emits a `dead_code` warning for `load_config` — this is expected on the current codebase, not an error.
- The `skill list` and `mcp list` subcommands are stubbed with `todo!()` and will panic if invoked; only `init` is fully implemented so far.
