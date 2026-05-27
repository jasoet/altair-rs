# Go → Rust Porting Tracker

Tracks the migration status of every package from [`github.com/jasoet/pkg`](https://github.com/jasoet/pkg) (Go) to its Rust equivalent in `altair-rs`.

**Last updated:** 2026-05-27
**Reference Go version:** v2.13.0

## Status Legend

| Symbol | Meaning |
|---|---|
| ✅ **Done** | Published to crates.io, stable |
| 🚧 **In Progress** | Branch open, not yet released |
| 📅 **Planned** | Committed roadmap item for current or next milestone |
| 💤 **Deferred** | Known scope, no current plan; revisit when demand appears |
| ❌ **Won't Port** | Intentionally not migrating (rationale in notes) |

## Starter Set — `v0.1.0`

| Go package | Rust crate | Status | Underlying libs | Notes |
|---|---|---|---|---|
| `otel` + `logging` | `altair-otel` | 🚧 In Progress | `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`, `tracing-opentelemetry`, `tracing-subscriber` | Go's two packages merged — `tracing` is the unifier in Rust |
| `config` | `altair-config` | 🚧 In Progress | `figment` (toml feature), `validator`, `serde`, `toml` | Thin wrapper; TOML-only (Rust ecosystem default) |
| `retry` | `altair-retry` | 🚧 In Progress | `backon`, `tracing`, `tokio-util` | OTel via global `tracing` subscriber |
| `concurrent` | `altair-concurrent` | 🚧 In Progress | `tokio`, `tokio-util`, `tracing` | Most original code in starter set (named keying over `JoinSet`) |

## Awaiting Demand

These have clear Rust equivalents and will be added when a project needs them.

| Go package | Likely Rust crate | Status | Underlying libs | Notes |
|---|---|---|---|---|
| `db` | `altair-db` | 💤 Deferred | `sqlx` (or `sea-orm`) | Migrations via `sqlx-cli` or `refinery` |
| `server` | `altair-server` | 💤 Deferred | `axum`, `tower`, `tower-http` | Closest Rust analog to Echo |
| `grpc` | `altair-grpc` | 💤 Deferred | `tonic`, `tower` | `tonic` = de-facto Rust gRPC |
| `rest` | `altair-rest` | 💤 Deferred | `reqwest`, `reqwest-middleware`, `reqwest-retry` | Resilient HTTP client |
| `compress` | `altair-compress` | 💤 Deferred | `flate2`, `tar`, `zip` | Direct equivalents exist |
| `base32` | `altair-base32` | 💤 Deferred | (custom impl, possibly `data-encoding` for primitives) | Crockford Base32 — small focused crate |
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
