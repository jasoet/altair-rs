# altair-retry

Async retry with exponential backoff and automatic tracing.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Add to your project

```bash
cargo add altair-retry
```

## Quick start

```rust,no_run
use altair_retry::prelude::*;
use std::time::Duration;

# async fn ping() -> std::io::Result<()> { Ok(()) }
# async fn run() -> altair_retry::Result<()> {
let cfg = Config::builder()
    .name("db.connect")
    .max_retries(3)
    .initial_interval(Duration::from_millis(100))
    .build();

retry(cfg, || async { ping().await }).await?;
# Ok(()) }
```

## Permanent (non-retryable) errors

```rust,no_run
use altair_retry::prelude::*;

# async fn run() -> altair_retry::Result<()> {
retry(Config::default().with_name("api"), || async {
    if invalid_request() {
        return Err::<&'static str, _>(PermanentError::wrap("invalid input"));
    }
    do_call().await
}).await?;
# Ok(()) }
# fn invalid_request() -> bool { false }
# async fn do_call() -> Result<&'static str, std::io::Error> { Ok("ok") }
```

## Tracing

Each attempt runs inside a `tracing::span!("retry.attempt", retry.attempt = N)` span, nested under a top-level `retry` span with `retry.name` and `retry.max_attempts` attributes. Final outcome (`success`, `permanent`, `exhausted`) is emitted as a `tracing::info!`/`warn!` event with `retry.elapsed_ms` and `retry.attempts`.

If `altair-otel` is initialized, these spans flow to OTLP automatically.

## License

[MIT](../../LICENSE)
