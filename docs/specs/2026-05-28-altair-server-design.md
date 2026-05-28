# altair-server — Design

**Date:** 2026-05-28
**Status:** Draft — awaiting review before implementation planning
**Author:** Jasoet
**Spec type:** Brainstorming output → input to writing-plans

---

## 1. Overview

`altair-server` is a thin convenience layer over [`axum`](https://crates.io/crates/axum) and [`tower-http`](https://crates.io/crates/tower-http) that handles the boilerplate every HTTP service needs: bind a port, install a sensible middleware stack (tracing, request-id propagation, request timeout), wire a `/health` endpoint, and shut down gracefully on SIGINT/SIGTERM.

**One-line product goal:** "Stop copying the same axum + tower-http boilerplate into every service."

The crate exposes a `ServerBuilder` whose route-registration methods delegate directly to axum's `Router`, so consumers get axum's full extractor/handler/state surface unchanged. Built-in middleware (tracing, request-id, timeout) is on by default and configurable; CORS and compression are opt-in. The underlying `axum`, `tower`, and `tower-http` crates are re-exported at the crate root so consumers don't need to add them as separate dependencies.

## 2. Decisions Locked

| Decision | Choice |
|---|---|
| Scope | HTTP/1.1+HTTP/2 server only. No TLS, no HTTP/3, no WebSocket helpers in v0.1. |
| Implementation strategy | Wrap `axum::Router` + selected `tower-http` middleware |
| Crate name | `altair-server` (verified available on crates.io 2026-05-28) |
| API style | Typed builder (`Server::builder()...build()`) that owns the lifecycle (`run()` / `run_with_shutdown()`) |
| Default middleware (always on, configurable) | `TraceLayer` (OTel-aware) + request-id (SetRequestIdLayer + PropagateRequestIdLayer) + `TimeoutLayer` (default 30s) |
| Opt-in middleware | CORS (`enable_cors()` / `enable_cors_with(layer)`), compression (`enable_compression()`), arbitrary tower Layer (`with_middleware(layer)`) |
| Health endpoint | `GET /health → 200 OK` by default; customizable path + response; `disable_health()` to skip |
| Graceful shutdown | `run()` installs `SIGINT + SIGTERM` (Unix) / `SIGINT` (Windows) handlers; `run_with_shutdown(future)` for custom drivers |
| Re-exports | `pub use ::axum;`, `pub use ::tower;`, `pub use ::tower_http;` |
| Error type | `thiserror` enum: `Bind { addr, source }`, `Io`, `Configuration(String)` |
| Async runtime | tokio (inherited from axum) |
| Dependencies | `axum`, `tower`, `tower-http`, `tokio`, `tracing`, `uuid` (for request IDs), `thiserror` |
| Edition / MSRV | Inherit from workspace (Edition 2024, Rust 1.95) |

## 3. Architecture

### 3.1 File layout

```
crates/altair-server/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs        # crate root: lints, mod declarations, re-exports, prelude
│   ├── error.rs      # Error enum + Result alias (thiserror)
│   ├── builder.rs    # ServerBuilder typed builder
│   ├── server.rs     # Server struct + run() / run_with_shutdown()
│   ├── middleware.rs # default-stack assembly helpers (trace + request-id + timeout)
│   ├── health.rs     # built-in /health endpoint with customizable response
│   ├── shutdown.rs   # shutdown_signal() future (Unix + Windows)
│   └── prelude.rs    # one-import bundle
├── tests/
│   └── integration.rs    # bind ephemeral port + hit it with reqwest
└── examples/
    ├── basic.rs
    ├── with_routes.rs
    ├── custom_middleware.rs
    ├── graceful_shutdown.rs
    ├── with_state.rs
    └── custom_health.rs
```

### 3.2 Module responsibilities

- **`error.rs`** — sole owner of the `Error` enum and `Result<T>` alias.
- **`builder.rs`** — `ServerBuilder` with all configuration knobs. Holds an `axum::Router` internally; route methods delegate. `build()` applies default + opt-in middleware in the correct order and constructs a `Server`.
- **`server.rs`** — `Server` holds a configured router and bind config. `run()` binds, installs default signal handlers, and serves until shutdown. `run_with_shutdown(future)` accepts a custom shutdown driver.
- **`middleware.rs`** — `pub(crate)` helpers to assemble the default middleware stack. Each layer can be skipped via builder flags.
- **`health.rs`** — `pub(crate) fn install_health(router, path, responder, enabled) -> Router`. Registers the health route before user routes (so collisions favour health).
- **`shutdown.rs`** — `pub fn shutdown_signal() -> impl Future<Output = ()>`. Listens for SIGINT (always) and SIGTERM (Unix only). Completes when either fires.

### 3.3 Public API

```rust
// crate root re-exports
pub use builder::ServerBuilder;
pub use error::{Error, Result};
pub use server::Server;
pub use shutdown::shutdown_signal;

// Re-exports for one-dep ergonomics
pub use ::axum;
pub use ::tower;
pub use ::tower_http;

pub mod prelude;
```

### 3.4 `ServerBuilder` surface

```rust
impl ServerBuilder {
    pub fn new() -> Self;

    // Binding
    pub fn bind_addr(self, addr: impl Into<String>) -> Self;       // default "0.0.0.0:8080"
    pub fn bind_socket(self, addr: SocketAddr) -> Self;

    // Routes — delegate to axum::Router
    pub fn route<H, T>(self, path: &str, handler: H) -> Self
    where H: axum::handler::Handler<T, ()>, T: 'static;
    pub fn merge(self, other: axum::Router) -> Self;
    pub fn nest(self, prefix: &str, router: axum::Router) -> Self;

    // State (axum-style shared app state)
    // For v0.1, ServerBuilder is generic over the state type; see §4.1
    pub fn with_state<S>(self, state: S) -> ServerBuilder<S>
    where S: Clone + Send + Sync + 'static;

    // Default-middleware tuning
    pub fn request_timeout(self, d: Duration) -> Self;             // default 30s
    pub fn disable_tracing(self) -> Self;
    pub fn disable_request_id(self) -> Self;

    // Opt-in middleware
    pub fn enable_cors(self) -> Self;                              // CorsLayer::permissive()
    pub fn enable_cors_with(self, layer: tower_http::cors::CorsLayer) -> Self;
    pub fn enable_compression(self) -> Self;                       // gzip + br + zstd

    // Arbitrary user middleware
    pub fn with_middleware<L>(self, layer: L) -> Self
    where L: tower::Layer<...> + Clone + Send + Sync + 'static;

    // Health
    pub fn health_path(self, path: &str) -> Self;                  // default "/health"
    pub fn health_response<F>(self, responder: F) -> Self
    where F: Fn() -> axum::response::Response + Send + Sync + 'static;
    pub fn disable_health(self) -> Self;

    pub fn build(self) -> Result<Server>;
}
```

### 3.5 `Server` surface

```rust
impl Server {
    /// Start building.
    pub fn builder() -> ServerBuilder;

    /// Bind the listener and serve until SIGINT/SIGTERM.
    pub async fn run(self) -> Result<()>;

    /// Bind and serve until the given future resolves.
    pub async fn run_with_shutdown<F>(self, shutdown: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static;

    /// Actual local socket address after binding. Useful when `bind_addr`
    /// is `0.0.0.0:0` and you need the OS-assigned port (e.g. in tests).
    pub fn local_addr(&self) -> Result<std::net::SocketAddr>;
}
```

### 3.6 Error model

```rust
#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to bind {addr}: {source}")]
    Bind {
        addr: String,
        #[source]
        source: std::io::Error,
    },

    #[error("server I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("server configuration: {0}")]
    Configuration(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

**Non-variants:**
- No `Handler` / `RouteFailed` — handler failures are user-domain (HTTP responses), not server-infrastructure.
- No `Shutdown` — graceful shutdown completing is `Ok(())`, not an error.
- No `Timeout` — request-timeout responses are surfaced to the *client* as HTTP 408 by `tower_http::timeout`, not back to `run()`.

## 4. Behaviour Details

### 4.1 State plumbing

The builder is generic over an optional axum state type. The default `ServerBuilder` (no state) operates on `axum::Router<()>`. `.with_state(s)` transitions to `ServerBuilder<S>` operating on `axum::Router<S>`. Once state is set, all subsequent `.route()`/`.merge()`/`.nest()` calls expect handlers compatible with that state.

Internally:

```rust
pub struct ServerBuilder<S = ()> {
    router: axum::Router<S>,
    state: Option<S>,
    // ... knobs
}
```

`build()` calls `router.with_state(state)` if state is set, producing `axum::Router<()>` ready to serve.

### 4.2 Middleware ordering

Outer-most first (requests pass through top-to-bottom; responses bottom-to-top):

1. `TraceLayer::new_for_http()` — span per request, OTel-aware
2. `SetRequestIdLayer::x_request_id(MakeRequestUuid)` — assigns UUID if missing
3. `PropagateRequestIdLayer::x_request_id()` — echoes ID in response headers
4. `CorsLayer` (if enabled)
5. `CompressionLayer` (if enabled)
6. `TimeoutLayer::new(request_timeout)` — innermost so user middleware sees the deadline
7. User-added layers via `with_middleware` (innermost-of-user-stack-first)
8. User-defined routes

Default order matches what most services expect: tracing wraps everything; request-id is observable in trace context; timeout is closest to handler so it gates execution.

### 4.3 Request-id behaviour

- Incoming `x-request-id` header: respected. Echoed in response.
- Missing header: a new UUID v4 is generated and used.
- The request-id is added as a `tracing` span field via the standard `tower_http::trace` integration.
- `disable_request_id()` skips both the SetRequestIdLayer and PropagateRequestIdLayer entirely.

### 4.4 Health endpoint registration

Registered *before* user routes are added to the internal router. Path defaults to `/health`. If a user route also claims `/health`, axum will use the *first* matching route, so the built-in health endpoint wins.

Customisation:

```rust
Server::builder()
    .health_path("/healthz")
    .health_response(|| {
        axum::Json(serde_json::json!({"status": "ok"})).into_response()
    })
    .route(...)
```

`disable_health()` skips registration entirely.

### 4.5 Graceful shutdown

`run()` is equivalent to `run_with_shutdown(shutdown_signal())`.

`shutdown_signal()`:
- Always listens for `tokio::signal::ctrl_c()` (Ctrl-C / SIGINT).
- On Unix, also listens for `SIGTERM` via `tokio::signal::unix::signal(SignalKind::terminate())`.
- Resolves as soon as either signal fires; doesn't drain.

After shutdown begins, axum's `with_graceful_shutdown` stops accepting new connections and waits up to the request-timeout for in-flight requests to complete. Past that, connections are abandoned and `run()` returns `Ok(())`.

### 4.6 Local address

If `bind_addr("0.0.0.0:0")` was used (let OS pick a port), `Server::local_addr()` returns the bound address after `build()` succeeds. Useful for integration tests that need to know the port.

Implementation: the `Server` binds the listener at `build()` time (not at `run()` time) so `local_addr()` is always callable after construction.

## 5. Testing Strategy

| Layer | Where | Run by |
|---|---|---|
| Unit (inline `#[cfg(test)]`) | each `src/*.rs` | `cargo test --lib` |
| Integration (real HTTP) | `tests/integration.rs` using `reqwest` against an ephemeral-port server | `cargo test --tests` |
| Doc tests | `///` examples in source (mostly `no_run`) | bundled with `cargo test` |
| Example-as-test | `examples/*.rs` compile-only | `cargo build --examples` |

**Specific test obligations:**

| File | Tests |
|---|---|
| `error.rs` | Display rendering for each variant |
| `builder.rs` | Defaults; route registration; bad bind address rejection; disable flags take effect; opt-in flags apply |
| `health.rs` | Default `GET /health → 200`; custom path; custom responder; `disable_health` removes the route |
| `shutdown.rs` | `shutdown_signal()` completes on a simulated signal (test via channel-based shim) |
| `tests/integration.rs` | bind ephemeral port; hit it with reqwest; verify status, request-id echo, timeout returns 408, CORS preflight works |

**Coverage target:** ≥85%.

**Dev-deps:**
- `reqwest` with default features (for integration tests)
- `tokio` with `macros` + `rt-multi-thread`
- `pretty_assertions`, `anyhow`, `serde`, `serde_json`

## 6. Cross-Crate Integration

- **`altair-otel`**: `TraceLayer` emits `tracing` spans using OpenTelemetry semantic conventions for HTTP. If `altair-otel::Config::init()` ran earlier, those spans flow to OTLP automatically.
- **`altair-config`**: `ServerBuilder` knobs accept primitive types (`SocketAddr`, `Duration`, etc.) — straightforward to populate from a `Deserialize`-derived config struct.
- **`altair-rest`**: server and client coexist without conflict. Different middleware ecosystems (server-side `tower-http`; client-side `reqwest-middleware`).
- **Other altair-rs crates**: no direct integration; orthogonal concerns.

## 7. Out of Scope (v0.1.0)

- **TLS / HTTPS** — deferred to a future opt-in feature flag with `axum-server` or `rustls`. Users wanting TLS in v0.1 wrap a `Server` in their own listener.
- **HTTP/3 / QUIC** — out of scope.
- **WebSockets, SSE** — axum has direct support; no helpers in v0.1.
- **Authentication / JWT / session middleware** — users add their own via `with_middleware`.
- **`/metrics` endpoint** — separate concern; would need an OTel-Prometheus bridge.
- **Multiple bind addresses / dual-stack** — single listener for v0.1.
- **Per-route timeout / rate-limit** — use `tower-http` directly via `with_middleware` or per-route `axum::Router::layer`.
- **Static file serving** — use `tower_http::services::ServeDir` directly.
- **OpenAPI / schema generation** — out of scope; use `utoipa` or `aide` alongside.

## 8. Risks & Open Questions

| Item | Risk | Mitigation |
|---|---|---|
| `axum` v0.8 → v0.9 trait changes | Medium | Pin in workspace.dependencies; absorb upgrades as our own minor bumps |
| `tower-http` major version bumps | Medium | Same |
| Re-exporting `axum` + `tower` + `tower-http` means their breaking changes become ours | Documented trade-off | Pin via workspace.deps; document |
| Type parameter for state makes the builder API noisier | Low | Default `S = ()` keeps the no-state case simple |
| `SIGTERM` handling differs on Windows | Low | Document; SIGINT only on Windows |
| `with_middleware` typing — `tower::Layer` generic constraints can be painful | Medium | Provide concrete `enable_cors`/`enable_compression`/etc. for common cases; document the generic escape hatch |

## 9. Versioning

- Inherits `version.workspace = true` — first publish at the current workspace shared version.
- Re-exports of `axum`, `tower`, `tower-http` are part of the public API; their upgrades become our own minor bumps.

## 10. Next Steps

1. **User reviews this spec** (current step)
2. On approval: `writing-plans` skill produces an implementation plan
3. Implementation plan drives: crate scaffolding → per-module TDD → testing → CI → publish at workspace version
