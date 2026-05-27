# altair-otel

One-call OpenTelemetry setup for tokio applications. Sets up the `tracing` subscriber, OTLP exporters, and provides a `Meter` handle for metrics.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Add to your project

```bash
cargo add altair-otel
```

You do **not** need to add `opentelemetry`, `tracing`, or `tracing-subscriber` separately — `altair-otel` re-exports them.

## Quick start

```rust,no_run
use altair_otel::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Config::builder()
        .service_name("payments-api")
        .service_version("0.1.0")
        .otlp_endpoint("http://collector:4317")
        .build()
        .init()?;

    info!(user_id = 42, "request received");

    let counter = meter().u64_counter("requests.total").build();
    counter.add(1, &[]);

    shutdown();
    Ok(())
}
```

## From environment

```rust,no_run
use altair_otel::prelude::*;

# fn main() -> altair_otel::Result<()> {
Config::from_env()?.init()?;
# Ok(()) }
```

Honored env vars:

- `OTEL_SERVICE_NAME` (required)
- `OTEL_SERVICE_VERSION` (optional)
- `OTEL_EXPORTER_OTLP_ENDPOINT` (optional)
- `OTEL_LOG_FORMAT` — `pretty` (default) or `json`
- `RUST_LOG` — standard tracing filter (e.g., `info,altair_otel=debug`)

## Exporters

- `Exporter::Otlp` (default) — OTLP/gRPC via tonic
- `Exporter::Stdout` — local dev, no collector needed
- `Exporter::None` — disable exporters (useful in tests)

## Feature flags

- `otlp-grpc` (default) — OTLP over gRPC
- `otlp-http` — OTLP over HTTP/protobuf
- `console` — additional stdout exporter helpers

## License

[MIT](../../LICENSE)
