# altair-rs Architecture

Cross-crate patterns and conventions. Companion to the per-crate READMEs.

## Workspace Layout

All crates live under `crates/<name>/`. Each is self-contained: own `Cargo.toml`, `src/`, `tests/`, `examples/`, `README.md`. Workspace-shared dependencies are pinned in the root `Cargo.toml` `[workspace.dependencies]`.

## Cross-Crate Conventions

### Error Handling

- Every public function returns `crate::Result<T>`
- Each crate defines `pub enum Error` via `thiserror`
- `pub type Result<T> = std::result::Result<T, Error>` at crate root
- No shared "altair-error" crate — each owns its error vocabulary

### API Style

- Typed builders for non-trivial config: `Config::builder()...build()`
- Plain structs with `Default` for simple cases
- Free functions for the 80% case (e.g., `retry()`, `from_toml_str()`)
- Builder methods take owned values where ergonomic, references where mandatory

### Re-Exports

Each crate re-exports its key public types at the crate root:

```rust
// In altair-retry/src/lib.rs
pub use config::{Config, ConfigBuilder};
pub use error::{Error, Result, PermanentError};
pub use retry::retry;
```

Each crate also provides a `prelude` module with the most common imports:

```rust
// In altair-retry/src/prelude.rs
pub use crate::{retry, Config, PermanentError, Result};
pub use tokio_util::sync::CancellationToken;
```

### OTel Integration

- **Spans/logs**: via the global `tracing` subscriber. Crates emit `tracing::span!` and `tracing::info!`; whatever subscriber is installed receives them. `altair-otel` provides the canonical OTLP-wiring subscriber.
- **Metrics**: via explicit `Meter` handle. Components that need metrics accept a `Meter` in their builder or obtain one via `altair_otel::meter()`.

### Lints

Per-crate lint policy via `#![lints]` in `src/lib.rs`:

```rust
#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
```

### Testing

- Unit tests: inline `#[cfg(test)] mod tests` blocks
- Integration tests: `crates/<name>/tests/<topic>.rs`
- Doc tests: `///` examples; every public function must have at least one
- Examples: `crates/<name>/examples/<topic>.rs`, must compile and run

### Versioning

- All crates start at `0.1.0`
- Per-crate independent bumps managed by `release-plz` from Conventional Commit scopes
- `0.x`: minor = breaking allowed, patch = additive/fix
- MSRV bumps are minor, never patch
