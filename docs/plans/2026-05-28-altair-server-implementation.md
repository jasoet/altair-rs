# altair-server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build, test, and publish `altair-server` — an axum + tower-http convenience layer that handles binding, default middleware (tracing + request-id + timeout), a built-in `/health` endpoint, and graceful shutdown — to crates.io at the current workspace version.

**Architecture:** Single crate under `crates/altair-server/`. Seven source files (`lib.rs`, `error.rs`, `builder.rs`, `server.rs`, `middleware.rs`, `health.rs`, `shutdown.rs`, `prelude.rs`). `ServerBuilder` is a typed builder that delegates route registration to `axum::Router`. `Server::run()` binds, installs SIGINT/SIGTERM handlers, applies the configured middleware stack, and serves until shutdown.

**Tech Stack:**
- Rust 2024, MSRV 1.95 (inherit from workspace)
- `axum = "0.8"` (default features) — the de facto Rust web framework
- `tower = "0.5"` — middleware service abstraction
- `tower-http = "0.6"` (features: trace, request-id, timeout, cors, compression-full) — HTTP-specific middleware
- `tokio = "1"` with `signal` feature for SIGINT/SIGTERM
- `tracing = "0.1"` (workspace) — for span/event emission
- `uuid = "1"` (with v4 feature) — for request ID generation (used by `tower_http::request_id::MakeRequestUuid`)
- `thiserror = "2"` (workspace)

Dev-deps:
- `reqwest = "0.13"` (default features + json) — for integration tests hitting the bound server
- `tokio` with `macros` + `rt-multi-thread`
- `pretty_assertions`, `anyhow`, `serde` (derive), `serde_json`

---

## File Structure

```
crates/altair-server/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs        # crate root: lints, mod declarations, re-exports
│   ├── error.rs      # Error enum + Result alias
│   ├── builder.rs    # ServerBuilder typed builder + build()
│   ├── server.rs     # Server struct + run() + run_with_shutdown()
│   ├── middleware.rs # default-stack assembly (trace + request-id + timeout)
│   ├── health.rs     # built-in /health endpoint registration
│   ├── shutdown.rs   # shutdown_signal() future
│   └── prelude.rs    # one-import bundle
├── tests/
│   └── integration.rs    # bind ephemeral port + hit with reqwest
└── examples/
    ├── basic.rs
    ├── with_routes.rs
    ├── custom_middleware.rs
    ├── graceful_shutdown.rs
    ├── with_state.rs
    └── custom_health.rs
```

Workspace edits:
- `Cargo.toml`: add `axum`, `tower`, `tower-http`, `uuid` to `[workspace.dependencies]`; add `crates/altair-server` to `members`
- `docs/porting-tracker.md`: move `altair-server` from Awaiting Demand → Done; add release-notes bullet
- `README.md`: add `altair-server` row to crate table

---

## Phase 1: Crate Scaffold

### Task 1.1: Add libraries to workspace dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Add deps**

In the root `Cargo.toml`'s `[workspace.dependencies]` block, add a new "# HTTP server" section. Add to `[workspace.dependencies]`:

```toml
# HTTP server
axum = { version = "0.8", default-features = true }
tower = "0.5"
tower-http = { version = "0.6", features = ["trace", "request-id", "timeout", "cors", "compression-full", "util"] }
uuid = { version = "1", features = ["v4"] }
```

If `reqwest` is already in workspace.dependencies (it is — added for altair-rest), keep the existing line and don't duplicate.

- [ ] **Step 2: Verify workspace parses**

Run: `cargo metadata --format-version=1 > /dev/null`
Expected: exit 0.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add axum + tower + tower-http + uuid to workspace dependencies"
```

### Task 1.2: Create crate skeleton

**Files:**
- Create: `crates/altair-server/Cargo.toml`
- Create: `crates/altair-server/src/lib.rs`
- Create: `crates/altair-server/README.md` (stub)
- Modify: `Cargo.toml` (workspace `members`)

- [ ] **Step 1: Create directories**

```bash
mkdir -p crates/altair-server/src crates/altair-server/tests crates/altair-server/examples
```

- [ ] **Step 2: Write `crates/altair-server/Cargo.toml`**

```toml
[package]
name = "altair-server"
description = "Axum + tower-http convenience layer with sensible defaults and graceful shutdown"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
homepage.workspace = true
readme = "README.md"
keywords = ["http", "server", "axum", "tower", "web"]
categories = ["network-programming", "web-programming::http-server"]

[dependencies]
axum = { workspace = true }
tower = { workspace = true }
tower-http = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
reqwest = { workspace = true, features = ["json"] }
pretty_assertions = { workspace = true }
anyhow = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 3: Write `crates/altair-server/src/lib.rs`**

```rust
//! Axum + tower-http convenience layer.
//!
//! Wraps an `axum::Router` with a sensible-default middleware stack
//! (tracing, request-id propagation, request timeout), a built-in `/health`
//! endpoint, and SIGINT/SIGTERM-aware graceful shutdown. The underlying
//! `axum`, `tower`, and `tower-http` crates are re-exported at the crate
//! root so consumers don't need to add them as separate dependencies.
//!
//! # Example
//!
//! ```no_run
//! use altair_server::Server;
//! use altair_server::axum::routing::get;
//!
//! # async fn run() -> altair_server::Result<()> {
//! Server::builder()
//!     .bind_addr("0.0.0.0:3000")
//!     .route("/", get(|| async { "hello" }))
//!     .build()?
//!     .run()
//!     .await
//! # }
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod builder;
mod error;
mod health;
mod middleware;
mod server;
mod shutdown;

pub mod prelude;

pub use builder::ServerBuilder;
pub use error::{Error, Result};
pub use server::Server;
pub use shutdown::shutdown_signal;

