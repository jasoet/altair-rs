# altair-server

Axum + tower-http convenience layer with sensible defaults and graceful shutdown.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Add to your project

```bash
cargo add altair-server
```

The underlying `axum`, `tower`, and `tower-http` are re-exported — you don't need to add them separately.

## Quick start

```rust,no_run
use altair_server::Server;
use altair_server::axum::routing::get;

#[tokio::main]
async fn main() -> altair_server::Result<()> {
    Server::builder()
        .bind_addr("0.0.0.0:3000")
        .route("/", get(|| async { "hello" }))
        .build()
        .await?
        .run()
        .await
}
```

`Server::run()` binds the listener, applies the default middleware stack, and serves until SIGINT (Ctrl-C) or SIGTERM. Returns `Ok(())` after graceful shutdown completes.

## What you get out of the box

- **Tracing** per request via `tower_http::trace::TraceLayer`. If [`altair-otel`](https://crates.io/crates/altair-otel) is initialized in the same process, those spans flow to OTLP automatically.
- **Request ID** (`x-request-id`) propagation — generated if missing, echoed in response.
- **Per-request timeout** (default 30s, configurable).
- **`GET /health` endpoint** returning `200 OK` (customizable path + body).
- **Graceful shutdown** on SIGINT/SIGTERM via `tokio::signal`.

## Routes

`.route()`, `.merge()`, and `.nest()` delegate directly to axum:

```rust,no_run
use altair_server::Server;
use altair_server::axum::Router;
use altair_server::axum::routing::get;

# async fn run() -> altair_server::Result<()> {
let api: Router = Router::new()
    .route("/users", get(list_users).post(create_user));

Server::builder()
    .route("/", get(|| async { "home" }))
    .nest("/api", api)
    .build()
    .await?
    .run()
    .await
# }
# async fn list_users() -> &'static str { "users" }
# async fn create_user() -> &'static str { "created" }
```

## Configuration

```rust,no_run
use altair_server::Server;
use std::time::Duration;

# async fn run() -> altair_server::Result<()> {
Server::builder()
    .bind_addr("0.0.0.0:3000")
    .request_timeout(Duration::from_secs(10))
    .enable_cors()              // CorsLayer::permissive()
    .enable_compression()       // gzip/br/zstd response encoding
    .health_path("/healthz")    // override "/health"
    .disable_tracing()          // skip the default TraceLayer
    .build()
    .await?
    .run()
    .await
# }
```

## Custom CORS

```rust,no_run
use altair_server::Server;
use altair_server::tower_http::cors::CorsLayer;
use altair_server::axum::http::Method;

# async fn run() -> altair_server::Result<()> {
let cors = CorsLayer::new()
    .allow_methods([Method::GET, Method::POST])
    .allow_origin(["https://app.example.com".parse().unwrap()])
    .allow_credentials(true);

Server::builder().enable_cors_with(cors).build().await?.run().await
# }
```

## Custom health response

```rust,no_run
use altair_server::Server;
use altair_server::axum::Json;
use serde_json::json;

# async fn run() -> altair_server::Result<()> {
Server::builder()
    .health_response(|| Json(json!({"status": "ok"})))
    .build()
    .await?
    .run()
    .await
# }
```

## Graceful shutdown patterns

`Server::run()` installs SIGINT (Ctrl-C) and SIGTERM (Unix) handlers automatically.

For tests or orchestration scenarios where you need programmatic shutdown:

```rust,no_run
use altair_server::Server;
use tokio::sync::oneshot;
use std::time::Duration;

# async fn run() -> altair_server::Result<()> {
let server = Server::builder().build().await?;
let (tx, rx) = oneshot::channel::<()>();

tokio::spawn(async move {
    tokio::time::sleep(Duration::from_secs(10)).await;
    let _ = tx.send(());
});

server.run_with_shutdown(async move {
    let _ = rx.await;
}).await
# }
```

## Error reference

| Variant | When |
|---|---|
| `Error::Bind` | TCP listener couldn't bind (port in use, permission denied, ...) |
| `Error::Io` | I/O error during the serve loop (rare, from tokio/hyper internals) |
| `Error::Configuration` | Builder rejected a configuration value (e.g. invalid bind address) |

## License

[MIT](../../LICENSE)
