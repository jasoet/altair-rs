# Project Instructions

<!-- AI: Read this file at the start of every session. Update it when conventions, -->
<!-- architecture, or key paths change. Also keep README.md in sync. -->

## Project Overview

`altair-rs` — Rust utility crates with OpenTelemetry instrumentation. Spiritual successor to the Go [`github.com/jasoet/pkg`](https://github.com/jasoet/pkg) library.

**Status:** Pre-alpha — design approved, no published crates yet.
**Starter set:** `altair-otel`, `altair-config`, `altair-retry`, `altair-concurrent`.
**MSRV:** Latest stable Rust.
**Edition:** 2024.
**Async runtime:** tokio (v0.x).

## ABSOLUTE RULE — Git Authorship

**NEVER add AI (Claude, Copilot, or any AI) as co-author, committer, or contributor in git commits.**
Only the user's registered email may appear in commits. This is company policy — commits with AI
authorship WILL BE REJECTED. Do not use `--author`, `Co-authored-by`, or any other mechanism to
attribute commits to AI. This applies to ALL commits, including those made by tools and subagents.

## Conventions

- **Node.js**: Always use `bun`/`bunx` (never node, npm, npx).
- **Commands**: Always use `task <name>` to run commands once the Taskfile is set up. If a command is important or repeated but has no task, suggest adding it to `Taskfile.yml`.
- **Brainstorming**: New topics or planning always start with the brainstorming skill first. If unsure, ask the user.
- **Superpowers**: Ensure superpowers skills are installed. Use TDD for implementation, systematic-debugging for bugs.
- **Commits**: Conventional Commits. Format: `<type>(<scope>): <description>`. Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`, `ci`. **Scope = crate name without `altair-` prefix** (e.g., `feat(otel): ...`, `fix(retry): ...`).
- **Branching**: New branch per change (`feat/...`, `fix/...`). PR with squash merge. Use `gh` for PR status and CI checks.
- **Nix**: All dev tools provided via `flake.nix` (once added). Use `task <name>` which wraps commands with `nix develop -c`. Prerequisites: Nix (with flakes), go-task (global via Homebrew).
- **Patterns**: Typed builders for non-trivial config; plain structs with `Default` for simple cases. `thiserror` for library errors; `anyhow` only in binaries/examples.
- **OTel integration**: Hybrid — spans/logs via global `tracing` subscriber (set up by `altair-otel`); metrics via explicit `Meter` handle obtained from `altair_otel::meter()`.
- **Re-exports**: Each crate re-exports underlying-library types consumers need, so users depend only on `altair-*` crates.
- **Self-maintaining docs**: Update `INSTRUCTION.md`, `README.md`, and per-crate `README.md` files when conventions change.

## Key Paths

| Path | Purpose |
|------|---------|
| `crates/<name>/` | Each Rust crate (self-contained: `src/`, `tests/`, `examples/`, `Cargo.toml`, `README.md`) |
| `crates/<name>/README.md` | Per-crate documentation — leads with complete recipes |
| `crates/<name>/examples/` | Per-crate runnable examples (`cargo run --example <name>`) |
| `docs/specs/` | Design specs (brainstorming output) |
| `docs/plans/` | Implementation plans (writing-plans output) |
| `docs/porting-tracker.md` | Go → Rust status table — kept current |
| `docs/architecture.md` | Cross-crate patterns & conventions (to be created) |
| `flake.nix` | Nix dev tool declarations (to be created) |
| `Taskfile.yml` | All project commands (to be created) |
| `INSTRUCTION.md` | AI dev context (this file) |
| `CLAUDE.md` | Critical rules / quick reference |
| `README.md` | Human documentation |

## Taskfile Commands (planned)

| Task | Description |
|------|-------------|
| `task test` | Unit tests (`cargo test --workspace --lib`) |
| `task test:integration` | Integration tests (`cargo test --workspace --tests`) |
| `task test:examples` | Build + run all examples |
| `task test:complete` | All tests with coverage (`cargo-llvm-cov`) |
| `task lint` | `cargo clippy --workspace --all-targets -- -D warnings` |
| `task fmt` | `cargo fmt --all` |
| `task fmt:check` | `cargo fmt --all --check` |
| `task doc` | `cargo doc --workspace --no-deps` |
| `task check` | test + lint + fmt:check |
| `task clean` | Remove build artifacts (`cargo clean`) |
| `task nix:check` | Verify Nix environment and tool availability |
| `task release` | `release-plz release` (CI only) |

## Testing Strategy

- **Unit tests**: inline `#[cfg(test)] mod tests` blocks, no special tagging
- **Integration tests**: `crates/<name>/tests/*.rs` — uses `testcontainers-modules` where needed
- **Doc tests**: `///` examples in source — bundled with `cargo test`
- **Examples-as-tests**: `crates/<name>/examples/*.rs` — must compile and run-to-completion
- **Coverage target**: 80%+ (matches Go project at 85%)

## Adding a New Crate

1. Create: `crates/<name>/` with `Cargo.toml`, `README.md`, `src/lib.rs`
2. Register crate in workspace root `Cargo.toml` `members` array
3. Follow conventions: typed builders, `Error` enum via `thiserror`, `Result<T>` alias, generous re-exports, `prelude` module, `#![deny(missing_docs)]`
4. Update: `README.md` crate table, `docs/porting-tracker.md` (move row from Deferred → Planned/In Progress)
5. Add per-crate examples in `crates/<name>/examples/`
