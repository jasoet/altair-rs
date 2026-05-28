# altair-rest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build, test, and publish `altair-rest` — a resilient HTTP client wrapping `reqwest` with built-in retry middleware (`reqwest-retry`) and OTel-aware tracing middleware (`reqwest-tracing`) — to crates.io at the current workspace version.

**Architecture:** Single crate under `crates/altair-rest/`. Six source files (`lib.rs`, `error.rs`, `client.rs`, `config.rs`, `json.rs`, `prelude.rs`). `Client` is a newtype around `reqwest_middleware::ClientWithMiddleware`. `ClientBuilder` is a typed builder that assembles the middleware chain (retry → tracing → custom) and produces a `Client`. JSON helpers (`get_json` / `post_json`) live in `json.rs` as additional `impl Client` methods.

**Tech Stack:**
- Rust 2024, MSRV 1.95 (inherit from workspace)
- `reqwest = "0.13"` (default features)
- `reqwest-middleware = "0.5"` — middleware chain framework
- `reqwest-retry = "0.9"` — exponential backoff retry middleware
- `reqwest-tracing = "0.7"` — OpenTelemetry-aware tracing middleware
- `serde_json = "1"` — JSON helpers
- `url = "2"` — URL parsing
- `http = "1"` — HeaderName/HeaderValue construction
- `thiserror = "2"` (workspace)

Dev-deps:
- `wiremock = "0.6"` — async HTTP mock for integration tests
- `tokio` with `macros` + `rt-multi-thread` features
- `pretty_assertions`, `anyhow`, `serde` (with derive)

---

## File Structure

```
crates/altair-rest/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs       # crate root: lints, mod declarations, re-exports
│   ├── error.rs     # Error enum + Result alias
│   ├── client.rs    # Client newtype + delegating methods (get/post/etc.)
│   ├── config.rs    # ClientBuilder + middleware chain construction
│   ├── json.rs      # get_json / post_json helpers on Client
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

Workspace edits:
- `Cargo.toml`: add `reqwest`, `reqwest-middleware`, `reqwest-retry`, `reqwest-tracing`, `serde_json`, `url`, `http`, `wiremock` to `[workspace.dependencies]`; add `crates/altair-rest` to `members`
- `docs/porting-tracker.md`: move `altair-rest` from Awaiting Demand → Done; add release-notes bullet
- `README.md`: add `altair-rest` row to crate table

---

## Phase 1: Crate Scaffold

### Task 1.1: Add libraries to workspace dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Add deps**

In the root `Cargo.toml`'s `[workspace.dependencies]` block, add a new "# HTTP client" section. Place it near the existing `# Archiving / compression` section. Add to `[workspace.dependencies]`:

```toml
# HTTP client
reqwest = { version = "0.13", default-features = true }
reqwest-middleware = "0.5"
reqwest-retry = "0.9"
reqwest-tracing = "0.7"
serde_json = "1"
url = "2"
http = "1"
wiremock = "0.6"
```

`reqwest`'s default features include the `default-tls` (rustls when available) and the `json` feature.

- [ ] **Step 2: Verify workspace parses**

Run: `cargo metadata --format-version=1 > /dev/null`
Expected: exit 0.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add reqwest stack + wiremock to workspace dependencies"
```

### Task 1.2: Create crate skeleton

**Files:**
- Create: `crates/altair-rest/Cargo.toml`
- Create: `crates/altair-rest/src/lib.rs`
- Create: `crates/altair-rest/README.md` (stub)
- Modify: `Cargo.toml` (workspace `members`)

- [ ] **Step 1: Create directories**

```bash
mkdir -p crates/altair-rest/src crates/altair-rest/tests crates/altair-rest/examples
```

- [ ] **Step 2: Write `crates/altair-rest/Cargo.toml`**

```toml
[package]
name = "altair-rest"
description = "Resilient HTTP client built on reqwest with retry and OTel-aware tracing baked in"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
homepage.workspace = true
readme = "README.md"
keywords = ["http", "client", "reqwest", "retry", "tracing"]
categories = ["network-programming", "web-programming::http-client"]

[dependencies]
reqwest = { workspace = true }
reqwest-middleware = { workspace = true }
reqwest-retry = { workspace = true }
reqwest-tracing = { workspace = true }
serde_json = { workspace = true }
url = { workspace = true }
http = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
pretty_assertions = { workspace = true }
anyhow = { workspace = true }
wiremock = { workspace = true }
serde = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 3: Write `crates/altair-rest/src/lib.rs`**

```rust
//! Resilient HTTP client built on `reqwest` with retry and `OTel`-aware
//! tracing middleware baked in.
//!
//! Wraps `reqwest_middleware::ClientWithMiddleware` with a sensible-default
//! middleware chain so each outgoing request automatically gets exponential
//! backoff on transient failures plus a per-request tracing span. The
//! underlying `reqwest` and `reqwest_middleware` crates are re-exported
//! at the crate root so consumers don't need to add them separately.
//!
//! # Example
//!
//! ```no_run
//! use altair_rest::Client;
//!
//! # async fn run() -> altair_rest::Result<()> {
//! let client = Client::builder()
//!     .base_url("https://api.example.com")?
//!     .bearer_token("secret-token")
//!     .build()?;
//!
//! let response = client.get("/users/42").send().await?;
//! # let _ = response;
//! # Ok(()) }
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod client;
mod config;
mod error;
mod json;

pub mod prelude;

pub use client::Client;
pub use config::ClientBuilder;
pub use error::{Error, Result};

// Re-exports for one-dep ergonomics
pub use ::reqwest;
pub use ::reqwest_middleware;
```

- [ ] **Step 4: Write stub README**

```markdown
# altair-rest

Resilient HTTP client built on reqwest with retry and OTel-aware tracing baked in.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