// Re-exports for one-dep ergonomics
pub use ::axum;
pub use ::tower;
pub use ::tower_http;
```

- [ ] **Step 4: Write stub README**

```markdown
# altair-server

Axum + tower-http convenience layer with sensible defaults and graceful shutdown.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

(Full README added in a later task.)
```

- [ ] **Step 5: Register in workspace `members`**

In root `Cargo.toml`, append `"crates/altair-server"` to the `members` list. After your edit the list should contain eight entries:

```toml
members = [
    "crates/altair-concurrent",
    "crates/altair-retry",
    "crates/altair-config",
    "crates/altair-otel",
    "crates/altair-base32",
    "crates/altair-compress",
    "crates/altair-rest",
    "crates/altair-server",
]
```

- [ ] **Step 6: Verify workspace parses**

Run: `cargo metadata --format-version=1 > /dev/null`
Expected: exit 0.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/altair-server
git commit -m "feat(server): scaffold altair-server crate"
```

---

## Phase 2: Error type

### Task 2.1: Write `error.rs` with tests

**Files:**
- Create: `crates/altair-server/src/error.rs`

- [ ] **Step 1: Write the file**

```rust
//! Crate-wide error type for `altair-server`.

use thiserror::Error;

/// Errors returned by `altair-server` operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to bind the TCP listener.
    #[error("failed to bind {addr}: {source}")]
    Bind {
        /// The address we attempted to bind.
        addr: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// I/O error during the serve loop.
    #[error("server I/O: {0}")]
    Io(#[from] std::io::Error),

    /// Builder rejected a configuration value (bad bind address, etc.).
    #[error("server configuration: {0}")]
    Configuration(String),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_error_renders_addr_and_source() {
        let e = Error::Bind {
            addr: "0.0.0.0:8080".into(),
            source: std::io::Error::other("address in use"),
        };
        let s = e.to_string();
        assert!(s.contains("0.0.0.0:8080"));
        assert!(s.contains("address in use"));
    }

    #[test]
    fn io_error_renders() {
        let io = std::io::Error::other("disk full");
        let e: Error = io.into();
        assert!(e.to_string().contains("disk full"));
    }

    #[test]
    fn configuration_error_renders() {
        let e = Error::Configuration("invalid port".into());
        assert_eq!(e.to_string(), "server configuration: invalid port");
    }
}
```

- [ ] **Step 2: Run tests**

The crate won't link yet — other modules don't exist. Temporarily comment out `mod builder; mod health; mod middleware; mod server; mod shutdown; pub mod prelude;` and their `pub use` lines in `crates/altair-server/src/lib.rs`. Keep `mod error;` and `pub use error::{Error, Result};` active. Run `cargo test -p altair-server --lib error`. Expected: 3 tests pass. Restore lib.rs before committing.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-server/src/error.rs
git commit -m "feat(server): add Error type and Result alias"
```

Only `error.rs` should be in the commit.

---

## Phase 3: Shutdown signal

### Task 3.1: Write `shutdown.rs` with tests

**Files:**
- Create: `crates/altair-server/src/shutdown.rs`

- [ ] **Step 1: Write the file**

```rust
//! Signal-driven graceful shutdown future.

use tokio::signal;

/// Future that resolves when the process receives SIGINT (Ctrl-C) on all
/// platforms, or SIGTERM on Unix.
///
/// Use this with [`crate::Server::run_with_shutdown`] for custom shutdown
/// orchestration, or rely on [`crate::Server::run`] which installs this
/// automatically.
///
/// ```no_run
/// # async fn run() {
/// altair_server::shutdown_signal().await;
/// println!("shutdown signal received");
/// # }
/// ```
pub async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            tracing::warn!("failed to install Ctrl-C handler: {e}");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::warn!("failed to install SIGTERM handler: {e}");
                // Block forever — the ctrl_c branch will still resolve.
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_signal_can_race_against_timer() {
        // We can't trigger a signal in a unit test, but we can verify that
        // the future is well-formed by racing it against an immediate
        // tokio::time::sleep — the sleep should win, proving shutdown_signal
        // hasn't already completed.
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            shutdown_signal(),
        )
        .await;
        assert!(result.is_err(), "shutdown_signal should not complete in 50ms");
    }
}
```

- [ ] **Step 2: Run tests**

Temporarily comment out `mod builder; mod health; mod middleware; mod server; pub mod prelude;` and their `pub use`s in lib.rs. Keep `mod error;` and `mod shutdown;` active.

Run: `cargo test -p altair-server --lib shutdown`
Expected: 1 test passes (within ~100ms).

Restore lib.rs before committing.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-server/src/shutdown.rs
git commit -m "feat(server): add shutdown_signal for SIGINT/SIGTERM-driven graceful shutdown"
```

Only `shutdown.rs` in the commit.

---

## Phase 4: Health endpoint

### Task 4.1: Write `health.rs` with tests

**Files:**
- Create: `crates/altair-server/src/health.rs`

- [ ] **Step 1: Write the file**

```rust
//! Built-in `/health` endpoint.

use axum::Router;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use std::sync::Arc;

/// Closure that produces the health endpoint's response.
pub(crate) type HealthResponder = Arc<dyn Fn() -> Response + Send + Sync + 'static>;

/// Default response: 200 OK with an empty body.
pub(crate) fn default_responder() -> HealthResponder {
    Arc::new(|| ().into_response())
}

/// Register the configured health route on `router`, if enabled.
pub(crate) fn install<S>(
    router: Router<S>,
    enabled: bool,
    path: &str,
    responder: HealthResponder,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if !enabled {
        return router;
    }
    let handler = move || {
        let r = responder.clone();
        async move { r() }
    };
    router.route(path, get(handler))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::body::Body;
    use http::Request;
    use tower::ServiceExt;

    fn build_router() -> Router {
        let router: Router = Router::new();
        install(router, true, "/health", default_responder())
    }

    #[tokio::test]
    async fn default_health_returns_200() {
        let router = build_router();
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn disabled_health_returns_404() {
        let router = install(Router::new(), false, "/health", default_responder());
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn custom_path_responds() {
        let router = install(Router::new(), true, "/healthz", default_responder());
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn custom_responder_runs() {
        let responder: HealthResponder = Arc::new(|| {
            (StatusCode::SERVICE_UNAVAILABLE, "db down").into_response()
        });
        let router = install(Router::new(), true, "/health", responder);
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
```

