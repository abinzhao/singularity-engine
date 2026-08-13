# P0 Evidence Summary

## Current State

- P0 protocol foundation crates and fixtures are present in the Cargo workspace.
- ADR-001 is recorded in `docs/adr/0001-modular-monolith.md`.
- CI defines separate `fmt`, `clippy`, `unit`, `protocol-compat`, and `architecture` jobs with read-only repository contents permission.
- `cargo xtask architecture` checks Cargo workspace dependency direction against ADR-001 rules.

## Commands Run

| Command | Result |
|---|---|
| `PATH="$HOME/.cargo/bin:$PATH" cargo test -p xtask` | Failed before implementation: missing `WorkspacePackage` and `architecture_violations`, confirming the new tests covered absent behavior. |
| `PATH="$HOME/.cargo/bin:$PATH" cargo test -p xtask` | Passed after implementation: 4 xtask unit tests passed. |
| `PATH="$HOME/.cargo/bin:$PATH" cargo test -p sge-protocol` | Passed: protocol fixture, schema drift, and doctest checks passed. |

## P0 Gate Results

| Command | Result |
|---|---|
| `PATH="$HOME/.cargo/bin:$PATH" cargo fmt --all -- --check` | Passed. |
| `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings` | Passed. |
| `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace` | Passed. |
| `PATH="$HOME/.cargo/bin:$PATH" cargo xtask architecture` | Passed: workspace dependency direction matches ADR-001 rules. |
| `git diff --check` | Passed. |