(Full README added in a later task.)
```

- [ ] **Step 5: Register in workspace `members`**

In root `Cargo.toml`, append `"crates/altair-rest"` to the `members` list:

```toml
members = [
    "crates/altair-concurrent",
    "crates/altair-retry",
    "crates/altair-config",
    "crates/altair-otel",
    "crates/altair-base32",
    "crates/altair-compress",
    "crates/altair-rest",
]
```

- [ ] **Step 6: Verify workspace parses**

Run: `cargo metadata --format-version=1 > /dev/null`
Expected: exit 0. (`cargo build -p altair-rest` will fail with "file not found" — expected.)

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/altair-rest
git commit -m "feat(rest): scaffold altair-rest crate"
```

---

## Phase 2: Error type

### Task 2.1: Write `error.rs` with tests

**Files:**
- Create: `crates/altair-rest/src/error.rs`

- [ ] **Step 1: Write the file**

```rust
//! Crate-wide error type for `altair-rest`.

use thiserror::Error;

/// Errors returned by `altair-rest` operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Middleware-chain failure — the typical retry-exhausted / network-error
    /// path. Wraps a `reqwest_middleware::Error`.
    #[error("HTTP request failed: {0}")]
    Middleware(#[from] reqwest_middleware::Error),

    /// Raw `reqwest::Error` — surfaces when a path doesn't flow through the
    /// middleware stack (e.g. `error_for_status` results after middleware
    /// has already returned the response).
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),

    /// Response body failed to deserialize as the requested type
    /// (`get_json` / `post_json`).
    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),

    /// Bad URL — typically from [`crate::ClientBuilder::base_url`] or
    /// from relative-path resolution at request time.
    #[error("invalid URL: {0}")]
    Url(#[from] url::ParseError),

    /// Invalid HTTP header name or value (from
    /// [`crate::ClientBuilder::default_header`]).
    #[error("invalid header: {0}")]
    InvalidHeader(String),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_error_renders() {
        let json_err: serde_json::Error =
            serde_json::from_str::<u32>("not a number").unwrap_err();
        let e: Error = json_err.into();
        assert!(e.to_string().starts_with("decode error:"));
    }

    #[test]
    fn url_error_renders() {
        let url_err: url::ParseError = "not a url".parse::<url::Url>().unwrap_err();
        let e: Error = url_err.into();
        assert!(e.to_string().starts_with("invalid URL:"));
    }

    #[test]
    fn invalid_header_renders() {
        let e = Error::InvalidHeader("name contains spaces".into());
        assert_eq!(e.to_string(), "invalid header: name contains spaces");
    }
}
```

- [ ] **Step 2: Run tests**

The crate won't link yet because `client.rs`, `config.rs`, etc. don't exist. Use the temporary-comment-out trick: in `crates/altair-rest/src/lib.rs`, comment out `mod client; mod config; mod json; pub mod prelude;` and their `pub use` lines. Run `cargo test -p altair-rest --lib error`. Expected: 3 tests pass. Restore lib.rs before committing.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-rest/src/error.rs
git commit -m "feat(rest): add Error type and Result alias"
```

Only `error.rs` should be in the commit.

---

## Phase 3: ClientBuilder + middleware chain

This phase is the heart of the crate — it constructs the typed builder, assembles the middleware chain with retry + tracing, and produces a Client.

### Task 3.1: Write `config.rs` with tests

**Files:**
- Create: `crates/altair-rest/src/config.rs`

- [ ] **Step 1: Write the file**

```rust
//! Typed `ClientBuilder` and the middleware-chain construction.

use crate::client::Client;
use crate::error::{Error, Result};
use http::header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use reqwest_middleware::{ClientBuilder as MiddlewareBuilder, Middleware};
use reqwest_retry::{
    RetryTransientMiddleware,
    policies::ExponentialBackoff,
};
use reqwest_tracing::TracingMiddleware;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_RETRY_INITIAL: Duration = Duration::from_millis(100);
const DEFAULT_RETRY_MAX: Duration = Duration::from_secs(5);
const DEFAULT_RETRY_ATTEMPTS: u32 = 3;
const DEFAULT_USER_AGENT: &str = concat!("altair-rest/", env!("CARGO_PKG_VERSION"));

/// Typed builder for [`Client`].
///
/// Construct via [`Client::builder`].
#[must_use]
pub struct ClientBuilder {
    base_url: Option<Url>,
    timeout: Duration,
    connect_timeout: Duration,
    user_agent: String,
    headers: HeaderMap,
    bearer_token: Option<String>,
    basic_auth: Option<(String, Option<String>)>,
    retry_max_attempts: u32,
    retry_initial_interval: Duration,
    retry_max_interval: Duration,
    enable_tracing: bool,
    reqwest_customize: Option<Box<dyn FnOnce(reqwest::ClientBuilder) -> reqwest::ClientBuilder + Send>>,
    extra_middleware: Vec<Arc<dyn Middleware>>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(DEFAULT_USER_AGENT),
        );
        Self {
            base_url: None,
            timeout: DEFAULT_TIMEOUT,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            user_agent: DEFAULT_USER_AGENT.to_string(),
            headers,
            bearer_token: None,
            basic_auth: None,
            retry_max_attempts: DEFAULT_RETRY_ATTEMPTS,
            retry_initial_interval: DEFAULT_RETRY_INITIAL,
            retry_max_interval: DEFAULT_RETRY_MAX,
            enable_tracing: true,
            reqwest_customize: None,
            extra_middleware: Vec::new(),
        }
    }
}

impl ClientBuilder {
    /// Create a builder with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the base URL for relative path resolution. Must end with a `/` if
    /// you want subpaths to nest underneath; e.g. `"https://api.example.com/v1/"`.
    pub fn base_url(mut self, url: &str) -> Result<Self> {
        let parsed = Url::parse(url)?;
        self.base_url = Some(parsed);
        Ok(self)
    }

    /// Total request timeout.
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    /// TCP connect timeout.
    pub fn connect_timeout(mut self, d: Duration) -> Self {
        self.connect_timeout = d;
        self
    }