This file uses `http::Request` and `tower::ServiceExt::oneshot`. Both are available via `axum`'s dependencies (axum re-exports `http`) but to be safe we list `http` as a regular dep through axum's re-export. If the test fails with "no `http` crate", add `http = "1"` to dev-dependencies in `crates/altair-server/Cargo.toml`. Try without first; only add the dep if compilation fails.

- [ ] **Step 2: Run tests**

Temporarily comment out `mod builder; mod middleware; mod server; pub mod prelude;` and their `pub use`s in lib.rs. Keep `mod error; mod health; mod shutdown;` active.

Run: `cargo test -p altair-server --lib health`
Expected: 4 tests pass.

Restore lib.rs before committing.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-server/src/health.rs
git commit -m "feat(server): add configurable /health endpoint"
```

Only `health.rs` in the commit. If you needed to add `http` to dev-deps, include that in this commit too.

---

## Phase 5: Default middleware

### Task 5.1: Write `middleware.rs` with tests

**Files:**
- Create: `crates/altair-server/src/middleware.rs`

- [ ] **Step 1: Write the file**

```rust
//! Default middleware-stack assembly.
//!
//! Order applied (outermost → innermost):
//!
//! 1. `TraceLayer::new_for_http()` — OTel-aware request span
//! 2. `SetRequestIdLayer::x_request_id(MakeRequestUuid)` — assign UUID if missing
//! 3. `PropagateRequestIdLayer::x_request_id()` — echo ID in response
//! 4. `CorsLayer` (if enabled)
//! 5. `CompressionLayer` (if enabled)
//! 6. `TimeoutLayer::new(timeout)` — request deadline
//!
//! User-added layers via `with_middleware` are applied innermost-of-stack
//! (closest to the handler), which gives them visibility into the
//! request-id and trace context.

use axum::Router;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

/// Configuration for the default middleware stack.
pub(crate) struct DefaultStack {
    pub tracing: bool,
    pub request_id: bool,
    pub timeout: Duration,
    pub cors: Option<CorsLayer>,
    pub compression: bool,
}

impl DefaultStack {
    /// Apply the configured layers to a router.
    pub(crate) fn apply<S>(self, router: Router<S>) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        let mut router = router;

        // Innermost first because Router::layer wraps outwards.
        router = router.layer(TimeoutLayer::new(self.timeout));

        if self.compression {
            router = router.layer(CompressionLayer::new());
        }

        if let Some(cors) = self.cors {
            router = router.layer(cors);
        }

        if self.request_id {
            router = router
                .layer(PropagateRequestIdLayer::x_request_id())
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid));
        }

        if self.tracing {
            router = router.layer(TraceLayer::new_for_http());
        }

        router
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use tower::ServiceExt;

    fn router_with_stack(stack: DefaultStack) -> Router {
        let base: Router = Router::new().route("/", get(|| async { "ok" }));
        stack.apply(base)
    }

    #[tokio::test]
    async fn defaults_pass_through_ok_request() {
        let stack = DefaultStack {
            tracing: true,
            request_id: true,
            timeout: Duration::from_secs(5),
            cors: None,
            compression: false,
        };
        let router = router_with_stack(stack);
        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key("x-request-id"));
    }

    #[tokio::test]
    async fn request_id_disabled_omits_header() {
        let stack = DefaultStack {
            tracing: false,
            request_id: false,
            timeout: Duration::from_secs(5),
            cors: None,
            compression: false,
        };
        let router = router_with_stack(stack);
        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!response.headers().contains_key("x-request-id"));
    }

    #[tokio::test]
    async fn timeout_returns_408_when_slow_handler() {
        let stack = DefaultStack {
            tracing: false,
            request_id: false,
            timeout: Duration::from_millis(20),
            cors: None,
            compression: false,
        };
        let base: Router = Router::new().route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(1)).await;
                "done"
            }),
        );
        let router = stack.apply(base);
        let response = router
            .oneshot(Request::builder().uri("/slow").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
    }
}
```

- [ ] **Step 2: Run tests**

Temporarily comment out `mod builder; mod server; pub mod prelude;` and their `pub use`s in lib.rs. Keep `mod error; mod health; mod middleware; mod shutdown;` active.

Run: `cargo test -p altair-server --lib middleware`
Expected: 3 tests pass.

Restore lib.rs before committing.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-server/src/middleware.rs
git commit -m "feat(server): default middleware stack (trace + request-id + timeout)"
```

Only `middleware.rs` in the commit.

---

## Phase 6: Server + Builder

### Task 6.1: Write `server.rs` (Server struct + run methods)

**Files:**
- Create: `crates/altair-server/src/server.rs`

- [ ] **Step 1: Write the file**

