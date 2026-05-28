# altair-rest — Design

**Date:** 2026-05-28
**Status:** Draft — awaiting review before implementation planning
**Author:** Jasoet
**Spec type:** Brainstorming output → input to writing-plans

---

## 1. Overview

`altair-rest` is a resilient HTTP client for Rust applications. It wraps [`reqwest`](https://crates.io/crates/reqwest) (the de facto Rust HTTP client) with a configured middleware stack from [`reqwest-middleware`](https://crates.io/crates/reqwest-middleware), [`reqwest-retry`](https://crates.io/crates/reqwest-retry), and [`reqwest-tracing`](https://crates.io/crates/reqwest-tracing) — wired with sensible defaults so each outgoing request automatically gets retries on transient failures, OpenTelemetry-aware spans, and consistent connection/timeout behaviour.

**One-line product goal:** "Make a real-world HTTP client without assembling four middleware crates yourself."

The crate exposes a `Client` newtype that delegates to `reqwest_middleware::ClientWithMiddleware`. Users get the entire `reqwest::RequestBuilder` API they already know, with retries and tracing baked in — no extra code per call site. JSON helpers (`get_json`, `post_json`) cover the 80% case of REST-API consumption.

## 2. Decisions Locked

| Decision | Choice |
|---|---|
| Scope | Outbound HTTP client only. No server-side. |
| Implementation strategy | Wrap `reqwest` + `reqwest-middleware` + `reqwest-retry` + `reqwest-tracing` |
| Crate name | `altair-rest` (verified available on crates.io 2026-05-28) |
| API style | Typed builder (`Client::builder()...build()`) returning a `Client` newtype around `ClientWithMiddleware` |
| Retries | **Built-in** via `reqwest-retry` middleware. Defaults: 3 attempts, exponential 100ms → 5s. Configurable via builder. |
| Tracing | **On by default** via `reqwest-tracing`. Spans flow to OTLP if `altair-otel` is initialized. Disable via `enable_tracing(false)`. |
| Re-exports | `pub use ::reqwest;` and `pub use ::reqwest_middleware;` — users can drop to the underlying types without a separate dep |
| Error type | Single `thiserror` enum: `Middleware`, `Http`, `Decode`, `Url`, `InvalidHeader` |
| HTTP 4xx/5xx | NOT errors by default (matches reqwest). Users call `response.error_for_status()` to promote. |
| Backoff impl | `reqwest-retry` uses `retry-policies`; the workspace's other backoff impl is `backon` (in `altair-retry`). Two impls coexist — documented in README. |
| Async runtime | tokio (inherits from reqwest) |
| TLS | `reqwest`'s default features (rustls preferred when available). Workspace pins. |
| Dependencies | `reqwest`, `reqwest-middleware`, `reqwest-retry`, `reqwest-tracing`, `serde_json`, `url`, `thiserror`, `http` |
| Edition / MSRV | Inherit from workspace (Edition 2024, Rust 1.95) |

## 3. Architecture

### 3.1 File layout

```
crates/altair-rest/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs       # crate root: lints, mod declarations, re-exports, prelude
│   ├── error.rs     # Error enum + Result alias (thiserror)
│   ├── client.rs    # Client newtype around ClientWithMiddleware; delegating methods
│   ├── config.rs    # ClientBuilder (typed builder) + defaults
│   ├── json.rs      # get_json / post_json JSON helpers
│   └── prelude.rs   # one-import bundle
├── tests/
│   └── integration.rs    # wiremock-backed end-to-end tests
└── examples/
    ├── basic.rs
    ├── with_retry.rs
    ├── bearer_auth.rs
    ├── json_round_trip.rs
    ├── with_tracing.rs
    └── custom_middleware.rs
```

### 3.2 Module responsibilities

- **`error.rs`** — sole owner of the `Error` enum and `Result<T>` alias.
- **`client.rs`** — `Client` newtype around `reqwest_middleware::ClientWithMiddleware`. Provides delegating methods (`get`/`post`/`put`/`delete`/`patch`/`head`/`request`/`execute`) plus `base_url` resolution. JSON helpers live in a separate file but are added to the same `impl Client { ... }` block via `mod json;`.
- **`config.rs`** — `ClientBuilder` with builder methods for every knob. `build()` constructs the middleware chain (retry → tracing → reqwest_middleware::ClientWithMiddleware) and returns a `Client`. Holds `base_url` for relative-path resolution.
- **`json.rs`** — convenience helpers on `Client`: `get_json::<T>(url) -> Result<T>` and `post_json::<T, R>(url, &R) -> Result<T>`. Uses `serde_json` for decoding; non-serialization errors flow through `Error::Decode`.

### 3.3 Public API

```rust
// crate root re-exports
pub use client::Client;
pub use config::ClientBuilder;
pub use error::{Error, Result};

// Re-exports for one-dep ergonomics
pub use ::reqwest;
pub use ::reqwest_middleware;

pub mod prelude;
```

### 3.4 `Client` surface

```rust
impl Client {
    pub fn builder() -> ClientBuilder;

    // HTTP verb shortcuts (delegate; honour base_url if set)
    pub fn get(&self, url: &str) -> reqwest_middleware::RequestBuilder;
    pub fn post(&self, url: &str) -> reqwest_middleware::RequestBuilder;
    pub fn put(&self, url: &str) -> reqwest_middleware::RequestBuilder;
    pub fn delete(&self, url: &str) -> reqwest_middleware::RequestBuilder;
    pub fn patch(&self, url: &str) -> reqwest_middleware::RequestBuilder;
    pub fn head(&self, url: &str) -> reqwest_middleware::RequestBuilder;
    pub fn request(&self, method: reqwest::Method, url: &str) -> reqwest_middleware::RequestBuilder;

    // Direct execute for pre-built requests
    pub async fn execute(&self, request: reqwest::Request) -> Result<reqwest::Response>;

    // JSON helpers (the 80% case)
    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T>;
    pub async fn post_json<T: DeserializeOwned, R: Serialize + ?Sized>(
        &self,
        url: &str,
        body: &R,
    ) -> Result<T>;

    // Escape hatch for power users wanting the raw client
    pub fn inner(&self) -> &reqwest_middleware::ClientWithMiddleware;
}
```

The `RequestBuilder` returned by HTTP shortcuts is the standard `reqwest_middleware::RequestBuilder`, which exposes the full reqwest builder API (`.header()`, `.json()`, `.timeout()`, `.send()`, etc.). Calling `.send()` returns `Result<reqwest::Response, reqwest_middleware::Error>`; users `?` it through our `Error::from` conversion.

### 3.5 `ClientBuilder` surface

```rust
impl ClientBuilder {
    pub fn new() -> Self;                                         // alias for default()

    // URL & connection
    pub fn base_url(self, url: &str) -> Result<Self>;
    pub fn timeout(self, d: Duration) -> Self;
    pub fn connect_timeout(self, d: Duration) -> Self;
    pub fn user_agent(self, ua: impl Into<String>) -> Self;

    // Headers & auth
    pub fn default_header(self, name: &str, value: &str) -> Result<Self>;
    pub fn bearer_token(self, token: impl Into<String>) -> Self;
    pub fn basic_auth(self, user: impl Into<String>, password: Option<&str>) -> Self;

    // Retries (built-in via reqwest-retry)
    pub fn retry_max_attempts(self, n: u32) -> Self;     // 0 disables
    pub fn retry_initial_interval(self, d: Duration) -> Self;
    pub fn retry_max_interval(self, d: Duration) -> Self;

    // Tracing
    pub fn enable_tracing(self, on: bool) -> Self;

    // Power-user escape hatch: customize the underlying reqwest::ClientBuilder
    pub fn with_reqwest_builder<F>(self, customize: F) -> Self
    where
        F: FnOnce(reqwest::ClientBuilder) -> reqwest::ClientBuilder;

    // Power-user escape hatch: add custom middleware (e.g. auth refresh).
    // Custom middleware is appended AFTER the built-in retry and tracing
    // middleware — so the built-ins still see the original request and the
    // final response, and custom middleware can introspect/modify in between.
    pub fn with_middleware<M: reqwest_middleware::Middleware + 'static>(
        self,
        middleware: M,
    ) -> Self;

    pub fn build(self) -> Result<Client>;
}
```

**Defaults applied when nothing is set:**
- timeout = 30s
- connect_timeout = 10s
- user_agent = `altair-rest/<version>`
- 3 retries with exponential 100ms → 5s
- tracing on
- no base URL, no auth, no extra headers

### 3.6 Error model

```rust
#[derive(Debug, Error)]
pub enum Error {
    /// Middleware-stack failure (typical retry-exhausted / network error path)
    #[error("HTTP request failed: {0}")]
    Middleware(#[from] reqwest_middleware::Error),

    /// Raw reqwest error (less common — surfaces when not flowing through middleware)
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),

    /// Response body failed to deserialize as the requested type
    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),

    /// Bad URL — typically from ClientBuilder::base_url or relative-path resolution
    #[error("invalid URL: {0}")]
    Url(#[from] url::ParseError),

    /// Invalid HTTP header name or value (from ClientBuilder::default_header)
    #[error("invalid header: {0}")]
    InvalidHeader(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

**Intentional non-variants:**
- `Timeout`: `reqwest::Error::is_timeout()` distinguishes this case; we surface as `Http` and let callers introspect. Adding a `Timeout` variant would force runtime translation in many paths.
- `HttpStatus { code, body }`: HTTP 4xx/5xx are not errors by default (matching reqwest). Users call `response.error_for_status()` if they want status-promotion.

## 4. Behaviour Details

### 4.1 `base_url` resolution

Set via `ClientBuilder::base_url("https://api.example.com/v1/")?`.

- Relative paths like `"/users/42"` or `"users/42"` are joined to the base URL.
- Absolute URLs like `"https://other.host/x"` bypass the base.
- Resolution uses `url::Url::join` semantics.
- If `base_url` is unset, all paths must be absolute or `.get("/users")` will return `Error::Url`.

### 4.2 Default headers

Set via `default_header(name, value)` (repeatable). Applied to every request. `bearer_token` is sugar for `default_header("authorization", &format!("Bearer {token}"))`.

### 4.3 Retry policy

`reqwest-retry` retries:
- Network errors (connect/dns/protocol failures)
- HTTP responses 5xx and 408/429 (configurable in `reqwest-retry`)
- NOT 4xx (other than 408/429) — those are application errors

Backoff: exponential with jitter, controlled by `retry_initial_interval` and `retry_max_interval`. `retry_max_attempts(0)` disables retries (returns first failure immediately).

The retry middleware sits **before** the tracing middleware in the chain, so each individual attempt gets its own span — caller sees retry behaviour in traces.

### 4.4 Tracing

When `enable_tracing(true)` (default), `reqwest_tracing::TracingMiddleware` is in the chain. For each request it emits a span:

- name: `HTTP {method}` (e.g. `HTTP GET`)
- attributes: `http.method`, `http.url`, `http.scheme`, `http.target`, `http.host`, `http.status_code`, `http.response_content_length`, `otel.kind = client`
- span status: `ERROR` for 5xx, `UNSET` otherwise

If `altair_otel::Config::init()` ran earlier in the process, those spans flow to OTLP via the global subscriber.

### 4.5 JSON helpers

```rust
let user: User = client.get_json("/users/42").await?;
let created: User = client.post_json("/users", &NewUser { ... }).await?;
```

Implementation:
- `get_json`: build GET request, send, `response.bytes().await`, `serde_json::from_slice`.
- `post_json`: build POST with `.json(body)` (which sets `Content-Type: application/json`), send, decode response.
- Both call `response.error_for_status()` before decoding — 4xx/5xx surface as `Error::Http` (the underlying reqwest::Error path), not as a misleading `Decode` failure on an HTML error page.

## 5. Cross-Crate Integration

- **`altair-otel`**: tracing middleware is wired in by default. Initialize altair-otel before constructing the client and request spans appear in your OTLP backend.
- **`altair-retry`**: not directly integrated. The crate has its own retry middleware (`reqwest-retry`). For sophisticated retry policies (e.g. classify by response body), disable built-in retries via `retry_max_attempts(0)` and wrap individual calls in `altair_retry::retry(...)`.
- **`altair-config`**: not directly integrated. `ClientBuilder` accepts deserialized config values cleanly — define a struct with `Deserialize` and pass fields to the builder.

## 6. Testing Strategy

| Layer | Where | Run by |
|---|---|---|
| Unit (inline `#[cfg(test)]`) | each `src/*.rs` | `cargo test --lib` |
| Integration (wiremock-backed) | `tests/integration.rs` | `cargo test --tests` |
| Doc tests | `///` examples in source (mostly `no_run`) | bundled with `cargo test` |
| Example-as-test | `examples/*.rs` (compile only) | `cargo build --examples` |

**Dev-dep choice:** `wiremock = "0.6"` — async HTTP mock library, the standard for reqwest-ecosystem testing.

**Test obligations per module:**

| File | Tests |
|---|---|
| `error.rs` | Display rendering; From conversions for each `#[from]` |
| `config.rs` | Builder defaults; invalid URL rejection; invalid header name/value rejection; retry policy zero-attempts disable |
| `client.rs` | base_url join for relative paths; absolute URL bypasses base; default headers applied; bearer_token applied |
| `json.rs` | get_json round-trip; post_json round-trip; 5xx → Error::Middleware; non-JSON body → Error::Decode |
| `tests/integration.rs` | wiremock: 5xx retried then succeeds; 4xx not retried; 429 retried; timeout enforced; tracing span emitted |

**Coverage target:** ≥85% per file.

## 7. Out of Scope (v0.1.0)

- Streaming response bodies — use `Response::bytes_stream()` directly.
- File upload / multipart — use `reqwest::multipart::Form` directly.
- Cookie jar — expose via `with_reqwest_builder` escape-hatch.
- Proxy configuration — expose via `with_reqwest_builder` escape-hatch.
- Per-request retry override — disable global retries and use `altair_retry::retry()` for those calls.
- Mock server bundled in the crate — testing strategy is wiremock as a dev-dep.
- HTTP/2 push, gRPC — out of scope.
- Async drainage / connection-pool tuning — defaults plus `with_reqwest_builder` for tweaking.

## 8. Risks & Open Questions

| Item | Risk | Mitigation |
|---|---|---|
| `reqwest-middleware` v0.5 → v0.6 trait changes | Medium | Pin in `[workspace.dependencies]`; absorb upgrades as our own minor bumps |
| Two backoff impls in user binaries (`backon` via altair-retry, `retry-policies` via altair-rest) | Low — small binary-size hit, no behavioural conflict | Document in README; offer "disable built-in retries + compose altair-retry" pattern |
| `reqwest`'s TLS feature choice varies by platform | Medium | Pin features in workspace.deps; document |
| `enable_tracing` adds a span per request | Low — typical app overhead negligible | Default-on; document for hot-path use cases |
| Re-exporting `reqwest` and `reqwest_middleware` means their breaking changes are ours | Documented trade-off | Pin in workspace.deps; treat upgrade as our own minor bump |

## 9. Versioning

- Inherits `version.workspace = true` — first publish at the current workspace shared version.
- Re-exports of `reqwest` and `reqwest_middleware` are part of the public API. Upgrades to those crates become our own minor bumps.
- Promotion to `1.0.0` deferred until the workspace as a whole stabilizes.

## 10. Next Steps

1. **User reviews this spec** (current step)
2. On approval: `writing-plans` skill produces an implementation plan
3. Implementation plan drives: crate scaffolding → per-module TDD → testing → CI → publish at workspace version