    /// Override the `User-Agent` header. Default: `altair-rest/<version>`.
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        let ua_string = ua.into();
        self.user_agent = ua_string.clone();
        if let Ok(value) = HeaderValue::from_str(&ua_string) {
            self.headers.insert(USER_AGENT, value);
        }
        self
    }

    /// Add a default header applied to every request.
    pub fn default_header(mut self, name: &str, value: &str) -> Result<Self> {
        let header_name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|e| Error::InvalidHeader(format!("name '{name}': {e}")))?;
        let header_value =
            HeaderValue::from_str(value).map_err(|e| Error::InvalidHeader(format!("value: {e}")))?;
        self.headers.insert(header_name, header_value);
        Ok(self)
    }

    /// Set a Bearer token. Equivalent to `default_header("authorization", "Bearer <token>")`.
    pub fn bearer_token(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    /// Set HTTP Basic auth credentials. Applied to every request.
    pub fn basic_auth(mut self, user: impl Into<String>, password: Option<&str>) -> Self {
        self.basic_auth = Some((user.into(), password.map(str::to_string)));
        self
    }

    /// Maximum number of retry attempts. `0` disables retries entirely.
    pub fn retry_max_attempts(mut self, n: u32) -> Self {
        self.retry_max_attempts = n;
        self
    }

    /// Initial retry backoff interval.
    pub fn retry_initial_interval(mut self, d: Duration) -> Self {
        self.retry_initial_interval = d;
        self
    }

    /// Maximum retry backoff interval (caps exponential growth).
    pub fn retry_max_interval(mut self, d: Duration) -> Self {
        self.retry_max_interval = d;
        self
    }

    /// Toggle the built-in tracing middleware.
    pub fn enable_tracing(mut self, on: bool) -> Self {
        self.enable_tracing = on;
        self
    }

    /// Escape hatch: customize the underlying `reqwest::ClientBuilder`.
    pub fn with_reqwest_builder<F>(mut self, customize: F) -> Self
    where
        F: FnOnce(reqwest::ClientBuilder) -> reqwest::ClientBuilder + Send + 'static,
    {
        self.reqwest_customize = Some(Box::new(customize));
        self
    }

    /// Escape hatch: append a custom middleware to the chain. Custom middleware
    /// runs AFTER the built-in retry and tracing middleware.
    pub fn with_middleware<M>(mut self, middleware: M) -> Self
    where
        M: Middleware + 'static,
    {
        self.extra_middleware.push(Arc::new(middleware));
        self
    }

    /// Build the configured [`Client`].
    pub fn build(self) -> Result<Client> {
        // 1) Construct the inner reqwest client.
        let mut reqwest_builder = reqwest::ClientBuilder::new()
            .timeout(self.timeout)
            .connect_timeout(self.connect_timeout)
            .default_headers(self.headers.clone());

        if let Some(customize) = self.reqwest_customize {
            reqwest_builder = customize(reqwest_builder);
        }

        let reqwest_client = reqwest_builder.build()?;

        // 2) Assemble the middleware chain.
        let mut chain = MiddlewareBuilder::new(reqwest_client);

        if self.retry_max_attempts > 0 {
            let policy = ExponentialBackoff::builder()
                .retry_bounds(self.retry_initial_interval, self.retry_max_interval)
                .build_with_max_retries(self.retry_max_attempts);
            chain = chain.with(RetryTransientMiddleware::new_with_policy(policy));
        }

        if self.enable_tracing {
            chain = chain.with(TracingMiddleware::default());
        }

        for middleware in self.extra_middleware {
            chain = chain.with_arc(middleware);
        }

        let inner = chain.build();

        Ok(Client::from_parts(
            inner,
            self.base_url,
            self.bearer_token,
            self.basic_auth,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn default_user_agent_has_crate_version() {
        let cb = ClientBuilder::default();
        let ua = cb.headers.get(USER_AGENT).unwrap().to_str().unwrap();
        assert!(ua.starts_with("altair-rest/"));
    }

    #[test]
    fn base_url_parsing_accepts_valid_url() {
        let cb = ClientBuilder::new().base_url("https://api.example.com/").unwrap();
        assert!(cb.base_url.is_some());
    }

    #[test]
    fn base_url_rejects_invalid_url() {
        let res = ClientBuilder::new().base_url("not a url");
        assert!(matches!(res, Err(Error::Url(_))));
    }

    #[test]
    fn default_header_rejects_invalid_name() {
        let res = ClientBuilder::new().default_header("name with space", "v");
        assert!(matches!(res, Err(Error::InvalidHeader(_))));
    }

    #[test]
    fn default_header_rejects_invalid_value() {
        let res = ClientBuilder::new().default_header("x-custom", "value\nwith\nnewlines");
        assert!(matches!(res, Err(Error::InvalidHeader(_))));
    }

    #[test]
    fn retry_zero_attempts_builds_without_retry_middleware() {
        let client = ClientBuilder::new().retry_max_attempts(0).build();
        assert!(client.is_ok());
    }

    #[test]
    fn bearer_token_is_stored() {
        let cb = ClientBuilder::new().bearer_token("xyz");
        assert_eq!(cb.bearer_token.as_deref(), Some("xyz"));
    }

    #[test]
    fn build_default_succeeds() {
        let client = ClientBuilder::new().build();
        assert!(client.is_ok());
    }
}
```

- [ ] **Step 2: Run tests**

Again, you'll need to temporarily comment out the modules that don't yet exist in `lib.rs` (`mod client; mod json; pub mod prelude;` and their re-exports). Run `cargo test -p altair-rest --lib config`. Expected: 8 tests pass. Restore lib.rs before committing.

Note: this test step requires `client.rs` to exist (since `config.rs` calls `Client::from_parts`). Apply the next task's `client.rs` first or stub `Client::from_parts` temporarily — see Task 3.2. Easier: write `client.rs` (Task 3.2) first, then come back to verify both together.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-rest/src/config.rs
git commit -m "feat(rest): add ClientBuilder with retry + tracing middleware chain"
```

### Task 3.2: Write `client.rs` with tests

**Files:**
- Create: `crates/altair-rest/src/client.rs`

- [ ] **Step 1: Write the file**

```rust
//! `Client` newtype around `reqwest_middleware::ClientWithMiddleware`.

use crate::config::ClientBuilder;
use crate::error::{Error, Result};
use reqwest_middleware::{ClientWithMiddleware, RequestBuilder};
use url::Url;

/// HTTP client with retry + tracing middleware baked in.
///
/// Construct via [`Client::builder`]. The client is cheap to clone and
/// uses an internal connection pool — share one instance across your app.
#[derive(Clone)]
pub struct Client {
    inner: ClientWithMiddleware,
    base_url: Option<Url>,
    bearer_token: Option<String>,
    basic_auth: Option<(String, Option<String>)>,
}

impl Client {
    /// Start building a new client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Internal constructor used by [`ClientBuilder::build`].
    pub(crate) fn from_parts(
        inner: ClientWithMiddleware,
        base_url: Option<Url>,
        bearer_token: Option<String>,
        basic_auth: Option<(String, Option<String>)>,
    ) -> Self {
        Self {
            inner,
            base_url,
            bearer_token,
            basic_auth,
        }
    }

    /// Access the underlying `reqwest_middleware::ClientWithMiddleware`.
    /// Use this if you need to call into the library directly.
    #[must_use]
    pub fn inner(&self) -> &ClientWithMiddleware {
        &self.inner
    }

    /// Resolve a relative or absolute URL against the optional base.
    pub(crate) fn resolve_url(&self, url: &str) -> Result<Url> {
        if let Some(base) = &self.base_url {
            base.join(url).map_err(Error::from)
        } else {
            Url::parse(url).map_err(Error::from)
        }
    }

    fn prepare(&self, builder: RequestBuilder) -> RequestBuilder {
        let mut builder = builder;
        if let Some(token) = &self.bearer_token {
            builder = builder.bearer_auth(token);
        }
        if let Some((user, password)) = &self.basic_auth {
            builder = builder.basic_auth(user, password.as_deref());
        }
        builder
    }

    /// Build a GET request.
    pub fn get(&self, url: &str) -> RequestBuilder {
        match self.resolve_url(url) {
            Ok(u) => self.prepare(self.inner.get(u)),
            // If URL resolution fails, return a request builder anyway with
            // the bad URL — `.send()` will surface the error path. Easier
            // than returning Result from every verb.
            Err(_) => self.prepare(self.inner.get(url)),
        }
    }

    /// Build a POST request.
    pub fn post(&self, url: &str) -> RequestBuilder {
        match self.resolve_url(url) {
            Ok(u) => self.prepare(self.inner.post(u)),
            Err(_) => self.prepare(self.inner.post(url)),
        }
    }

    /// Build a PUT request.
    pub fn put(&self, url: &str) -> RequestBuilder {
        match self.resolve_url(url) {
            Ok(u) => self.prepare(self.inner.put(u)),
            Err(_) => self.prepare(self.inner.put(url)),
        }
    }

    /// Build a DELETE request.
    pub fn delete(&self, url: &str) -> RequestBuilder {
        match self.resolve_url(url) {
            Ok(u) => self.prepare(self.inner.delete(u)),
            Err(_) => self.prepare(self.inner.delete(url)),
        }
    }

    /// Build a PATCH request.
    pub fn patch(&self, url: &str) -> RequestBuilder {
        match self.resolve_url(url) {
            Ok(u) => self.prepare(self.inner.patch(u)),
            Err(_) => self.prepare(self.inner.patch(url)),
        }
    }

    /// Build a HEAD request.
    pub fn head(&self, url: &str) -> RequestBuilder {
        match self.resolve_url(url) {
            Ok(u) => self.prepare(self.inner.head(u)),
            Err(_) => self.prepare(self.inner.head(url)),
        }
    }

    /// Build a request with a custom method.
    pub fn request(&self, method: reqwest::Method, url: &str) -> RequestBuilder {
        match self.resolve_url(url) {
            Ok(u) => self.prepare(self.inner.request(method, u)),
            Err(_) => self.prepare(self.inner.request(method, url)),
        }
    }

    /// Execute a pre-built request.
    pub async fn execute(&self, request: reqwest::Request) -> Result<reqwest::Response> {
        self.inner.execute(request).await.map_err(Error::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_client() -> Client {
        Client::builder()
            .base_url("https://api.example.com/v1/")
            .unwrap()
            .build()
            .unwrap()
    }

    #[test]
    fn resolve_url_joins_relative_path() {
        let c = test_client();
        let url = c.resolve_url("users/42").unwrap();
        assert_eq!(url.as_str(), "https://api.example.com/v1/users/42");
    }

    #[test]
    fn resolve_url_absolute_overrides_base() {
        let c = test_client();
        let url = c.resolve_url("https://other.example.com/path").unwrap();
        assert_eq!(url.as_str(), "https://other.example.com/path");
    }

    #[test]
    fn resolve_url_without_base_requires_absolute() {
        let c = Client::builder().build().unwrap();
        // Absolute works
        assert!(c.resolve_url("https://example.com/x").is_ok());
        // Relative fails (no base to join against)
        assert!(c.resolve_url("/x").is_err());
    }

    #[tokio::test]
    async fn client_is_clone() {
        let c1 = Client::builder().build().unwrap();
        let _c2 = c1.clone();
    }
}
```

- [ ] **Step 2: Run tests**

The crate should now mostly link (still missing `json.rs` and `prelude.rs`). Temporarily comment out `mod json; pub mod prelude;` and their re-exports in lib.rs.

Run: `cargo test -p altair-rest --lib client`
Expected: 4 tests pass.

Run: `cargo test -p altair-rest --lib config`
Expected: 8 tests pass (from Task 3.1).

Restore lib.rs before committing.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-rest/src/client.rs
git commit -m "feat(rest): add Client newtype with delegating HTTP verbs and base_url resolution"
```

---

## Phase 4: JSON helpers

### Task 4.1: Write `json.rs` with tests

**Files:**
- Create: `crates/altair-rest/src/json.rs`

- [ ] **Step 1: Write the file**

```rust
//! JSON request/response helpers on [`Client`].

use crate::client::Client;
use crate::error::{Error, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

impl Client {
    /// GET the URL and decode the response body as JSON.
    ///
    /// Calls `response.error_for_status()` before decoding — 4xx/5xx responses
    /// surface as [`Error::Http`], not as a misleading [`Error::Decode`] on
    /// an HTML error page.
    pub async fn get_json<T>(&self, url: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let response = self.get(url).send().await.map_err(Error::from)?;
        let response = response.error_for_status().map_err(Error::from)?;
        let bytes = response.bytes().await.map_err(Error::from)?;
        let value = serde_json::from_slice(&bytes)?;
        Ok(value)
    }

    /// POST a JSON body and decode the response body as JSON.
    pub async fn post_json<T, R>(&self, url: &str, body: &R) -> Result<T>
    where
        T: DeserializeOwned,
        R: Serialize + ?Sized,
    {
        let response = self.post(url).json(body).send().await.map_err(Error::from)?;
        let response = response.error_for_status().map_err(Error::from)?;
        let bytes = response.bytes().await.map_err(Error::from)?;
        let value = serde_json::from_slice(&bytes)?;
        Ok(value)
    }
}

// json.rs has no inline unit tests — the JSON helpers' behaviour is verified
// in tests/integration.rs against a wiremock server.
```

- [ ] **Step 2: Run tests**

Temporarily comment out `pub mod prelude;` in lib.rs. Run:

`cargo build -p altair-rest --lib`
Expected: clean build.

Restore lib.rs before committing.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-rest/src/json.rs
git commit -m "feat(rest): add get_json and post_json helpers on Client"
```

---

## Phase 5: Prelude module

### Task 5.1: Write `prelude.rs`

**Files:**
- Create: `crates/altair-rest/src/prelude.rs`

- [ ] **Step 1: Write the file**

```rust
//! Common imports for users of this crate.
//!
//! ```no_run
//! use altair_rest::prelude::*;
//!
//! # async fn run() -> altair_rest::Result<()> {
//! let client = Client::builder().build()?;
//! let _ = client.get("https://example.com").send().await?;
//! # Ok(()) }
//! ```

pub use crate::{Client, ClientBuilder, Error, Result};
```

- [ ] **Step 2: Verify the whole crate compiles and unit tests pass**

Run: `cargo test -p altair-rest --lib`
Expected: all unit tests pass (3 from error + 8 from config + 4 from client = 15).

Run: `cargo test -p altair-rest --doc`
Expected: doc tests pass (lib.rs example + prelude.rs example).

- [ ] **Step 3: Commit**

```bash
git add crates/altair-rest/src/prelude.rs
git commit -m "feat(rest): add prelude module"
```

---

## Phase 6: Integration tests (wiremock)

### Task 6.1: Write integration tests

**Files:**
- Create: `crates/altair-rest/tests/integration.rs`

- [ ] **Step 1: Write the file**

```rust
//! End-to-end behaviour tests using wiremock as an in-process HTTP server.

use altair_rest::prelude::*;
use pretty_assertions::assert_eq;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Respond, ResponseTemplate};

#[derive(Deserialize, Serialize, Debug, PartialEq)]
struct User {
    id: u64,
    name: String,
}

#[tokio::test]
async fn get_json_round_trip() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(User {
                    id: 1,
                    name: "alice".into(),
                }),
        )
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .build()
        .unwrap();

    let user: User = client.get_json("/users/1").await.unwrap();
    assert_eq!(
        user,
        User {
            id: 1,
            name: "alice".into()
        }
    );
}

#[tokio::test]
async fn post_json_round_trip() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/users"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(User {
                    id: 2,
                    name: "bob".into(),
                }),
        )
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .build()
        .unwrap();

    let new_user = User {
        id: 0,
        name: "bob".into(),
    };
    let created: User = client.post_json("/users", &new_user).await.unwrap();
    assert_eq!(created.id, 2);
}