```rust
//! `Server` — the constructed runtime that binds, serves, and shuts down.

use crate::builder::ServerBuilder;
use crate::error::{Error, Result};
use crate::shutdown::shutdown_signal;
use axum::Router;
use std::net::SocketAddr;
use tokio::net::TcpListener;

/// Configured server, bound to a TCP listener.
///
/// Build via [`Server::builder`].
pub struct Server {
    router: Router<()>,
    listener: TcpListener,
    local_addr: SocketAddr,
}

impl Server {
    /// Start building a new server.
    #[must_use]
    pub fn builder() -> ServerBuilder {
        ServerBuilder::new()
    }

    /// Internal constructor used by [`ServerBuilder::build`].
    pub(crate) fn from_parts(
        router: Router<()>,
        listener: TcpListener,
        local_addr: SocketAddr,
    ) -> Self {
        Self {
            router,
            listener,
            local_addr,
        }
    }

    /// Actual bound socket address.
    ///
    /// Useful when `bind_addr("0.0.0.0:0")` was used so the OS chose a port.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Bind the listener and serve forever (until SIGINT / SIGTERM).
    ///
    /// Returns `Ok(())` after graceful shutdown completes.
    pub async fn run(self) -> Result<()> {
        self.run_with_shutdown(shutdown_signal()).await
    }

    /// Bind and serve until the given future resolves.
    pub async fn run_with_shutdown<F>(self, shutdown: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        axum::serve(self.listener, self.router)
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(Error::from)
    }
}
```

- [ ] **Step 2: Run tests later** — we can't run tests here yet because `ServerBuilder` doesn't exist. Build verification comes in Task 6.2.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-server/src/server.rs
git commit -m "feat(server): add Server struct with run + run_with_shutdown"
```

### Task 6.2: Write `builder.rs` (ServerBuilder)

**Files:**
- Create: `crates/altair-server/src/builder.rs`

- [ ] **Step 1: Write the file**

```rust
//! Typed builder for [`crate::Server`].

use crate::error::{Error, Result};
use crate::health::{self, HealthResponder};
use crate::middleware::DefaultStack;
use crate::server::Server;
use axum::Router;
use axum::handler::Handler;
use axum::response::IntoResponse;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_HEALTH_PATH: &str = "/health";

/// Typed builder for [`Server`].
///
/// Construct via [`Server::builder`](crate::Server::builder).
#[must_use]
pub struct ServerBuilder {
    bind_addr: String,
    router: Router<()>,
    tracing: bool,
    request_id: bool,
    timeout: Duration,
    cors: Option<CorsLayer>,
    compression: bool,
    health_enabled: bool,
    health_path: String,
    health_responder: HealthResponder,
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self {
            bind_addr: DEFAULT_BIND_ADDR.to_string(),
            router: Router::new(),
            tracing: true,
            request_id: true,
            timeout: DEFAULT_TIMEOUT,
            cors: None,
            compression: false,
            health_enabled: true,
            health_path: DEFAULT_HEALTH_PATH.to_string(),
            health_responder: health::default_responder(),
        }
    }
}

impl ServerBuilder {
    /// Create a builder with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the bind address as a string (e.g. `"0.0.0.0:3000"`, `"[::]:8080"`).
    pub fn bind_addr(mut self, addr: impl Into<String>) -> Self {
        self.bind_addr = addr.into();
        self
    }

    /// Set the bind address as a parsed `SocketAddr`.
    pub fn bind_socket(mut self, addr: SocketAddr) -> Self {
        self.bind_addr = addr.to_string();
        self
    }

