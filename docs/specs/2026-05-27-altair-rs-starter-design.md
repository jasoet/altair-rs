# altair-rs — Starter Design

**Date:** 2026-05-27
**Status:** Draft — awaiting review before implementation planning
**Author:** Jasoet
**Spec type:** Brainstorming output → input to writing-plans

---

## 1. Overview

`altair-rs` is the spiritual successor of the Go [`github.com/jasoet/pkg`](https://github.com/jasoet/pkg) utility library, redesigned for idiomatic Rust. It is **not** a 1:1 port: APIs, error handling, async story, and module boundaries are adapted to fit Rust ecosystem conventions.

**One-line product goal:** "Add one crate, write app code — not glue code."

Where the Go library wraps existing best-in-class libraries (viper, validator, GORM, zerolog, Echo, etc.) to provide a consistent surface across packages, `altair-rs` does the same for the Rust ecosystem (figment, validator, backon, tracing, opentelemetry, tokio, etc.). The product is the **cross-crate consistency**, not the underlying functionality.

## 2. Decisions Locked

| Decision | Choice |
|---|---|
| Strategy | Spiritual successor — idiomatic Rust, not a 1:1 port |
| Distribution | Published to crates.io; stable APIs + docs mandatory; support optional |
| Scope (v0.1.0) | Starter set: `altair-otel`, `altair-config`, `altair-retry`, `altair-concurrent` |
| Crate naming | `altair-*` prefix (all starter names verified available on crates.io 2026-05-27) |
| Repo | New repo `altair-rs` at `~/Documents/Rust/altair-rs/` |
| Async runtime | tokio-only for v0.x (revisit only if demand materializes) |
| MSRV | Latest stable Rust |
| Edition | 2024 |
| Errors | `thiserror` for library APIs; `anyhow` allowed in binaries/examples only |
| License | MIT (matches Go project) |
| Build tooling | Nix flake + Taskfile + GitHub Actions (mirrors Go project) |
| OTel integration | **Hybrid C** — tracing/logs via global `tracing` subscriber, metrics via explicit `Meter` handle |

## 3. Architecture

### 3.1 Workspace Layout

```
altair-rs/
├── Cargo.toml              # workspace root (shared [workspace.dependencies])
├── flake.nix               # Nix dev env (mirrors Go pattern)
├── Taskfile.yml            # task commands
├── .envrc                  # direnv (optional convenience)
├── .github/workflows/      # CI: ci.yml, release.yml, docs.yml
├── docs/
│   ├── specs/              # design specs (this document lives here)
│   ├── plans/              # implementation plans (created by writing-plans)
│   ├── porting-tracker.md  # Go → Rust status table
│   └── architecture.md     # cross-crate patterns & conventions
├── crates/
│   ├── altair-otel/
│   ├── altair-config/
│   ├── altair-retry/
│   └── altair-concurrent/
├── INSTRUCTION.md          # AI dev context for this project
├── CLAUDE.md               # Project rules (incl. absolute git authorship rule)
├── README.md
├── LICENSE                 # MIT
└── CHANGELOG.md            # per-crate changelogs managed by release-plz
```

Each crate is self-contained:

```
crates/<name>/
├── Cargo.toml
├── README.md               # complete recipes, not API reference
├── src/
│   └── lib.rs
├── tests/                  # integration tests
└── examples/               # runnable examples
```

### 3.2 Cross-Crate Conventions

- **Workspace-shared deps**: every crate uses the same pinned versions of `tokio`, `tracing`, `thiserror`, `serde`, `opentelemetry`, etc. via `[workspace.dependencies]`. Reduces drift, makes upgrades atomic.
- **API style**: typed builders for non-trivial config (`Config::builder()...build()`); plain structs with `Default` for simple cases.
- **Errors**: each crate exposes `pub enum Error` via `thiserror` and `pub type Result<T> = std::result::Result<T, Error>`. **No shared `altair-error` crate** — each crate owns its error vocabulary.
- **Public re-exports**: each crate re-exports its key public types at the crate root so consumers use `use altair_retry::{Config, retry};` not deep paths.
- **Lints**: `#![deny(missing_docs)]` and `#![warn(clippy::pedantic)]` per crate. Docs are mandatory; pedantic warnings are guidance.
- **Tests**: unit tests inline (`#[cfg(test)] mod tests`), integration tests in `tests/`, examples in `examples/`.
- **Versioning**: all crates start at `0.1.0`, independently bumped via `release-plz`.

### 3.3 Design Philosophy — "One Dependency, Everything You Need"

When a user adds `altair-config` to their `Cargo.toml`, they must be able to do everything config-related **without** also adding `figment`, `validator`, or `serde_yaml`. Same for every other crate.

Concrete implications:

1. **Generous re-exports** of underlying library types and derives:
   - `altair-config` re-exports `validator::{Validate, ValidationError}` and `serde::{Deserialize, Serialize}`
   - `altair-retry` re-exports `backon` backoff strategies and `tokio_util::sync::CancellationToken`
   - `altair-otel` re-exports `tracing::{info, warn, error, instrument, span, Span}` and `opentelemetry::metrics::{Counter, Histogram, UpDownCounter, Meter}`
   - `altair-concurrent` re-exports `tokio_util::sync::CancellationToken`

2. **Common-case helpers over flexibility-first APIs.** Expose 2–3 knobs with sensible defaults; provide an `escape-hatch` constructor (`Config::from_raw(...)`) for advanced use.

3. **Cross-crate auto-integration.** Adding `altair-otel` + `altair-retry` means retries are automatically traced — zero configuration. `altair-retry` reads from `tracing` globals; `altair-otel` sets them up.

4. **A `prelude` module per crate**: `use altair_retry::prelude::*;` brings in common types.

5. **Documentation is "complete recipes"**, not API reference. Each crate README leads with a 5-line snippet for the 80% case.

6. **Wrap underlying-library errors and types in our public surface**. Consumers should not need to learn `figment` to understand a config error.

**Acknowledged trade-offs:**

- Re-exporting transitive types means an upgrade to a wrapped library (e.g., `validator` 0.17 → 0.18) can be a breaking change for consumers even with no logic change. Mitigated by pinned `[workspace.dependencies]` and thoughtful upgrade cadence.
- Larger surface area to maintain; small extra docs burden.

## 4. Per-Crate Design

### 4.1 `altair-otel`

**Purpose:** One-call OpenTelemetry setup. Provides the global tracing subscriber wire-up (spans + logs) and a `Meter` handle for explicit metric instrumentation. **Subsumes the Go `otel` and `logging` packages** — in Rust, `tracing` is the unifier.

**Public API surface:**

```rust
// Setup (called once in main)
altair_otel::Config::from_env()?.init()?;          // OTEL_* env vars
altair_otel::Config::builder()
    .service_name("payments-api")
    .service_version("0.1.0")
    .otlp_endpoint("http://collector:4317")
    .resource_attribute("env", "prod")
    .build()
    .init()?;

// Get a Meter for components that need explicit metrics
let meter = altair_otel::meter();
let counter = meter.u64_counter("requests.total").init();

// Graceful shutdown — flushes pending spans/metrics
altair_otel::shutdown();
```

**What `init()` does:**

1. Builds `opentelemetry_sdk` `TracerProvider`, `MeterProvider`, `LoggerProvider` from config
2. Sets them as OTel globals
3. Installs a `tracing_subscriber::Registry` with `tracing_opentelemetry::layer()` so any `tracing::info!` / `#[instrument]` automatically becomes OTLP spans
4. Adds a `fmt::Layer` for stdout (pretty in dev, JSON in prod — toggle via config)

**Key dependencies:** `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`, `tracing-opentelemetry`, `tracing-subscriber`.

**Feature flags:**
- `otlp-grpc` (default) — OTLP over gRPC via `tonic`
- `otlp-http` — OTLP over HTTP
- `console` — adds a stdout exporter for local dev (no collector needed)

### 4.2 `altair-config`

**Purpose:** Type-safe YAML config loading with env-var overrides and validation. Thin convenience wrapper over `figment` + `validator`.

**Public API surface:**

```rust
use altair_config::{Validate, Deserialize};      // re-exported

#[derive(Debug, Deserialize, Validate)]
struct AppConfig {
    #[validate(range(min = 1, max = 65535))]
    port: u16,
    database: DbConfig,
}

let cfg: AppConfig = altair_config::from_yaml_str(YAML, "APP")?;
let cfg: AppConfig = altair_config::from_file("config.yaml", "APP")?;

// Layered explicit loader for multi-source scenarios
let cfg: AppConfig = altair_config::Loader::new()
    .yaml_file("base.yaml")
    .yaml_file_optional("local.yaml")
    .env_prefix("APP")
    .build()?;
```

**Behavior:**

- YAML deserialization via `serde_yaml`
- Env overrides: `APP_DATABASE_HOST=db.prod` sets `cfg.database.host`
- Validation runs automatically before returning
- Validation failures → typed `Error::Validation { field, message }`
- Env parse failures → `Error::EnvParse { key, source }`

**Key dependencies:** `figment`, `validator`, `serde`, `serde_yaml`.

**Intentionally NOT included (yet):** hot-reload — deferred to a future `altair-config-watch` separate crate to avoid the `notify` dep tax.

### 4.3 `altair-retry`

**Purpose:** Async retry with exponential backoff. Each attempt is automatically traced via the global `tracing` subscriber (set up by `altair-otel` if present).

**Public API surface:**

```rust
use altair_retry::{retry, Config, PermanentError};

let result = retry(
    Config::builder()
        .name("db.connect")
        .max_retries(3)
        .initial_interval(Duration::from_millis(100))
        .build(),
    || async { db.ping().await }
).await?;

// Mark non-retryable errors
retry(Config::default().with_name("api.call"), || async {
    match api.call().await {
        Ok(v) => Ok(v),
        Err(e) if e.is_client_error() => Err(PermanentError::wrap(e).into()),
        Err(e) => Err(e.into()),
    }
}).await?;
```

**Behavior:**

- Exponential backoff + jitter (defaults: 100ms initial, 1.5× multiplier, max 30s, max 5 retries)
- Each attempt wrapped in `tracing::span!("retry.attempt", name = ..., attempt = ...)`
- Span attributes: `retry.name`, `retry.attempt`, `retry.max_attempts`, `retry.elapsed_ms`, `retry.outcome`
- `PermanentError` short-circuits retry
- Honors a passed `CancellationToken` for graceful shutdown

**Key dependencies:** `backon`, `tracing`, `tokio`, `tokio-util`, `thiserror`.

### 4.4 `altair-concurrent`

**Purpose:** Type-safe parallel execution of named async tasks with aggregated results.

**Public API surface:**

```rust
use altair_concurrent::{execute_concurrently, TaskMap};

let tasks: TaskMap<String> = TaskMap::new()
    .insert("fetch_user", |_ctx| async { fetch_user(42).await })
    .insert("fetch_orders", |_ctx| async { fetch_orders(42).await })
    .insert("fetch_prefs", |_ctx| async { fetch_prefs(42).await });

let results: HashMap<&'static str, String> = execute_concurrently(tasks).await?;

// With cancellation + timeout
let results = execute_concurrently(tasks)
    .with_cancellation(token)
    .with_timeout(Duration::from_secs(5))
    .with_partial_results()           // return per-task Result instead of fail-fast
    .await?;
```

**Behavior:**

- All tasks start concurrently via `tokio::task::JoinSet`
- Per-task `tracing::span!("concurrent.task", name = ...)` and aggregate `tracing::span!("concurrent.batch", task_count = ...)`
- Fail-fast by default: first error cancels remaining tasks via cancellation token
- `with_partial_results()` switches to "run all, return per-task Results"
- All tasks must return the same `Result<T, E>` (heterogeneous batches are out-of-scope — use `tokio::join!` directly)

**Key dependencies:** `tokio`, `tokio-util`, `tracing`, `thiserror`.

**Honest disclaimer:** most original code in the starter set (~200–300 LOC). Value-add is named keying, span auto-instrumentation, cancellation handling, error aggregation. Without these, `tokio::task::JoinSet` covers the underlying mechanics.

## 5. Testing, CI, and Release Strategy

### 5.1 Testing

| Layer | Location | Run via |
|---|---|---|
| Unit | inline `#[cfg(test)] mod tests` | `task test` (`cargo test --workspace --lib`) |
| Integration | `crates/<name>/tests/*.rs` | `task test:integration` (`cargo test --workspace --tests`) |
| Examples-as-tests | `crates/<name>/examples/*.rs` | `task test:examples` (build + run) |
| Doc tests | `///` examples in source | bundled with `cargo test` |

**Coverage:** `cargo-llvm-cov` (target 80%+, matching Go project's 85%).

**Convention:** every public API function has at least one doc-test example.

### 5.2 CI (`.github/workflows/`)

1. **`ci.yml`** (on PR + push to main):
   - `cargo fmt --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace`
   - `cargo doc --workspace --no-deps` (with `-D rustdoc::broken-intra-doc-links`)
   - MSRV check
2. **`release.yml`** (manual or label-triggered, uses `release-plz`):
   - Detect changed crates
   - Bump versions per Conventional Commits
   - Generate per-crate changelogs
   - Publish to crates.io in dependency order
3. **`docs.yml`** (on tag): deploy `cargo doc` output to GitHub Pages

**Runner:** `ubuntu-latest` primary; macOS/Windows added if cross-platform issues surface.

### 5.3 Release & Versioning

- **`release-plz`** — Rust's analog of `semantic-release`
- Per-crate independent version bumps
- Per-crate `CHANGELOG.md`
- crates.io publish order respects dep graph (`altair-otel` first since others reference its globals conceptually, though no compile-time dep)
- All crates begin at `0.1.0`; `0.x` allows breaking on minor, additive on patch
- Promote to `1.0.0` after 3–6 months of real use
- MSRV bumps are minor version, never patch

**Conventional Commits scope = crate name without prefix**: `feat(otel): add OTLP HTTP exporter`, `fix(retry): correct jitter calculation`.

### 5.4 Quality Gates

- Clippy `pedantic` warnings on (treated as guidance; `#[allow(...)]` with rationale comments where needed)
- `#![deny(missing_docs)]` per crate
- `#![forbid(unsafe_code)]` per starter crate (none of these need `unsafe`)
- `cargo-deny` in CI for advisories + license compliance

## 6. Porting Tracker

Lives at [`docs/porting-tracker.md`](../porting-tracker.md). Source of truth for Go → Rust status across all 15 Go packages. Updated as part of every release PR.

## 7. Risks & Open Questions

| Item | Risk | Mitigation |
|---|---|---|
| Transitive-type re-exports cause unintended breaking changes | Medium | Pin underlying versions in `[workspace.dependencies]`; document which re-exports are "stable surface" |
| `altair-concurrent` overlaps heavily with `tokio::task::JoinSet` | Low (acknowledged) | Be honest in README: it's an ergonomic layer; users who prefer raw tokio can skip the crate |
| OTel Rust SDK is less mature than Go's | Medium | Pin to opentelemetry 0.27+; track upstream stabilization; absorb breaking changes as our own minor bumps |
| Latest-stable MSRV blocks consumers on older toolchains | Low | Acceptable for self-use; revisit MSRV policy if any external user reports it |
| `release-plz` learning curve vs Go's semantic-release | Low | Both are config-driven Conventional Commits tools; similar mental model |

## 8. Out of Scope (v0.1.0)

Explicitly **not** included in the starter release:

- `altair-db`, `altair-server`, `altair-grpc`, `altair-rest` — added on demand
- `altair-temporal`, `altair-argo` — waiting on ecosystem maturity
- Hot-reload config — `altair-config-watch` as a future separate crate
- Heterogeneous-typed concurrent tasks — use raw `tokio::join!`
- Async-std / smol runtime support — tokio-only for v0.x

## 9. Next Steps

1. **User reviews this spec** (current step)
2. On approval: `writing-plans` skill produces an implementation plan
3. Implementation plan drives: workspace scaffolding → per-crate implementation → testing → CI → first crates.io publish