#[tokio::test]
async fn retries_on_5xx_then_succeeds() {
    // Custom responder that fails twice then succeeds.
    struct Flaky {
        counter: Arc<AtomicU32>,
    }
    impl Respond for Flaky {
        fn respond(&self, _: &wiremock::Request) -> ResponseTemplate {
            let n = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
            if n < 3 {
                ResponseTemplate::new(503)
            } else {
                ResponseTemplate::new(200).set_body_string("ok")
            }
        }
    }

    let counter = Arc::new(AtomicU32::new(0));
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/flaky"))
        .respond_with(Flaky {
            counter: counter.clone(),
        })
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .retry_initial_interval(Duration::from_millis(10))
        .retry_max_interval(Duration::from_millis(50))
        .build()
        .unwrap();

    let response = client.get("/flaky").send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn does_not_retry_on_400() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bad"))
        .respond_with(move |_: &wiremock::Request| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(400)
        })
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .retry_initial_interval(Duration::from_millis(10))
        .build()
        .unwrap();

    let response = client.get("/bad").send().await.unwrap();
    assert_eq!(response.status(), 400);
    // 400 is a client error — should not be retried.
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn bearer_token_is_applied() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/secure"))
        .and(wiremock::matchers::header(
            "authorization",
            "Bearer my-secret-token",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .bearer_token("my-secret-token")
        .build()
        .unwrap();

    let response = client.get("/secure").send().await.unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn default_headers_are_applied() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/with-header"))
        .and(wiremock::matchers::header("x-tenant", "acme"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .default_header("x-tenant", "acme")
        .unwrap()
        .build()
        .unwrap();

    let response = client.get("/with-header").send().await.unwrap();
    assert_eq!(response.status(), 200);
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p altair-rest --tests`
Expected: 6 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-rest/tests/integration.rs
git commit -m "test(rest): wiremock integration tests for json, retries, headers"
```

---

## Phase 7: Examples

### Task 7.1: basic.rs

**Files:**
- Create: `crates/altair-rest/examples/basic.rs`

- [ ] **Step 1: Write the file**

```rust
//! Make a single GET request with default settings.
//!
//! Run with: `cargo run --example basic -p altair-rest`

use altair_rest::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::builder().build()?;
    let response = client.get("https://httpbin.org/get").send().await?;
    println!("status: {}", response.status());
    let body = response.text().await?;
    println!("body (first 200 chars):\n{}", &body[..body.len().min(200)]);
    Ok(())
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p altair-rest --example basic`
Expected: clean build.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-rest/examples/basic.rs
git commit -m "docs(rest): basic usage example"
```

### Task 7.2: with_retry.rs

**Files:**
- Create: `crates/altair-rest/examples/with_retry.rs`

- [ ] **Step 1: Write the file**

```rust
//! Configure retry behaviour: more attempts, custom backoff.
//!
//! Run with: `cargo run --example with_retry -p altair-rest`

use altair_rest::Client;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::builder()
        .retry_max_attempts(5)
        .retry_initial_interval(Duration::from_millis(50))
        .retry_max_interval(Duration::from_secs(2))
        .build()?;

    // Point at a flaky service. The middleware will retry on 5xx and
    // network errors up to 5 times with exponential backoff.
    let response = client.get("https://httpbin.org/status/200").send().await?;
    println!("status: {}", response.status());

    // Disable retries entirely:
    let strict = Client::builder().retry_max_attempts(0).build()?;
    let response = strict.get("https://httpbin.org/status/200").send().await?;
    println!("strict status: {}", response.status());

    Ok(())
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p altair-rest --example with_retry`
Expected: clean build.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-rest/examples/with_retry.rs
git commit -m "docs(rest): with_retry example"
```

### Task 7.3: bearer_auth.rs

**Files:**
- Create: `crates/altair-rest/examples/bearer_auth.rs`

- [ ] **Step 1: Write the file**

```rust
//! Construct an API client with a Bearer token and a default tenant header.
//!
//! Run with: `cargo run --example bearer_auth -p altair-rest`

use altair_rest::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::builder()
        .base_url("https://httpbin.org")?
        .bearer_token("ya29.example-token-value")
        .default_header("x-tenant", "acme")?
        .build()?;

    let response = client.get("/headers").send().await?;
    println!("status: {}", response.status());
    let body: serde_json::Value = response.json().await?;
    println!("headers as seen by server:\n{}", serde_json::to_string_pretty(&body)?);
    Ok(())
}
```

- [ ] **Step 2: Add dev-dep**

We need `serde_json` for the example output. It's in `[workspace.dependencies]` but not yet in altair-rest's `[dev-dependencies]`. Add to `crates/altair-rest/Cargo.toml`:

```toml
[dev-dependencies]
# ... existing deps ...
serde_json = { workspace = true }
```

(serde_json is also a regular dep so this is technically redundant but explicit.)

Actually since `serde_json` is already in `[dependencies]`, examples can use it directly — no dev-dep edit needed.

- [ ] **Step 3: Build**

Run: `cargo build -p altair-rest --example bearer_auth`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-rest/examples/bearer_auth.rs
git commit -m "docs(rest): bearer_auth example"
```

### Task 7.4: json_round_trip.rs

**Files:**
- Create: `crates/altair-rest/examples/json_round_trip.rs`

- [ ] **Step 1: Write the file**

```rust
//! `get_json` / `post_json` helpers — the 80% case of REST consumption.
//!
//! Run with: `cargo run --example json_round_trip -p altair-rest`

use altair_rest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Post {
    id: Option<u64>,
    title: String,
    body: String,
    #[serde(rename = "userId")]
    user_id: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::builder()
        .base_url("https://jsonplaceholder.typicode.com")?
        .build()?;

    // GET → decode into Post
    let post: Post = client.get_json("/posts/1").await?;
    println!("fetched: {post:#?}");

    // POST a new Post → decode the server's response
    let new_post = Post {
        id: None,
        title: "altair-rest demo".into(),
        body: "hello from the example".into(),
        user_id: 1,
    };
    let created: Post = client.post_json("/posts", &new_post).await?;
    println!("created (assigned id {:?}): {created:#?}", created.id);

    Ok(())
}
```

- [ ] **Step 2: Add dev-dep**

`serde` with the derive feature isn't in altair-rest's dev-deps yet. Add to `crates/altair-rest/Cargo.toml` `[dev-dependencies]`:

```toml
serde = { workspace = true }
```

(already added in Task 1.2 — verify and skip if present.)

- [ ] **Step 3: Build**

Run: `cargo build -p altair-rest --example json_round_trip`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-rest/examples/json_round_trip.rs
git commit -m "docs(rest): json_round_trip example"
```

### Task 7.5: with_tracing.rs

**Files:**
- Create: `crates/altair-rest/examples/with_tracing.rs`

- [ ] **Step 1: Add tracing-subscriber to dev-deps**

In `crates/altair-rest/Cargo.toml` `[dev-dependencies]`, add:

```toml
tracing-subscriber = { workspace = true }
```

- [ ] **Step 2: Write the file**

```rust
//! Each request emits a `tracing::span!("HTTP {method}")` with attributes.
//! Install a tracing subscriber to see them. With `altair-otel` initialized,
//! these spans flow to OTLP automatically.
//!
//! Run with:
//!   `RUST_LOG=info cargo run --example with_tracing -p altair-rest`

use altair_rest::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let client = Client::builder()
        .base_url("https://httpbin.org")?
        .build()?;

    // First request — observe the span and event.
    let response = client.get("/get").send().await?;
    tracing::info!(status = ?response.status(), "got first response");

    // Second request to a different path.
    let response = client.get("/uuid").send().await?;
    tracing::info!(status = ?response.status(), "got second response");

    Ok(())
}
```

- [ ] **Step 3: Build**

Run: `cargo build -p altair-rest --example with_tracing`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-rest/Cargo.toml crates/altair-rest/examples/with_tracing.rs
git commit -m "docs(rest): with_tracing example"
```

### Task 7.6: custom_middleware.rs

**Files:**
- Create: `crates/altair-rest/examples/custom_middleware.rs`

- [ ] **Step 1: Write the file**

```rust
//! Append your own middleware to the chain. Custom middleware runs AFTER
//! the built-in retry and tracing middleware.
//!
//! Run with: `cargo run --example custom_middleware -p altair-rest`

use altair_rest::Client;
use altair_rest::reqwest_middleware::{Middleware, Next};
use async_trait::async_trait;
use http::Extensions;
use reqwest::{Request, Response};

struct LoggingMiddleware;

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        println!("[middleware] {} {}", req.method(), req.url());
        let response = next.run(req, extensions).await?;
        println!("[middleware] -> {}", response.status());
        Ok(response)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::builder()
        .base_url("https://httpbin.org")?
        .with_middleware(LoggingMiddleware)
        .build()?;

    let _ = client.get("/get").send().await?;
    let _ = client.get("/uuid").send().await?;
    Ok(())
}
```

- [ ] **Step 2: Add dev-deps for the example**

The example needs `async_trait` (the Middleware trait uses it). In `crates/altair-rest/Cargo.toml` `[dev-dependencies]`, add:

```toml
async-trait = "0.1"
```

Also confirm `http` is reachable from examples — it's in `[dependencies]`, so yes.

`reqwest` is also in `[dependencies]`, accessible directly.

- [ ] **Step 3: Build**

Run: `cargo build -p altair-rest --example custom_middleware`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-rest/Cargo.toml crates/altair-rest/examples/custom_middleware.rs
git commit -m "docs(rest): custom_middleware example"
```

---

## Phase 8: README

### Task 8.1: Write the full README

**Files:**
- Modify: `crates/altair-rest/README.md`

- [ ] **Step 1: Replace the stub with the full README**

````markdown
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

`retry_max_attempts(0)` disables built-in retries. Pair with `altair-retry` if you want a custom retry policy:

```rust,no_run
use altair_rest::Client;
use altair_retry::{retry, Config};

# async fn run() -> anyhow::Result<()> {
let client = Client::builder().retry_max_attempts(0).build()?;

let response = retry(
    Config::builder().name("my.api").max_retries(5).build(),
    || async { Ok::<_, altair_rest::Error>(client.get("https://api.example.com").send().await?) },
).await?;
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
````

- [ ] **Step 2: Verify doc tests pass**

Run: `cargo test -p altair-rest --doc`
Expected: all doc tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-rest/README.md
git commit -m "docs(rest): complete README with examples and error reference"
```

---

## Phase 9: Tracker, root README, CI gate

### Task 9.1: Update porting tracker

**Files:**
- Modify: `docs/porting-tracker.md`

- [ ] **Step 1: Add to the published-set table**

In the "Published crates" table, after the `altair-compress` row, add:

```markdown
| [`altair-rest`](https://crates.io/crates/altair-rest) | 0.1.2 (date TBD on publish) |
```

- [ ] **Step 2: Add release notes bullet**

```markdown
- **`altair-rest` 0.1.2** (date TBD on publish) — Resilient HTTP client built on `reqwest`. Built-in retries via `reqwest-retry` + tracing via `reqwest-tracing`. JSON helpers (`get_json`/`post_json`) for the 80% case.
```

- [ ] **Step 3: Move row in the Starter Set / Done table**

Move the `rest` row from "Awaiting Demand" to the Done table. After the `altair-compress` row in the Done table, add:

```markdown
| `rest` | `altair-rest` | ✅ Done | `reqwest`, `reqwest-middleware`, `reqwest-retry`, `reqwest-tracing` | Resilient HTTP client with retry + tracing baked in |
```

And remove the corresponding "Awaiting Demand" row:

```markdown
| `rest` | `altair-rest` | 💤 Deferred | `reqwest`, `reqwest-middleware`, `reqwest-retry` | Resilient HTTP client |
```

- [ ] **Step 4: Bump "Last updated"**

Replace the existing "Last updated" line with today's date and the latest crate.

- [ ] **Step 5: Commit**

```bash
git add docs/porting-tracker.md
git commit -m "docs: add altair-rest to porting tracker"
```

### Task 9.2: Add to root README

**Files:**
- Modify: `README.md` (workspace root)

- [ ] **Step 1: Add a row to the crate table**

After the `altair-compress` row, add:

```markdown
| [`altair-rest`](crates/altair-rest) | Resilient HTTP client — reqwest + retry + tracing baked in | [![crate](https://img.shields.io/crates/v/altair-rest.svg)](https://crates.io/crates/altair-rest) |
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: list altair-rest in workspace README"
```

### Task 9.3: Full CI gate

- [ ] **Step 1: Run formatter, clippy, tests, doc, deny**

Run:
```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --no-deps --all-features
```

Expected: all four exit 0. If clippy flags issues:
- Most likely candidates: `must_use_candidate` on builder methods (add `#[must_use]`), `unnecessary_wraps` on builder methods that always return Ok (`#[allow(clippy::unnecessary_wraps)]` is fine for those that take a `&str` to keep signature uniform), `doc_markdown` on backtick-needing terms.
- Don't paper over: investigate each one.

- [ ] **Step 2: Verify dry-run publish**

Run: `cargo publish --dry-run -p altair-rest`
Expected: `Uploading altair-rest v0.1.2`; `warning: aborting upload due to dry run`.

- [ ] **Step 3: Commit any clippy/fmt fixes**

```bash
git add -p
git commit -m "fix(rest): satisfy clippy/fmt"
```

(Skip if nothing to commit.)

---

## Phase 10: Push, PR, publish

### Task 10.1: Push branch and open PR

- [ ] **Step 1: Push**

```bash
git push -u origin feat/altair-rest
```

- [ ] **Step 2: Open PR**

```bash
gh pr create --title "feat(rest): add altair-rest crate" --body "$(cat <<'EOF'
## Summary

Adds the seventh crate to the workspace: \`altair-rest\` — resilient HTTP client built on \`reqwest\` with retry and OTel-aware tracing middleware baked in.

- \`Client::builder()...build()\` returns a configured client (newtype around \`reqwest_middleware::ClientWithMiddleware\`).
- Built-in retry via \`reqwest-retry\` (3 attempts, exponential 100ms → 5s, configurable).
- Built-in tracing via \`reqwest-tracing\` — spans flow to OTLP if \`altair-otel\` is initialized.
- \`get_json\` / \`post_json\` for the 80% case.
- Re-exports \`reqwest\` and \`reqwest_middleware\` so consumers don't need them as separate deps.

Spec: docs/specs/2026-05-28-altair-rest-design.md
Plan: docs/plans/2026-05-28-altair-rest-implementation.md

## Test plan

- [x] 15+ unit tests + 6 integration tests pass
- [x] \`cargo clippy --workspace --all-targets --all-features -- -D warnings\` clean
- [x] \`cargo fmt --all --check\` clean
- [x] \`cargo test --workspace --doc\` clean
- [x] \`cargo publish --dry-run -p altair-rest\` clean
- [x] All six examples build (\`cargo build --workspace --examples\`)
- [ ] CI passes on this PR
EOF
)"
```

- [ ] **Step 3: Watch CI and merge**

Use a foreground bash poll so progress is visible:

```bash
until gh pr checks <pr-number> --required 2>/dev/null | grep -qE "fail|pass" && ! gh pr checks <pr-number> 2>/dev/null | grep -q pending; do sleep 15; done
gh pr checks <pr-number>
```

If all checks pass:

```bash
gh pr merge <pr-number> --squash --delete-branch
git checkout main && git pull
```

### Task 10.2: First publish via release-plz

Same pattern as previous crates. release-plz runs on push to main and publishes the new crate. Verify on crates.io:

```bash
curl -s -H 'User-Agent: altair-rs (jasoet87@gmail.com)' \
  https://crates.io/api/v1/crates/altair-rest | jq -r .crate.max_version
```

Expected: matches the workspace version (likely 0.1.2).

A subsequent release-plz "v0.1.3" PR may open for empty workspace churn; close it as before.

### Task 10.3: Final tracker update

**Files:**
- Modify: `docs/porting-tracker.md`

- [ ] **Step 1: Replace "date TBD on publish" with today's date**

In both the published-set table row and the release-notes bullet.

- [ ] **Step 2: Commit and PR**

```bash
git checkout -b docs/rest-published
# (edit the file)
git commit -am "docs: record altair-rest publish date"
git push -u origin docs/rest-published
gh pr create --title "docs: record altair-rest publish date" --body "Trivial tracker update."
gh pr merge <pr-number> --squash --delete-branch
```

---

## Self-Review

### Spec Coverage Check

| Spec section | Implemented in task |
|---|---|
| §1 Overview | Plan header + Task 8.1 README |
| §2 Decisions Locked | Tasks 1.1, 1.2 |
| §3.1 File layout | Tasks 1.2, 2.1, 3.1, 3.2, 4.1, 5.1, 6.1 |
| §3.2 Module responsibilities | Each module is one file with one concern |
| §3.3 Public API | Task 1.2 lib.rs re-exports + per-feature tasks |
| §3.4 Client surface (get/post/put/delete/patch/head/request/execute/inner/get_json/post_json) | Tasks 3.2 + 4.1 |
| §3.5 ClientBuilder surface (all knobs) | Task 3.1 |
| §3.6 Error model | Task 2.1 — every variant has tests |
| §4.1 base_url resolution | Task 3.2 `resolve_url`; tests verify |
| §4.2 Default headers + bearer_token + basic_auth | Task 3.1 builder methods + Task 3.2 `prepare` |
| §4.3 Retry policy (5xx + network + 429, NOT 4xx; configurable; 0 disables) | Task 3.1 + Task 6.1 integration tests |
| §4.4 Tracing middleware default-on | Task 3.1 + Task 7.5 example |
| §4.5 JSON helpers + error_for_status before decoding | Task 4.1 |
| §5 Cross-crate (otel, retry, config) | Documented in Task 8.1 README + Task 7.5 (tracing) + Task 8.1 (altair-retry composition snippet) |
| §6 Testing (unit, wiremock integration, doc, examples) | Task 2.1, 3.1, 3.2 (unit); Task 6.1 (integration); Task 7.* (examples); Task 8.1 (doc tests) |
| §7 Out of scope | Not implemented; no task adds them |
| §8 Risks | Documented in spec; README mentions backoff-impl coexistence (Task 8.1) |
| §9 Versioning (workspace-shared, re-export upgrade discipline) | Inherited via `version.workspace = true` (Task 1.2) |

### Placeholder Scan

- "(date TBD on publish)" — intentional in Task 9.1; resolved in Task 10.3.
- No "TBD", "TODO", or "fill in later" elsewhere.

### Type Consistency

- `Client::from_parts(inner, base_url, bearer_token, basic_auth)` defined in Task 3.2; called from `ClientBuilder::build()` in Task 3.1. Signatures match.
- `Error` enum variants used in tests across `error.rs` (Task 2.1), `config.rs` (Task 3.1), `client.rs` (Task 3.2), `json.rs` (Task 4.1): `Middleware`, `Http`, `Decode`, `Url`, `InvalidHeader`. Consistent.
- `Result<T>` alias used uniformly across all modules.
- `ClientBuilder` returns `Result<Self>` from `base_url()` and `default_header()` (validating input); other knobs return `Self` directly. Consistent with the spec.

No drift identified.

---

## Execution Handoff

**Plan complete and saved to `docs/plans/2026-05-28-altair-rest-implementation.md`. Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** — execute tasks in this session via executing-plans, batch with checkpoints

Pick when ready to start implementation.