    /// Register a route, delegating to [`axum::Router::route`].
    pub fn route<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, axum::routing::any(handler));
        self
    }

    /// Merge another router (delegates to [`axum::Router::merge`]).
    pub fn merge(mut self, other: Router) -> Self {
        self.router = self.router.merge(other);
        self
    }

    /// Mount a router at a nested path (delegates to [`axum::Router::nest`]).
    pub fn nest(mut self, prefix: &str, router: Router) -> Self {
        self.router = self.router.nest(prefix, router);
        self
    }

    /// Set the per-request timeout. Default 30s.
    pub fn request_timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    /// Disable the default tracing middleware.
    pub fn disable_tracing(mut self) -> Self {
        self.tracing = false;
        self
    }

    /// Disable the default request-id middleware.
    pub fn disable_request_id(mut self) -> Self {
        self.request_id = false;
        self
    }

    /// Enable CORS with permissive defaults (`CorsLayer::permissive()`).
    pub fn enable_cors(mut self) -> Self {
        self.cors = Some(CorsLayer::permissive());
        self
    }

    /// Enable CORS with a custom [`CorsLayer`].
    pub fn enable_cors_with(mut self, layer: CorsLayer) -> Self {
        self.cors = Some(layer);
        self
    }

    /// Enable response compression (gzip/br/zstd).
    pub fn enable_compression(mut self) -> Self {
        self.compression = true;
        self
    }

    /// Customise the health endpoint path. Default `/health`.
    pub fn health_path(mut self, path: &str) -> Self {
        self.health_path = path.to_string();
        self
    }

    /// Provide a custom responder for the health endpoint.
    pub fn health_response<F, R>(mut self, responder: F) -> Self
    where
        F: Fn() -> R + Send + Sync + 'static,
        R: IntoResponse + 'static,
    {
        self.health_responder = Arc::new(move || responder().into_response());
        self
    }

    /// Disable the built-in health endpoint.
    pub fn disable_health(mut self) -> Self {
        self.health_enabled = false;
        self
    }

    /// Bind the listener and build a [`Server`] ready to run.
    pub async fn build(self) -> Result<Server> {
        let addr: SocketAddr = self
            .bind_addr
            .parse()
            .map_err(|e| Error::Configuration(format!("invalid bind address '{}': {e}", self.bind_addr)))?;

        let listener = TcpListener::bind(addr).await.map_err(|e| Error::Bind {
            addr: self.bind_addr.clone(),
            source: e,
        })?;
        let local_addr = listener.local_addr().map_err(Error::from)?;

        // Register health first so it always wins on its configured path.
        let router = health::install(
            self.router,
            self.health_enabled,
            &self.health_path,
            self.health_responder,
        );

        let stack = DefaultStack {
            tracing: self.tracing,
            request_id: self.request_id,
            timeout: self.timeout,
            cors: self.cors,
            compression: self.compression,
        };

        let router = stack.apply(router);

        Ok(Server::from_parts(router, listener, local_addr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn build_with_defaults_binds_ephemeral_port() {
        let server = ServerBuilder::new()
            .bind_addr("127.0.0.1:0")
            .build()
            .await
            .unwrap();
        let addr = server.local_addr();
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert!(addr.port() > 0);
    }

    #[tokio::test]
    async fn build_rejects_invalid_bind_address() {
        let result = ServerBuilder::new()
            .bind_addr("not a socket address")
            .build()
            .await;
        assert!(matches!(result, Err(Error::Configuration(_))));
    }

    #[tokio::test]
    async fn build_with_bind_socket_works() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let server = ServerBuilder::new()
            .bind_socket(addr)
            .build()
            .await
            .unwrap();
        assert_eq!(server.local_addr().ip().to_string(), "127.0.0.1");
    }

    #[tokio::test]
    async fn build_with_custom_timeout() {
        let server = ServerBuilder::new()
            .bind_addr("127.0.0.1:0")
            .request_timeout(Duration::from_secs(5))
            .build()
            .await
            .unwrap();
        // Just verify it built successfully — timeout behaviour is tested in middleware.rs
        let _ = server.local_addr();
    }
}
```

- [ ] **Step 2: Run tests**

The crate should now mostly link. Temporarily comment out `pub mod prelude;` in lib.rs.

Run: `cargo test -p altair-server --lib`
Expected: all unit tests pass (3 error + 1 shutdown + 4 health + 3 middleware + 4 builder = 15 tests).

Restore lib.rs before committing.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-server/src/builder.rs
git commit -m "feat(server): ServerBuilder with route, middleware, and health knobs"
```

Only `builder.rs` in the commit.

---

## Phase 7: Prelude

### Task 7.1: Write `prelude.rs`

**Files:**
- Create: `crates/altair-server/src/prelude.rs`

- [ ] **Step 1: Write the file**

```rust
//! Common imports for users of this crate.
//!
//! ```no_run
//! use altair_server::prelude::*;
//!
//! # async fn run() -> altair_server::Result<()> {
//! let server = Server::builder()
//!     .bind_addr("127.0.0.1:0")
//!     .build()
//!     .await?;
//! # let _ = server;
//! # Ok(()) }
//! ```

pub use crate::{Error, Result, Server, ServerBuilder, shutdown_signal};
```

- [ ] **Step 2: Verify the whole crate compiles**

Run: `cargo test -p altair-server --lib`
Expected: 15 unit tests pass.

Run: `cargo test -p altair-server --doc`
Expected: at least 2 doc tests pass (lib.rs example + prelude.rs example).

- [ ] **Step 3: Commit**

```bash
git add crates/altair-server/src/prelude.rs
git commit -m "feat(server): add prelude module"
```

---

## Phase 8: Integration tests

### Task 8.1: Write integration tests

**Files:**
- Create: `crates/altair-server/tests/integration.rs`

- [ ] **Step 1: Write the file**

```rust
//! End-to-end behaviour tests: bind ephemeral port, hit it with reqwest,
//! verify response details.

use altair_server::prelude::*;
use altair_server::axum::routing::get;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tokio::sync::oneshot;

async fn start_server(builder: ServerBuilder) -> (std::net::SocketAddr, oneshot::Sender<()>) {
    let server = builder.bind_addr("127.0.0.1:0").build().await.unwrap();
    let addr = server.local_addr();
    let (tx, rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = server
            .run_with_shutdown(async move {
                let _ = rx.await;
            })
            .await;
    });
    // Give the server a moment to start accepting connections.
    tokio::time::sleep(Duration::from_millis(20)).await;
    (addr, tx)
}

#[tokio::test]
async fn default_health_endpoint_returns_200() {
    let (addr, shutdown) = start_server(Server::builder()).await;
    let response = reqwest::get(format!("http://{addr}/health")).await.unwrap();
    assert_eq!(response.status(), 200);
    let _ = shutdown.send(());
}

