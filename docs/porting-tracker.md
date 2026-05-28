# Go → Rust Porting Tracker

Tracks the migration status of every package from [`github.com/jasoet/pkg`](https://github.com/jasoet/pkg) (Go) to its Rust equivalent in `altair-rs`.

**Last updated:** 2026-05-29 (altair-server 0.1.2 in flight)
**Reference Go version:** v2.13.0

## Published crates

All crates live on crates.io:

| Crate | Latest |
|---|---|
| [`altair-concurrent`](https://crates.io/crates/altair-concurrent) | 0.1.2 |
| [`altair-retry`](https://crates.io/crates/altair-retry) | 0.1.2 |
| [`altair-config`](https://crates.io/crates/altair-config) | 0.1.2 |
| [`altair-otel`](https://crates.io/crates/altair-otel) | 0.1.2 |
| [`altair-base32`](https://crates.io/crates/altair-base32) | 0.1.2 |
| [`altair-compress`](https://crates.io/crates/altair-compress) | 0.1.2 |
| [`altair-rest`](https://crates.io/crates/altair-rest) | 0.1.2 |
| [`altair-server`](https://crates.io/crates/altair-server) | 0.1.2 (date TBD on publish) |

**Release notes:**
- **0.1.0** (2026-05-27) — initial release of starter scaffolding
- **0.1.1** (2026-05-27) — wire `MeterProvider` and real `shutdown()` in `altair-otel`; accept `CancellationToken` in `altair-retry`; typed `PartialExecutor` for per-task results in `altair-concurrent`; coverage 85% → 89.6%
- **0.1.2** (2026-05-27) — expanded crate READMEs with examples; `altair-otel` re-exports `KeyValue`
- **`altair-base32` 0.1.2** (2026-05-28) — Crockford Base32 encode/decode for bytes and `u64`, plus Mod-37 check digit. Lenient decoding per spec. Published at workspace version 0.1.2 rather than 0.1.0 because it inherits `version.workspace`.
- **`altair-compress` 0.1.2** (2026-05-28) — Path-based recipes for gzip, tar, zip, and tar.gz with zip-slip protection. Re-exports `flate2`, `tar`, `zip` for power users.
- **`altair-rest` 0.1.2** (2026-05-28) — Resilient HTTP client built on `reqwest`. Built-in retries via `reqwest-retry` + tracing via `reqwest-tracing`. JSON helpers (`get_json`/`post_json`) for the 80% case.
- **`altair-server` 0.1.2** (date TBD on publish) — Axum + tower-http convenience layer with default middleware (tracing + request-id + timeout), built-in `/health` endpoint, and SIGINT/SIGTERM-aware graceful shutdown.

Next milestone: depends on real-world need. Most likely candidates from `Awaiting Demand`:
`altair-db` (sqlx), `altair-grpc` (tonic).

## Status Legend

| Symbol | Meaning |
|---|---|
| ✅ **Done** | Published to crates.io, stable |
| 🚧 **In Progress** | Branch open, not yet released |
| 📅 **Planned** | Committed roadmap item for current or next milestone |
| 💤 **Deferred** | Known scope, no current plan; revisit when demand appears |
| ❌ **Won't Port** | Intentionally not migrating (rationale in notes) |

## Starter Set — `v0.1.x`

| Go package | Rust crate | Status | Underlying libs | Notes |
|---|---|---|---|---|
| `otel` + `logging` | `altair-otel` | ✅ Done | `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`, `tracing-opentelemetry`, `tracing-subscriber` | Go's two packages merged — `tracing` is the unifier in Rust |
| `config` | `altair-config` | ✅ Done | `figment` (toml feature), `validator`, `serde`, `toml` | Thin wrapper; TOML-only (Rust ecosystem default) |
| `retry` | `altair-retry` | ✅ Done | `backon`, `tracing`, `tokio-util` | OTel via global `tracing` subscriber |
| `concurrent` | `altair-concurrent` | ✅ Done | `tokio`, `tokio-util`, `tracing` | Most original code in starter set (named keying over `JoinSet`) |
| `base32` | `altair-base32` | ✅ Done | `base32` (Crockford alphabet) + in-crate Mod-37 check | Crockford Base32 — small focused crate |
| `compress` | `altair-compress` | ✅ Done | `flate2`, `tar`, `zip` | Path-based recipes with zip-slip protection |
| `rest` | `altair-rest` | ✅ Done | `reqwest`, `reqwest-middleware`, `reqwest-retry`, `reqwest-tracing` | Resilient HTTP client with retry + tracing baked in |
| `server` | `altair-server` | ✅ Done | `axum`, `tower`, `tower-http` | Convenience layer with default middleware + health endpoint + graceful shutdown |

## Awaiting Demand

These have clear Rust equivalents and will be added when a project needs them.

| Go package | Likely Rust crate | Status | Underlying libs | Notes |
|---|---|---|---|---|
| `db` | `altair-db` | 💤 Deferred | `sqlx` (or `sea-orm`) | Migrations via `sqlx-cli` or `refinery` |
| `grpc` | `altair-grpc` | 💤 Deferred | `tonic`, `tower` | `tonic` = de-facto Rust gRPC |
| `ssh` | `altair-ssh` | 💤 Deferred | `russh` | Async pure-Rust SSH |
| `docker` | `altair-docker` | 💤 Deferred | `bollard`, `testcontainers` | `testcontainers-rs` is mature |

## At-Risk / Harder Ports

| Go package | Likely Rust crate | Status | Concern | Notes |
|---|---|---|---|---|
| `temporal` | `altair-temporal` | 💤 Deferred | Rust SDK is pre-1.0 | Wait for `temporal-sdk-core` Rust crates to stabilize before porting |
| `argo` | `altair-argo` | ❌ Won't Port (provisional) | No Rust Argo client | Would need to wrap `kube-rs` + raw Argo CRDs manually — large surface area for narrow use case; revisit if real need surfaces |

## Cross-Cutting Deferred Items

| Item | Status | Notes |
|---|---|---|
| Hot-reload config | 💤 Deferred | Future `altair-config-watch` separate crate to avoid `notify` dep on the base config crate |
| Heterogeneous concurrent tasks | ❌ Won't Port | Out of scope for `altair-concurrent`; users can call `tokio::join!` directly |
| Async-std / smol runtime support | ❌ Won't Port (v0.x) | Tokio-only until demand materializes |

## Maintenance Rules

- This file is updated as part of every crate release PR
- Status moves: `📅 Planned` → `🚧 In Progress` (branch open) → `✅ Done` (published to crates.io)
- "Underlying libs" column is the honesty receipt — confirms we're not reinventing wheels
- A new Go package added in the parent repo triggers a row added here within the same week (manual, no automation)
