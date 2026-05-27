# altair-retry

Async retry with exponential backoff, jitter, cancellation, and per-attempt tracing.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Add to your project

```bash
cargo add altair-retry
```

`CancellationToken` is re-exported — no need to add `tokio-util` separately.

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

## All the knobs

```rust,no_run
use altair_retry::prelude::*;
use std::time::Duration;

# fn build_cfg() -> Config {
Config::builder()
    .name("checkout.api")            // appears in tracing spans + error messages
    .max_retries(5)                  // 5 retries after the initial call → 6 total attempts
    .initial_interval(Duration::from_millis(50))
    .max_interval(Duration::from_secs(10))  // cap exponential growth
    .multiplier(2.0)                 // 50ms → 100ms → 200ms → 400ms ...
    .jitter(true)                    // randomize delays to avoid thundering herd
    .build()
# }
```

Sensible defaults if you skip any: 5 retries, 100ms initial, 30s max, ×1.5 multiplier, jitter on.

## Permanent (non-retryable) errors

Wrap an error in `PermanentError` to short-circuit retry — e.g. a 4xx response that won't get better with another attempt:

```rust,no_run
use altair_retry::prelude::*;

# async fn run() -> altair_retry::Result<()> {
retry(Config::default().with_name("api"), || async {
    let response = make_request().await?;
    if response.status_code == 401 {
        // Bad credentials — no point retrying.
        return Err::<Response, _>(PermanentError::wrap("invalid credentials"));
    }
    Ok(response)
}).await?;
# Ok(()) }
# struct Response { status_code: u16 }
# async fn make_request() -> Result<Response, std::io::Error> { Ok(Response { status_code: 200 }) }
```

The retry returns `Error::Permanent { name, source }` — no further attempts are made.

## Cancellation

For graceful shutdown — pass a `CancellationToken` and any cancel signal aborts the retry loop:

```rust,no_run
use altair_retry::prelude::*;
use std::time::Duration;

# async fn flaky_call() -> Result<&'static str, std::io::Error> { Ok("ok") }
# async fn run() -> altair_retry::Result<()> {
let cancel = CancellationToken::new();

// Fire cancel from elsewhere (signal handler, timeout, parent task, ...)
tokio::spawn({
    let cancel = cancel.clone();
    async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        cancel.cancel();
    }
});

let cfg = Config::builder()
    .name("shutdown.sensitive")
    .max_retries(10)
    .initial_interval(Duration::from_secs(1))
    .cancellation_token(cancel)      // observed pre-attempt AND during backoff sleep
    .build();

match retry(cfg, || async { flaky_call().await }).await {
    Ok(v) => println!("success: {v}"),
    Err(Error::Cancelled { name }) => eprintln!("retry '{name}' cancelled"),
    Err(e) => eprintln!("other failure: {e}"),
}
# Ok(()) }
```

## Tracing output

Each attempt runs inside a `tracing::span!("retry.attempt", retry.attempt = N)` span, nested under a top-level `retry` span:

```text
INFO retry{retry.name=db.connect retry.max_attempts=4}:retry.attempt{retry.attempt=1}: retrying after backoff retry.delay_ms=100
INFO retry{retry.name=db.connect retry.max_attempts=4}:retry.attempt{retry.attempt=2}: retrying after backoff retry.delay_ms=150
INFO retry{retry.name=db.connect retry.max_attempts=4}:retry.attempt{retry.attempt=3}: retry succeeded retry.outcome=success retry.elapsed_ms=347 retry.attempts=3
```

Final outcome (`success`, `permanent`, `exhausted`, `cancelled`) emits as an event with `retry.elapsed_ms` and `retry.attempts` attributes.

If `altair-otel` is initialized in the same process, these spans flow to OTLP automatically — zero extra setup.

## Error reference

| Variant | When |
|---|---|
| `Error::Exhausted` | All retry attempts used up; final underlying error preserved as `source` |
| `Error::Permanent` | Operation returned `PermanentError::wrap(...)`; no more attempts made |
| `Error::Cancelled` | Provided `CancellationToken` fired |

## License

[MIT](../../LICENSE)