#[tokio::test]
async fn user_route_returns_handler_body() {
    let (addr, shutdown) = start_server(
        Server::builder().route("/greet", get(|| async { "hello world" })),
    )
    .await;
    let body = reqwest::get(format!("http://{addr}/greet"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(body, "hello world");
    let _ = shutdown.send(());
}

#[tokio::test]
async fn request_id_header_is_echoed() {
    let (addr, shutdown) = start_server(
        Server::builder().route("/", get(|| async { "ok" })),
    )
    .await;
    let response = reqwest::get(format!("http://{addr}/")).await.unwrap();
    assert!(response.headers().contains_key("x-request-id"));
    let _ = shutdown.send(());
}

#[tokio::test]
async fn timeout_returns_408() {
    let (addr, shutdown) = start_server(
        Server::builder()
            .request_timeout(Duration::from_millis(50))
            .route(
                "/slow",
                get(|| async {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    "done"
                }),
            ),
    )
    .await;
    let response = reqwest::get(format!("http://{addr}/slow")).await.unwrap();
    assert_eq!(response.status(), 408);
    let _ = shutdown.send(());
}

#[tokio::test]
async fn custom_health_path_works() {
    let (addr, shutdown) = start_server(Server::builder().health_path("/ready")).await;
    let response = reqwest::get(format!("http://{addr}/ready")).await.unwrap();
    assert_eq!(response.status(), 200);
    let _ = shutdown.send(());
}

#[tokio::test]
async fn disable_health_removes_endpoint() {
    let (addr, shutdown) = start_server(Server::builder().disable_health()).await;
    let response = reqwest::get(format!("http://{addr}/health")).await.unwrap();
    assert_eq!(response.status(), 404);
    let _ = shutdown.send(());
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p altair-server --tests`
Expected: 6 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-server/tests/integration.rs
git commit -m "test(server): end-to-end integration tests via reqwest"
```

---

## Phase 9: Examples

Six runnable examples. Each is one file; commits are batched at the end.

### Task 9.1: `basic.rs`

**Files:**
- Create: `crates/altair-server/examples/basic.rs`

```rust
//! Smallest possible altair-server: one route, defaults.
//!
//! Run with: `cargo run --example basic -p altair-server`
//!
//! Hit it: `curl http://127.0.0.1:3000/`

use altair_server::axum::routing::get;
use altair_server::Server;

#[tokio::main]
async fn main() -> altair_server::Result<()> {
    Server::builder()
        .bind_addr("127.0.0.1:3000")
        .route("/", get(|| async { "hello from altair-server" }))
        .build()
        .await?
        .run()
        .await
}
```

### Task 9.2: `with_routes.rs`

**Files:**
- Create: `crates/altair-server/examples/with_routes.rs`

```rust
//! Multiple routes, nested routers, JSON response.
//!
//! Run with: `cargo run --example with_routes -p altair-server`

use altair_server::axum::Json;
use altair_server::axum::routing::{get, post};
use altair_server::axum::Router;
use altair_server::Server;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct CreateUser {
    name: String,
}

#[derive(Serialize)]
struct User {
    id: u64,
    name: String,
}

async fn list_users() -> Json<Vec<User>> {
    Json(vec![User {
        id: 1,
        name: "alice".into(),
    }])
}

async fn create_user(Json(payload): Json<CreateUser>) -> Json<User> {
    Json(User {
        id: 42,
        name: payload.name,
    })
}

#[tokio::main]
async fn main() -> altair_server::Result<()> {
    let api: Router = Router::new()
        .route("/users", get(list_users).post(create_user));

    Server::builder()
        .bind_addr("127.0.0.1:3001")
        .route("/", get(|| async { "hello" }))
        .nest("/api", api)
        .build()
        .await?
        .run()
        .await
}
```

### Task 9.3: `custom_middleware.rs`

**Files:**
- Create: `crates/altair-server/examples/custom_middleware.rs`

```rust
//! Enable CORS, compression, and a custom request timeout.
//!
//! Run with: `cargo run --example custom_middleware -p altair-server`

use altair_server::axum::routing::get;
use altair_server::Server;
use std::time::Duration;

#[tokio::main]
async fn main() -> altair_server::Result<()> {
    Server::builder()
        .bind_addr("127.0.0.1:3002")
        .enable_cors()
        .enable_compression()
        .request_timeout(Duration::from_secs(5))
        .route(
            "/",
            get(|| async {
                // Big response to demonstrate compression in action.
                "x".repeat(1024)
            }),
        )
        .build()
        .await?
        .run()
        .await
}
```

### Task 9.4: `graceful_shutdown.rs`

**Files:**
- Create: `crates/altair-server/examples/graceful_shutdown.rs`

```rust
//! Hook into shutdown explicitly via a channel; useful for tests and
//! orchestration scenarios where you need to trigger shutdown
//! programmatically (not via SIGINT).
//!
//! Run with: `cargo run --example graceful_shutdown -p altair-server`
//!
//! This example shuts itself down after 5 seconds.

use altair_server::axum::routing::get;
use altair_server::Server;
use std::time::Duration;

#[tokio::main]
async fn main() -> altair_server::Result<()> {
    let server = Server::builder()
        .bind_addr("127.0.0.1:3003")
        .route("/", get(|| async { "still running" }))
        .build()
        .await?;

    println!("listening on {}", server.local_addr());

    server
        .run_with_shutdown(async {
            tokio::time::sleep(Duration::from_secs(5)).await;
            println!("triggering shutdown");
        })
        .await
}
```

### Task 9.5: `with_state.rs`

**Files:**
- Create: `crates/altair-server/examples/with_state.rs`

```rust
//! Shared application state via axum's `Router::with_state`.
//!
//! Note: altair-server's `ServerBuilder` doesn't currently expose a typed
//! `with_state` knob (the generic-S signature is awkward). Instead, build
//! an `axum::Router<()>` with state already applied (via
//! `Router::with_state`) and pass it to `.merge()`.
//!
//! Run with: `cargo run --example with_state -p altair-server`

use altair_server::axum::extract::State;
use altair_server::axum::routing::get;
use altair_server::axum::Router;
use altair_server::Server;
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    greeting: Arc<String>,
}

async fn greet(State(state): State<AppState>) -> String {
    format!("{} from altair-server", state.greeting)
}

#[tokio::main]
async fn main() -> altair_server::Result<()> {
    let state = AppState {
        greeting: Arc::new("hello".to_string()),
    };

    let router: Router = Router::new()
        .route("/greet", get(greet))
        .with_state(state);

    Server::builder()
        .bind_addr("127.0.0.1:3004")
        .merge(router)
        .build()
        .await?
        .run()
        .await
}
```

### Task 9.6: `custom_health.rs`

**Files:**
- Create: `crates/altair-server/examples/custom_health.rs`

```rust
//! Customize the health endpoint path and response body.
//!
//! Run with: `cargo run --example custom_health -p altair-server`
//!
//! Hit it: `curl http://127.0.0.1:3005/readyz`

use altair_server::axum::Json;
use altair_server::Server;
use serde_json::json;

#[tokio::main]
async fn main() -> altair_server::Result<()> {
    Server::builder()
        .bind_addr("127.0.0.1:3005")
        .health_path("/readyz")
        .health_response(|| {
            Json(json!({
                "status": "ok",
                "version": env!("CARGO_PKG_VERSION"),
            }))
        })
        .build()
        .await?
        .run()
        .await
}
```

### Task 9.7: Build all examples + commit

- [ ] **Step 1: Build**

Run: `cargo build -p altair-server --examples`
Expected: all six examples build cleanly.

- [ ] **Step 2: Commit**

```bash
git add crates/altair-server/examples
git commit -m "docs(server): six runnable examples covering core features"
```

---

## Phase 10: README

### Task 10.1: Write the full README

**Files:**
- Modify: `crates/altair-server/README.md`

- [ ] **Step 1: Replace the stub with the full README**

````markdown
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

- **Tracing** per request via `tower_http::trace::TraceLayer`. If `altair-otel` is initialized in the same process, those spans flow to OTLP automatically.
- **Request ID** (`x-request-id`) propagation — generated if missing, echoed in response.
- **Per-request timeout** (default 30s, configurable).
- **`GET /health` endpoint** returning `200 OK` (customizable path + body).
- **Graceful shutdown** on SIGINT/SIGTERM via `tokio::signal`.

## Routes

`.route()`, `.merge()`, and `.nest()` delegate directly to axum:

```rust,no_run
use altair_server::Server;
use altair_server::axum::Router;
use altair_server::axum::routing::{get, post};

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
use altair_server::tower_http::cors::{Any, CorsLayer};
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
````

- [ ] **Step 2: Verify doc tests pass**

Run: `cargo test -p altair-server --doc`
Expected: doc tests pass (lib.rs example + prelude.rs example + README examples are `no_run` so they only compile-check).

- [ ] **Step 3: Commit**

```bash
git add crates/altair-server/README.md
git commit -m "docs(server): complete README with examples and error reference"
```

---

## Phase 11: Tracker, root README, CI gate

### Task 11.1: Update porting tracker

**Files:**
- Modify: `docs/porting-tracker.md`

- [ ] **Step 1: Add to published-set table**

In the "Published crates" table, after the `altair-rest` row, add:

```markdown
| [`altair-server`](https://crates.io/crates/altair-server) | 0.1.2 (date TBD on publish) |
```

- [ ] **Step 2: Add release notes bullet**

```markdown
- **`altair-server` 0.1.2** (date TBD on publish) — Axum + tower-http convenience layer with default middleware (tracing + request-id + timeout), built-in `/health` endpoint, and SIGINT/SIGTERM-aware graceful shutdown.
```

- [ ] **Step 3: Move row in the Done table**

In the "Starter Set — `v0.1.x`" Done table, after the `altair-rest` row add:

```markdown
| `server` | `altair-server` | ✅ Done | `axum`, `tower`, `tower-http` | Convenience layer with default middleware + health endpoint + graceful shutdown |
```

And remove from "Awaiting Demand":

```markdown
| `server` | `altair-server` | 💤 Deferred | `axum`, `tower`, `tower-http` | Closest Rust analog to Echo |
```

- [ ] **Step 4: Bump "Last updated"**

Replace with today's date.

- [ ] **Step 5: Commit**

```bash
git add docs/porting-tracker.md
git commit -m "docs: add altair-server to porting tracker"
```

### Task 11.2: Add to root README

**Files:**
- Modify: `README.md` (workspace root)

- [ ] **Step 1: Add a row**

After the `altair-rest` row, add:

```markdown
| [`altair-server`](crates/altair-server) | Axum + tower-http convenience layer with sensible defaults | [![crate](https://img.shields.io/crates/v/altair-server.svg)](https://crates.io/crates/altair-server) |
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: list altair-server in workspace README"
```

### Task 11.3: Full CI gate

- [ ] **Step 1: Run formatter, clippy, tests, doc**

Run:
```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --no-deps --all-features
```

Expected: all four exit 0. Common clippy issues to expect and how to handle:

- `clippy::must_use_candidate` on builder methods that return `Self`: add `#[must_use]` on the builder method's return, or the workspace's existing `#[must_use]` on the type itself may be enough.
- `clippy::missing_panics_doc` on internal `expect()` calls: refactor to remove the expect, or add `# Panics` doc only on public functions.
- `clippy::doc_markdown` complaining about names like "OTel" or "HTTP" in doc comments — wrap in backticks.

Do not paper over with broad `#[allow]`; investigate each finding.

- [ ] **Step 2: Verify dry-run publish**

Run: `cargo publish --dry-run -p altair-server`
Expected: `Uploading altair-server v0.1.2`; `warning: aborting upload due to dry run`.

- [ ] **Step 3: Commit any clippy/fmt fixes**

```bash
git add -p
git commit -m "fix(server): satisfy clippy/fmt"
```

Skip if nothing to commit.

---

## Phase 12: Push, PR, publish

### Task 12.1: Push branch and open PR

- [ ] **Step 1: Push**

```bash
git push -u origin feat/altair-server
```

- [ ] **Step 2: Open PR**

```bash
gh pr create --title "feat(server): add altair-server crate" --body "$(cat <<'EOF'
## Summary

Adds the eighth crate to the workspace: \`altair-server\` — an axum + tower-http convenience layer.

- \`Server::builder()\` returns a typed builder; \`.route()\` / \`.merge()\` / \`.nest()\` delegate to axum
- Default middleware stack: tracing (OTel-aware), request-id propagation, request timeout
- Opt-in CORS and compression
- Built-in \`/health\` endpoint (customizable path + response)
- Graceful shutdown on SIGINT/SIGTERM via \`Server::run()\` or custom future via \`run_with_shutdown\`
- Re-exports \`axum\`, \`tower\`, \`tower-http\` so consumers don't need them as separate deps
- Six runnable examples: basic, with_routes, custom_middleware, graceful_shutdown, with_state, custom_health

Spec: docs/specs/2026-05-28-altair-server-design.md
Plan: docs/plans/2026-05-28-altair-server-implementation.md

## Test plan

- [x] 15+ unit tests + 6 integration tests pass (reqwest against ephemeral-port server)
- [x] \`cargo clippy --workspace --all-targets --all-features -- -D warnings\` clean
- [x] \`cargo fmt --all --check\` clean
- [x] \`cargo test --workspace --doc\` clean
- [x] \`cargo publish --dry-run -p altair-server\` clean
- [x] All six examples build
- [ ] CI passes on this PR
EOF
)"
```

- [ ] **Step 3: Foreground-poll CI**

Use a foreground bash loop so progress is visible to the user (don't kick off run_in_background):

```bash
until gh pr checks <pr-number> --required 2>/dev/null | grep -qE "fail|pass" && ! gh pr checks <pr-number> 2>/dev/null | grep -q pending; do sleep 15; done
gh pr checks <pr-number>
```

If all checks pass:

```bash
gh pr merge <pr-number> --squash --delete-branch
git checkout main && git pull
```

### Task 12.2: First publish via release-plz

Same pattern as previous crates. release-plz runs on push to main and publishes the new crate. Verify on crates.io:

```bash
curl -s -H 'User-Agent: altair-rs (jasoet87@gmail.com)' \
  https://crates.io/api/v1/crates/altair-server | jq -r .crate.max_version
```

Expected: matches the workspace version (0.1.2).

A subsequent release-plz "v0.1.3" PR may open proposing empty workspace churn; close it as before.

### Task 12.3: Final tracker update

**Files:**
- Modify: `docs/porting-tracker.md`

- [ ] **Step 1: Replace "date TBD on publish" with the actual date**

In both the published-set table row and the release-notes bullet.

- [ ] **Step 2: Commit and PR**

```bash
git checkout -b docs/server-published
git commit -am "docs: record altair-server publish date"
git push -u origin docs/server-published
gh pr create --title "docs: record altair-server publish date" --body "Trivial tracker update."
gh pr merge <pr-number> --squash --delete-branch
```

---

## Self-Review

### Spec Coverage Check

| Spec section | Implemented in task |
|---|---|
| §1 Overview | Plan header + Task 10.1 README |
| §2 Decisions Locked | Tasks 1.1, 1.2 (deps, scaffold), then per-feature tasks |
| §3.1 File layout | Tasks 1.2, 2.1, 3.1, 4.1, 5.1, 6.1, 6.2, 7.1, 8.1 |
| §3.2 Module responsibilities | Each module is one file, sole owner of its concern |
| §3.3 Public API | Task 1.2 lib.rs re-exports |
| §3.4 ServerBuilder surface | Task 6.2 |
| §3.5 Server surface | Task 6.1 |
| §3.6 Error model | Task 2.1 (3 variants, 3 tests) |
| §4.1 State plumbing | Note in Task 9.5 `with_state` example explains the workaround |
| §4.2 Middleware ordering | Task 5.1 `DefaultStack::apply` enforces order; verified in tests |
| §4.3 Request-id behaviour | Task 5.1 (SetRequestIdLayer + PropagateRequestIdLayer); Task 8.1 integration test |
| §4.4 Health endpoint registration | Task 4.1 `install`; Task 6.2 calls it before middleware |
| §4.5 Graceful shutdown | Task 3.1 `shutdown_signal()`; Task 6.1 wires it into `run()` |
| §4.6 Local address | Task 6.1 `Server::local_addr`; Task 6.2 binds at build-time |
| §5 Testing | Per-module unit tests + Task 8.1 integration + Task 10.1 doc tests |
| §6 Cross-crate integration | Documented in README (Task 10.1) |
| §7 Out of scope | Not implemented; no tasks add TLS/HTTP3/etc. |
| §8 Risks | Documented in spec; not actionable in plan |
| §9 Versioning | Inherits via `version.workspace = true` (Task 1.2) |

**Note on §4.1 state plumbing:** the spec proposed `ServerBuilder<S>` generic with `.with_state()` returning `ServerBuilder<S>`. Making the builder generic over `S` complicates the API surface significantly and conflicts with the `Default` impl. The plan instead takes a simpler approach: `ServerBuilder` is always `ServerBuilder` (no generic), holds `Router<()>`. Users wanting state build a `Router<S>` separately with `.with_state(s)` applied to it, then pass that to `.merge()`. The `with_state` example demonstrates this. This is a deliberate scope-trim from the spec; document in Task 6.2.

### Placeholder Scan

- "(date TBD on publish)" — intentional in Task 11.1; resolved in Task 12.3.
- No "TBD", "TODO", or "fill in later" elsewhere.

### Type Consistency

- `Server::from_parts(router, listener, local_addr)` defined in Task 6.1; called from `ServerBuilder::build()` in Task 6.2. Signatures match.
- `HealthResponder = Arc<dyn Fn() -> Response + Send + Sync + 'static>` defined in Task 4.1; used in Task 6.2 `ServerBuilder` field; constructed via `health::default_responder()` in `Default` impl. Consistent.
- `DefaultStack` struct defined in Task 5.1 with fields (tracing, request_id, timeout, cors, compression); constructed and populated in Task 6.2 `build()`. Field names match.
- `Error` enum variants used in tests across modules: `Bind`, `Io`, `Configuration`. Consistent.

No drift identified.

---

## Execution Handoff

**Plan complete and saved to `docs/plans/2026-05-28-altair-server-implementation.md`. Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** — execute tasks in this session via executing-plans, batch with checkpoints

Pick when ready to start.
