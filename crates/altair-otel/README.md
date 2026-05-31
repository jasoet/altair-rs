# altair-otel

One-call OpenTelemetry setup for tokio applications. Wires the `tracing` subscriber to OTLP exporters and gives you a `Meter` handle for explicit metrics. Spans + logs + metrics, configured once.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Add to your project

```bash
cargo add altair-otel
```

You do **not** need to add `opentelemetry`, `tracing`, or `tracing-subscriber` separately — `altair-otel` re-exports the types and macros you'll use.

## Quick start — production with OTLP collector

```rust,no_run
use altair_otel::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Config::builder()
        .service_name("payments-api")
        .service_version("0.1.0")
        .otlp_endpoint("http://collector:4317")
        .resource_attribute("env", "prod")
        .build()
        .init()?;

    info!(user_id = 42, "request received");

    let counter = meter().u64_counter("requests.total").build();
    counter.add(1, &[]);

    shutdown();
    Ok(())
}
```

After `init()`, **all** `tracing::info!`, `tracing::warn!`, `#[instrument]`, etc. emit OTLP spans + logs to the collector. No further wiring needed in your app code.

## Local dev — stdout exporter, no collector required

```rust,no_run
use altair_otel::prelude::*;
use altair_otel::Exporter;

# fn main() -> altair_otel::Result<()> {
Config::builder()
    .service_name("my-app")
    .exporter(Exporter::Stdout)      // prints spans + metrics to stdout
    .build()
    .init()?;
# Ok(()) }
```

## Config from environment variables

For 12-factor-style deployment — read everything from env:

```rust,no_run
use altair_otel::prelude::*;

# fn main() -> altair_otel::Result<()> {
Config::from_env()?.init()?;
# Ok(()) }
```

Honored env vars:

| Variable | Purpose | Default |
|---|---|---|
| `OTEL_SERVICE_NAME` | Service name (required) | — |
| `OTEL_SERVICE_VERSION` | Service version | unset |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OTLP collector URL | `http://localhost:4317` |
| `OTEL_LOG_FORMAT` | `pretty` or `json` log output | `pretty` |
| `RUST_LOG` | Standard tracing filter | `info` |

## Instrumented async functions

```rust,no_run
use altair_otel::prelude::*;

#[instrument(skip(db), fields(user.id = id))]
async fn fetch_user(db: &Database, id: u64) -> Result<User, std::io::Error> {
    info!("fetching user");
    db.find_user(id).await
}
# struct Database; struct User;
# impl Database { async fn find_user(&self, _: u64) -> Result<User, std::io::Error> { Ok(User) } }
```

Calling `fetch_user(db, 42)` produces a span named `fetch_user` with attribute `user.id=42`, plus a nested `info` event for `"fetching user"`. If the function errors, the span carries the error status automatically.

## Metrics — all three instrument types

```rust,no_run
use altair_otel::prelude::*;

# fn record() {
let m = meter();

// Counter — monotonic, only goes up (e.g. total requests)
let requests = m.u64_counter("http.requests.total").build();
requests.add(1, &[KeyValue::new("route", "/checkout"), KeyValue::new("status", 200_i64)]);

// UpDownCounter — can go up or down (e.g. in-flight connections)
let in_flight = m.i64_up_down_counter("http.in_flight").build();
in_flight.add(1, &[]);    // request started
in_flight.add(-1, &[]);   // request finished

// Histogram — distributions (e.g. latency)
let latency = m.f64_histogram("http.latency.seconds").build();
latency.record(0.124, &[KeyValue::new("route", "/checkout")]);
# }
```

## Graceful shutdown pattern

`shutdown()` flushes pending spans and metrics — call it once before the process exits:

```rust,no_run
use altair_otel::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Config::from_env()?.init()?;

    // ... your app does work, emits spans + metrics ...

    // Wait for shutdown signal (Ctrl-C, SIGTERM, etc.)
    tokio::signal::ctrl_c().await?;

    // Drain any pending telemetry to the collector before exiting.
    shutdown();
    Ok(())
}
```

`shutdown()` is idempotent — safe to call multiple times. Subsequent calls are no-ops.

## Exporters

| Variant | Use case |
|---|---|
| `Exporter::Otlp` (default) | Production. OTLP/gRPC via `tonic`, points to a collector |
| `Exporter::Stdout` | Local dev / debugging. Prints spans + metrics to stdout |
| `Exporter::None` | Tests / disabling telemetry. No-op exporter |

## Feature flags

| Feature | Effect |
|---|---|
| `otlp-grpc` (default) | OTLP over gRPC via `tonic` |
| `otlp-http` | OTLP over HTTP/protobuf via `reqwest` |
| `console` | Additional stdout exporter helpers |

## Error reference

| Variant | When |
|---|---|
| `Error::Exporter` | The OTLP exporter couldn't be constructed (bad endpoint, missing transport) |
| `Error::AlreadyInitialized` | `init()` called more than once in the same process |
| `Error::EnvConfig` | `from_env()` saw a missing/invalid required env var |

## Integration testing with `OtelCollectorContainer`

Spin up a real OpenTelemetry Collector in your own integration tests via
the `testcontainers` feature:

```toml
[dev-dependencies]
altair-otel = { version = "0.1", features = ["testcontainers"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use altair_otel::testcontainer::OtelCollectorContainer;
use altair_otel::{Config, Exporter};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn my_app_exports_traces() {
    let collector = OtelCollectorContainer::start().await.unwrap();
    let cfg = Config::builder()
        .service_name("my-app")
        .otlp_endpoint(collector.grpc_endpoint())
        .exporter(Exporter::Otlp)
        .build();
    cfg.init().unwrap();

    tracing::info!("hello collector");
    altair_otel::shutdown();
}
```

Pulls `otel/opentelemetry-collector:latest` (default config exposes OTLP
gRPC on 4317 + HTTP on 4318). Container starts in ~1s with the image
cached. Drop the `OtelCollectorContainer` handle to stop the container.

Override the image, tag, or startup timeout via
`OtelCollectorContainer::builder()`.

Run this crate's own collector-backed integration test:

```bash
task test:integration:otel
```

## License

[MIT](../../LICENSE)
