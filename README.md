# altair-rs

Production-ready Rust utility crates for tokio applications. Each crate wraps a best-in-class library (`figment`, `validator`, `backon`, `tracing`, `opentelemetry`, `tokio`) with smart defaults, typed errors, generous re-exports, and a `prelude` module — so adding one crate gives you everything you need without pulling in five more.

## Crates

| Crate | Purpose | crates.io |
|---|---|---|
| [`altair-otel`](crates/altair-otel) | One-call OpenTelemetry setup — `tracing` subscriber, OTLP exporters, `Meter` handle | [![crate](https://img.shields.io/crates/v/altair-otel.svg)](https://crates.io/crates/altair-otel) |
| [`altair-config`](crates/altair-config) | Type-safe TOML config with env-var overrides and validation | [![crate](https://img.shields.io/crates/v/altair-config.svg)](https://crates.io/crates/altair-config) |
| [`altair-retry`](crates/altair-retry) | Async retry with exponential backoff and per-attempt tracing | [![crate](https://img.shields.io/crates/v/altair-retry.svg)](https://crates.io/crates/altair-retry) |
| [`altair-concurrent`](crates/altair-concurrent) | Type-safe parallel execution of named async tasks | [![crate](https://img.shields.io/crates/v/altair-concurrent.svg)](https://crates.io/crates/altair-concurrent) |
| [`altair-base32`](crates/altair-base32) | Crockford Base32 — bytes, u64 IDs, optional check digit | [![crate](https://img.shields.io/crates/v/altair-base32.svg)](https://crates.io/crates/altair-base32) |
| [`altair-compress`](crates/altair-compress) | gzip + tar + zip + tar.gz recipes with zip-slip protection | [![crate](https://img.shields.io/crates/v/altair-compress.svg)](https://crates.io/crates/altair-compress) |
| [`altair-rest`](crates/altair-rest) | Resilient HTTP client — reqwest + retry + tracing baked in | [![crate](https://img.shields.io/crates/v/altair-rest.svg)](https://crates.io/crates/altair-rest) |
| [`altair-server`](crates/altair-server) | Axum + tower-http convenience layer with sensible defaults | [![crate](https://img.shields.io/crates/v/altair-server.svg)](https://crates.io/crates/altair-server) |

Pick one, several, or all four — each is usable standalone. Pair `altair-otel` with any of the others and tracing flows automatically (spans + metrics).

## Versioning

Pre-1.0: minor bumps allow breaking changes, patch bumps are additive. Promotion to 1.0.0 after real-world use stabilizes the APIs.

## License

[MIT](LICENSE)
