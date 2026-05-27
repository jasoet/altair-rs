# altair-rs

Production-ready Rust utility crates with OpenTelemetry instrumentation. Spiritual successor to the Go [`github.com/jasoet/pkg`](https://github.com/jasoet/pkg) library — same problem space, idiomatic Rust APIs.

## Status

**v0.1.0** — first public release. APIs are stable within `0.x` (minor = breaking allowed, patch = additive).

See [`docs/specs/2026-05-27-altair-rs-starter-design.md`](docs/specs/2026-05-27-altair-rs-starter-design.md) for the full design.
See [`docs/porting-tracker.md`](docs/porting-tracker.md) for Go → Rust mapping status.

## Starter Set — v0.1.0

| Crate | Purpose | crates.io |
|---|---|---|
| [`altair-otel`](crates/altair-otel) | One-call OpenTelemetry setup | [![crate](https://img.shields.io/crates/v/altair-otel.svg)](https://crates.io/crates/altair-otel) |
| [`altair-config`](crates/altair-config) | Type-safe TOML config + env + validation | [![crate](https://img.shields.io/crates/v/altair-config.svg)](https://crates.io/crates/altair-config) |
| [`altair-retry`](crates/altair-retry) | Async retry with auto-tracing | [![crate](https://img.shields.io/crates/v/altair-retry.svg)](https://crates.io/crates/altair-retry) |
| [`altair-concurrent`](crates/altair-concurrent) | Type-safe parallel execution | [![crate](https://img.shields.io/crates/v/altair-concurrent.svg)](https://crates.io/crates/altair-concurrent) |

## Design Pillar

**"Add one crate, write app code — not glue code."** Each crate wraps best-in-class Rust libraries (`figment`, `validator`, `backon`, `tracing`, `opentelemetry`, `tokio`) with generous re-exports, smart defaults, and cross-crate auto-integration. The product is cross-crate consistency.

## License

[MIT](LICENSE)
