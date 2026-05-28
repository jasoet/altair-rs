# altair-rest

Resilient HTTP client for Rust — built on `reqwest`, with retry and OpenTelemetry-aware tracing baked in.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Add to your project

```bash
cargo add altair-rest
```

The underlying `reqwest` and `reqwest-middleware` are re-exported — you don't need to add them separately.

## Quick start

```rust,no_run
use altair_rest::Client;

# async fn run() -> altair_rest::Result<()> {
let client = Client::builder()
    .base_url("https://api.example.com")?
    .bearer_token("secret-token")
    .build()?;

let response = client.get("/users/42").send().await?;
println!("{}", response.status());
# Ok(()) }
```

`Client` is cheap to clone and uses an internal connection pool — share one instance across your app.

## What you get out of the box

- **Retries** on transient failures (5xx, network errors, 408/429) — 3 attempts with exponential 100ms → 5s backoff by default.
- **Tracing** spans per request via `reqwest-tracing`. If [`altair-otel`](https://crates.io/crates/altair-otel) is initialized in the same process, those spans flow to OTLP automatically.
- **Sensible timeouts**: 30s total, 10s connect.
- **User-Agent**: `altair-rest/<version>` (override via `.user_agent(...)`).

## JSON helpers (the 80% case)

```rust,no_run
use altair_rest::Client;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)] struct User { id: u64, name: String }
#[derive(Serialize)]   struct NewUser { name: String }

# async fn run() -> altair_rest::Result<()> {
let client = Client::builder().base_url("https://api.example.com")?.build()?;

let user: User = client.get_json("/users/42").await?;
let created: User = client
    .post_json("/users", &NewUser { name: "alice".into() })
    .await?;
# let _ = (user, created);
# Ok(()) }
```

4xx/5xx responses surface as `Error::Http`, not as a misleading decode failure on an error page — `get_json`/`post_json` call `error_for_status()` before decoding.

## Configuration

```rust,no_run
use altair_rest::Client;
use std::time::Duration;

# async fn run() -> altair_rest::Result<()> {
let client = Client::builder()
    .base_url("https://api.example.com")?
    .timeout(Duration::from_secs(10))
    .connect_timeout(Duration::from_secs(3))
    .user_agent("my-app/2.0")
    .default_header("x-tenant", "acme")?
    .bearer_token("eyJhbGciOi...")
    .retry_max_attempts(5)
    .retry_initial_interval(Duration::from_millis(50))
    .retry_max_interval(Duration::from_secs(2))
    .enable_tracing(true)
    .build()?;
# let _ = client;
# Ok(()) }
```

## Disable retries

`retry_max_attempts(0)` disables built-in retries. Pair with `altair-retry` if you want a custom retry policy across the workspace:

```rust,no_run
use altair_rest::Client;

# async fn run() -> altair_rest::Result<()> {
let client = Client::builder().retry_max_attempts(0).build()?;
let response = client.get("https://api.example.com/users").send().await?;
# let _ = response;
# Ok(()) }
```

## Power-user escape hatches

```rust,no_run
use altair_rest::Client;

# async fn run() -> altair_rest::Result<()> {
// Tweak the underlying reqwest::ClientBuilder before middleware is added:
let client = Client::builder()
    .with_reqwest_builder(|b| b.cookie_store(true).http2_prior_knowledge())
    .build()?;
# let _ = client;
# Ok(()) }
```

`Client::inner()` returns the underlying `reqwest_middleware::ClientWithMiddleware` if you need it.

## Error reference

| Variant | When |
|---|---|
| `Error::Middleware` | Middleware chain failure (retry exhausted, network error) |
| `Error::Http` | Raw `reqwest::Error` from `error_for_status` or pre-middleware paths |
| `Error::Decode` | `serde_json` failed to deserialize a response body |
| `Error::Url` | Bad URL (in `base_url` or relative-path resolution) |
| `Error::InvalidHeader` | Header name or value rejected by `http` |

`reqwest::Error::is_timeout()` distinguishes timeout cases — surfaces as `Error::Http` and you call `err.is_timeout()` on it.

## License

[MIT](../../LICENSE)
