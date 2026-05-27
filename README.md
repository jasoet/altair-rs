# altair-rs

Production-ready Rust utility crates with OpenTelemetry instrumentation. Spiritual successor to the Go [`github.com/jasoet/pkg`](https://github.com/jasoet/pkg) library — same problem space, idiomatic Rust APIs.

## Status

**Pre-alpha** — design approved, implementation not yet started. No crates published to crates.io yet.

See [`docs/specs/2026-05-27-altair-rs-starter-design.md`](docs/specs/2026-05-27-altair-rs-starter-design.md) for the full design.
See [`docs/porting-tracker.md`](docs/porting-tracker.md) for Go → Rust mapping status.

## Starter Set (v0.1.0 planned)

| Crate | Purpose |
|---|---|
| `altair-otel` | One-call OpenTelemetry setup — tracing subscriber + OTLP exporters + Meter handle |
| `altair-config` | Type-safe TOML config with env overrides and validation |
| `altair-retry` | Async retry with exponential backoff, auto-traced |
| `altair-concurrent` | Type-safe parallel execution of named async tasks |

## Design Pillar

**"Add one crate, write app code — not glue code."** Each crate wraps best-in-class Rust libraries (`figment`, `validator`, `backon`, `tracing`, `opentelemetry`, `tokio`) with generous re-exports, smart defaults, and cross-crate auto-integration. The product is cross-crate consistency.

## License

[MIT](LICENSE)
