# altair-temporal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build, test, document, and publish `altair-temporal` — a thin facade over the five `temporalio-*` Rust SDK crates that owns typed `Config`, `Client::from_config`, `WorkerBuilder`, `RetryPolicy` builder, `Schedule` builder, `classify_error` helper, and `workflow_id::{encode,decode}` — to crates.io at the current workspace version.

**Architecture:** Single crate under `crates/altair-temporal/`. Eight source files (`lib.rs`, `error.rs`, `config.rs`, `client.rs`, `worker.rs`, `retry.rs`, `schedule.rs`, `activity.rs`, `workflow_id.rs`, `prelude.rs`). Boxed error sources (`Box<dyn Error + Send + Sync>`) keep the public `Error` stable across SDK majors — that is the shielding contract. All five `temporalio-*` crates re-exported at the root. Bootstrap helpers (Client, Worker) and builder helpers (RetryPolicy, Schedule) own the surface; SDK macros and runtime types are passthrough.

**Tech Stack:**
- Rust 2024, MSRV 1.95 (inherit from workspace)
- `temporalio-sdk = "~0.4"`, `temporalio-sdk-core = "~0.4"`, `temporalio-client = "~0.4"`, `temporalio-common = "~0.4"`, `temporalio-macros = "~0.4"` (minor-locked)
- `prost-wkt-types = "0.7"` — for `Duration` construction in retry policy
- `altair-base32 = { path = "../altair-base32", version = "0.1" }` — for workflow_id Crockford encoding
- `tokio` (workspace) — async runtime
- `serde`, `serde_json` (workspace) — for Config + workflow_id payloads
- `tracing = "0.1"` (workspace)
- `thiserror = "2"` (workspace)

Dev-deps:
- `tokio` with `macros` + `rt-multi-thread`
- `anyhow`, `pretty_assertions`, `tempfile`, `toml` (workspace)
- `testcontainers`, `testcontainers-modules` (workspace) — for gated integration test
- `altair-otel = { path = "../altair-otel", version = "0.1" }` — for `with_otel.rs` example
- `altair-config = { path = "../altair-config", version = "0.1" }` — for `with_config.rs` example

---

## File Structure

```
crates/altair-temporal/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs          # crate root: lints, mod decls, re-exports (5 temporalio-* crates)
│   ├── error.rs        # Error enum (boxed sources) + Result alias
│   ├── config.rs       # Config + TlsConfig + Default + serde
│   ├── client.rs       # Client::from_config(&Config) -> Result<temporalio_client::Client>
│   ├── worker.rs       # WorkerBuilder + Worker + run / run_with_shutdown
│   ├── retry.rs        # RetryPolicy newtype + RetryPolicyBuilder
│   ├── schedule.rs     # Schedule + ScheduleBuilder + delete()
│   ├── activity.rs     # classify_error() helper
│   ├── workflow_id.rs  # encode<T: Serialize> / decode<T: DeserializeOwned>
│   └── prelude.rs      # one-import bundle
├── tests/
│   ├── retry.rs        # RetryPolicy builder unit tests
│   ├── schedule.rs     # ScheduleBuilder unit tests
│   ├── workflow_id.rs  # encode/decode round-trip + edge cases
│   └── integration.rs  # gated; testcontainers Temporal server smoke test
└── examples/
    ├── basic_worker.rs
    ├── retry_policy.rs
    ├── schedule_cron.rs
    ├── workflow_id_payload.rs
    ├── with_config.rs
    └── with_otel.rs
```

Workspace + repo edits:
- `Cargo.toml`: add Temporal section to `[workspace.dependencies]`; add `"crates/altair-temporal"` to `members`
- `docs/porting-tracker.md`: move `altair-temporal` from "At-Risk" → "Published crates" + Starter Set
- `README.md`: add `altair-temporal` row

---

## Phase 1: Crate Scaffold

### Task 1.1: Add workspace dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root, `[workspace.dependencies]`)

- [ ] **Step 1: Add the new dependency block**

In root `Cargo.toml`, inside `[workspace.dependencies]`, append a new `# Temporal` section after the existing `# Database` block:

```toml
# Temporal
temporalio-sdk = "~0.4"
temporalio-sdk-core = "~0.4"
temporalio-client = "~0.4"
temporalio-common = "~0.4"
temporalio-macros = "~0.4"
prost-wkt-types = "0.7"
```

- [ ] **Step 2: Verify workspace parses**

Run: `cargo metadata --format-version=1 > /dev/null`
Expected: exit 0, no errors. The Temporal crates do not need to be in `members` (path deps).

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add temporalio-* and prost-wkt-types to workspace dependencies"
```

### Task 1.2: Create crate skeleton

**Files:**
- Create: `crates/altair-temporal/Cargo.toml`
- Create: `crates/altair-temporal/src/lib.rs`
- Create: `crates/altair-temporal/README.md` (stub)
- Modify: `Cargo.toml` (workspace `members`)

- [ ] **Step 1: Create directories**

```bash
mkdir -p crates/altair-temporal/src crates/altair-temporal/tests crates/altair-temporal/examples
```

- [ ] **Step 2: Write `crates/altair-temporal/Cargo.toml`**

```toml
[package]
name = "altair-temporal"
description = "Stable facade over the pre-1.0 temporalio Rust SDK: typed Config, worker/client builders, retry + schedule builders, OTel-aware tracing"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
homepage.workspace = true
readme = "README.md"
keywords = ["temporal", "workflow", "activity", "async"]
categories = ["asynchronous"]

[features]
default = ["tls"]
tls = []
integration-tests = []

[dependencies]
temporalio-sdk = { workspace = true }
temporalio-sdk-core = { workspace = true }
temporalio-client = { workspace = true }
temporalio-common = { workspace = true }
temporalio-macros = { workspace = true }
prost-wkt-types = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }
altair-base32 = { path = "../altair-base32", version = "0.1" }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
anyhow = { workspace = true }
pretty_assertions = { workspace = true }
tempfile = { workspace = true }
toml = { workspace = true }
testcontainers = { workspace = true }
testcontainers-modules = { workspace = true }
# Sibling crates for examples only.
altair-otel = { path = "../altair-otel", version = "0.1" }
altair-config = { path = "../altair-config", version = "0.1" }

[lints]
workspace = true
```

- [ ] **Step 3: Write minimal `crates/altair-temporal/src/lib.rs`**

```rust
//! Stable facade over the pre-1.0 `temporalio-*` Rust SDK.
//!
//! Owns config, client/worker builders, retry-policy and schedule builders,
//! error classification, and workflow-ID-encoded payload helpers. The five
//! `temporalio-*` crates are re-exported at the crate root so consumers
//! depend on `altair-temporal` alone.
//!
//! See the crate README for usage.

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

// Underlying-lib re-exports
pub use ::temporalio_sdk;
pub use ::temporalio_sdk_core;
pub use ::temporalio_client;
pub use ::temporalio_common;
pub use ::temporalio_macros;
```

- [ ] **Step 4: Write stub README**

```markdown
# altair-temporal

Stable facade over the pre-1.0 `temporalio-*` Rust SDK.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

(Full README added in a later task.)
```

- [ ] **Step 5: Register in workspace `members`**

In root `Cargo.toml`, append `"crates/altair-temporal"` to the `members` list. After the edit it should contain ten entries.

- [ ] **Step 6: Verify the empty crate compiles**

Run: `cargo build -p altair-temporal`
Expected: clean build.

- [ ] **Step 7: Commit**

```bash
git add crates/altair-temporal Cargo.toml
git commit -m "feat(temporal): scaffold altair-temporal crate"
```

---

## Phase 2: Error + Config (pure, no SDK calls)

### Task 2.1: Error enum

**Files:**
- Create: `crates/altair-temporal/src/error.rs`
- Modify: `crates/altair-temporal/src/lib.rs`

- [ ] **Step 1: Write `error.rs`**

```rust
//! Error type for altair-temporal.
//!
//! Source fields are boxed (`Box<dyn Error + Send + Sync>`) so this
//! type's public surface stays stable across temporalio-sdk majors —
//! the SDK's concrete error types are exactly what this crate shields.
//! Consumers can downcast through `err.source()` when they need the
//! original.

use thiserror::Error;

/// Boxed error source.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// All errors that may surface from `altair-temporal`.
#[derive(Debug, Error)]
pub enum Error {
    /// Could not establish a gRPC connection to the Temporal server.
    #[error("failed to connect to temporal at {host}")]
    Connect {
        /// The host URL that the connect attempt targeted.
        host: String,
        /// Underlying error (typically a tonic/transport error).
        #[source]
        source: BoxError,
    },

    /// A client-side operation failed (start workflow, get handle, etc.).
    #[error("temporal client error")]
    Client(#[source] BoxError),

    /// A worker-side operation failed (build, poll, shutdown).
    #[error("temporal worker error")]
    Worker(#[source] BoxError),

    /// A schedule operation (create/update/delete) failed.
    #[error("temporal schedule error")]
    Schedule(#[source] BoxError),

    /// The supplied `Config` is invalid.
    #[error("invalid configuration: {0}")]
    Configuration(String),
}

/// Shorthand `Result` parameterised over the crate's `Error`.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Construct a `Connect` error from a host and any boxable source.
    pub fn connect(host: impl Into<String>, source: impl Into<BoxError>) -> Self {
        Self::Connect { host: host.into(), source: source.into() }
    }

    /// Construct a `Client` error from any boxable source.
    pub fn client(source: impl Into<BoxError>) -> Self {
        Self::Client(source.into())
    }

    /// Construct a `Worker` error from any boxable source.
    pub fn worker(source: impl Into<BoxError>) -> Self {
        Self::Worker(source.into())
    }

    /// Construct a `Schedule` error from any boxable source.
    pub fn schedule(source: impl Into<BoxError>) -> Self {
        Self::Schedule(source.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_carries_host_and_source() {
        let err = Error::connect("http://localhost:7233", "boom".to_string());
        assert_eq!(
            err.to_string(),
            "failed to connect to temporal at http://localhost:7233"
        );
        assert!(matches!(err, Error::Connect { .. }));
    }

    #[test]
    fn client_wraps_source() {
        let err = Error::client("io issue".to_string());
        assert_eq!(err.to_string(), "temporal client error");
    }

    #[test]
    fn configuration_carries_message() {
        let err = Error::Configuration("task_queue is required".to_string());
        assert_eq!(
            err.to_string(),
            "invalid configuration: task_queue is required"
        );
    }
}
```

- [ ] **Step 2: Wire into `lib.rs`**

Insert before the re-exports:

```rust
mod error;

pub use error::{BoxError, Error, Result};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p altair-temporal --lib`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-temporal/src/error.rs crates/altair-temporal/src/lib.rs
git commit -m "feat(temporal): add Error enum with boxed source variants"
```

### Task 2.2: Config + TlsConfig

**Files:**
- Create: `crates/altair-temporal/src/config.rs`
- Modify: `crates/altair-temporal/src/lib.rs`

- [ ] **Step 1: Write `config.rs`**

```rust
//! Configuration types for altair-temporal.

use std::path::PathBuf;

/// Connection + worker configuration.
///
/// All fields have defaults except `task_queue`, which is required.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    /// Temporal server URL, e.g. `https://temporal.prod.internal:7233`.
    pub host: String,
    /// Temporal namespace.
    pub namespace: String,
    /// Task queue this worker polls / this client targets.
    pub task_queue: String,
    /// Worker / client identity (visible in the Temporal UI).
    pub identity: String,
    /// Maximum concurrent activities the worker may execute.
    pub max_concurrent_activities: u32,
    /// Maximum concurrent workflow tasks the worker may execute.
    pub max_concurrent_workflows: u32,
    /// Optional TLS configuration. `None` = plaintext (local dev).
    pub tls: Option<TlsConfig>,
}

/// TLS configuration for the gRPC connection to Temporal.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TlsConfig {
    /// Path to the server's CA certificate (PEM).
    pub server_root_ca_cert: PathBuf,
    /// Optional client certificate (PEM) for mutual TLS.
    pub client_cert: Option<PathBuf>,
    /// Optional client key (PEM) for mutual TLS.
    pub client_key: Option<PathBuf>,
    /// Optional gRPC SNI / server-name override.
    pub server_name_override: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "http://localhost:7233".to_string(),
            namespace: "default".to_string(),
            task_queue: String::new(),
            identity: "altair-temporal".to_string(),
            max_concurrent_activities: 100,
            max_concurrent_workflows: 100,
            tls: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let c = Config::default();
        assert_eq!(c.host, "http://localhost:7233");
        assert_eq!(c.namespace, "default");
        assert_eq!(c.task_queue, "");
        assert_eq!(c.identity, "altair-temporal");
        assert_eq!(c.max_concurrent_activities, 100);
        assert_eq!(c.max_concurrent_workflows, 100);
        assert!(c.tls.is_none());
    }

    #[test]
    fn deserialise_minimal_toml() {
        let toml_src = r#"
task_queue = "demo"
"#;
        let c: Config = toml::from_str(toml_src).unwrap();
        assert_eq!(c.task_queue, "demo");
        assert_eq!(c.host, "http://localhost:7233"); // default kicks in
    }

    #[test]
    fn deserialise_full_toml() {
        let toml_src = r#"
host = "https://temporal.prod.example:7233"
namespace = "archive"
task_queue = "archive-tq"
identity = "archive-rs-worker"
max_concurrent_activities = 50
max_concurrent_workflows = 50

[tls]
server_root_ca_cert = "/etc/temporal/ca.pem"
client_cert = "/etc/temporal/client.crt"
client_key = "/etc/temporal/client.key"
server_name_override = "temporal.internal"
"#;
        let c: Config = toml::from_str(toml_src).unwrap();
        assert_eq!(c.namespace, "archive");
        assert_eq!(c.max_concurrent_activities, 50);
        let tls = c.tls.expect("tls");
        assert_eq!(tls.server_root_ca_cert.to_str().unwrap(), "/etc/temporal/ca.pem");
        assert_eq!(tls.server_name_override.as_deref(), Some("temporal.internal"));
    }
}
```

- [ ] **Step 2: Wire into `lib.rs`**

Insert below `mod error;`:

```rust
mod config;

pub use config::{Config, TlsConfig};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p altair-temporal --lib config::`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-temporal/src/config.rs crates/altair-temporal/src/lib.rs
git commit -m "feat(temporal): add Config + TlsConfig with serde defaults"
```

---

## Phase 3: RetryPolicy

### Task 3.1: RetryPolicy + RetryPolicyBuilder

**Files:**
- Create: `crates/altair-temporal/src/retry.rs`
- Modify: `crates/altair-temporal/src/lib.rs`

- [ ] **Step 1: Write `retry.rs`**

```rust
//! Retry-policy builder over the SDK's proto `RetryPolicy`.

use std::time::Duration;

/// A Temporal `RetryPolicy` ready to plug into `ActivityOptions`.
///
/// Constructed via [`RetryPolicy::builder`]; converted to the SDK type
/// via [`RetryPolicy::into_inner`].
#[derive(Debug, Clone)]
pub struct RetryPolicy(
    temporalio_common::protos::temporal::api::common::v1::RetryPolicy,
);

impl RetryPolicy {
    /// Start building a retry policy.
    #[must_use]
    pub fn builder() -> RetryPolicyBuilder {
        RetryPolicyBuilder::default()
    }

    /// Yield the underlying proto type for SDK calls.
    #[must_use]
    pub fn into_inner(
        self,
    ) -> temporalio_common::protos::temporal::api::common::v1::RetryPolicy {
        self.0
    }
}

/// Builder for [`RetryPolicy`].
#[derive(Debug, Clone)]
pub struct RetryPolicyBuilder {
    initial_interval: Duration,
    maximum_interval: Duration,
    backoff_coefficient: f64,
    max_attempts: u32,
    non_retryable_error_types: Vec<String>,
}

impl Default for RetryPolicyBuilder {
    fn default() -> Self {
        Self {
            initial_interval: Duration::from_secs(1),
            maximum_interval: Duration::from_secs(30),
            backoff_coefficient: 2.0,
            max_attempts: 0,
            non_retryable_error_types: Vec::new(),
        }
    }
}

impl RetryPolicyBuilder {
    /// Initial backoff interval (default `1s`).
    #[must_use]
    pub fn initial_interval(mut self, d: Duration) -> Self {
        self.initial_interval = d;
        self
    }

    /// Maximum backoff interval (default `30s`).
    #[must_use]
    pub fn maximum_interval(mut self, d: Duration) -> Self {
        self.maximum_interval = d;
        self
    }

    /// Exponential backoff multiplier (default `2.0`).
    #[must_use]
    pub fn backoff_coefficient(mut self, c: f64) -> Self {
        self.backoff_coefficient = c;
        self
    }

    /// Maximum number of attempts. `0` = unlimited (Temporal convention).
    #[must_use]
    pub fn max_attempts(mut self, n: u32) -> Self {
        self.max_attempts = n;
        self
    }

    /// Append an error type name that should never be retried.
    ///
    /// Matched against the `type` field of `ApplicationFailure` at runtime.
    /// Call repeatedly to add more.
    #[must_use]
    pub fn non_retryable(mut self, error_type: impl Into<String>) -> Self {
        self.non_retryable_error_types.push(error_type.into());
        self
    }

    /// Finalise into a [`RetryPolicy`].
    #[must_use]
    pub fn build(self) -> RetryPolicy {
        use temporalio_common::protos::temporal::api::common::v1::RetryPolicy as Proto;
        RetryPolicy(Proto {
            initial_interval: Some(duration_to_proto(self.initial_interval)),
            backoff_coefficient: self.backoff_coefficient,
            maximum_interval: Some(duration_to_proto(self.maximum_interval)),
            maximum_attempts: i32::try_from(self.max_attempts).unwrap_or(i32::MAX),
            non_retryable_error_types: self.non_retryable_error_types,
        })
    }
}

fn duration_to_proto(d: Duration) -> prost_wkt_types::Duration {
    prost_wkt_types::Duration {
        seconds: i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
        nanos: i32::try_from(d.subsec_nanos()).unwrap_or(i32::MAX),
    }
}
```

- [ ] **Step 2: Wire into `lib.rs`**

Insert below `mod config;`:

```rust
mod retry;

pub use retry::{RetryPolicy, RetryPolicyBuilder};
```

- [ ] **Step 3: Verify build**

Run: `cargo build -p altair-temporal`
Expected: clean.

- [ ] **Step 4: API verification**

The proto field names (`initial_interval`, `backoff_coefficient`, `maximum_interval`, `maximum_attempts`, `non_retryable_error_types`) come from `temporalio-common 0.4`. If the build fails, check the actual field names with:

```bash
cargo doc -p temporalio-common --no-deps --open
```

Navigate to `protos::temporal::api::common::v1::RetryPolicy`. Adjust field names in `RetryPolicyBuilder::build()` to match. Public signature does NOT change.

- [ ] **Step 5: Commit (without tests yet — tests come in Phase 5 integration file)**

```bash
git add crates/altair-temporal/src/retry.rs crates/altair-temporal/src/lib.rs
git commit -m "feat(temporal): add RetryPolicy newtype + RetryPolicyBuilder"
```

---

## Phase 4: workflow_id

### Task 4.1: workflow_id::encode + decode

**Files:**
- Create: `crates/altair-temporal/src/workflow_id.rs`
- Modify: `crates/altair-temporal/src/lib.rs`

- [ ] **Step 1: Write `workflow_id.rs`**

```rust
//! Encode a small structured payload into a workflow ID.
//!
//! Temporal's `ScheduleAction::StartWorkflow` cannot attach workflow input,
//! so projects encode small payloads into the workflow ID itself. This
//! module standardises the encoding using Crockford Base32 (via
//! [`altair_base32`]) over the JSON bytes of the payload.
//!
//! Format: `{prefix}-{base32}`.
//!
//! # Limits
//!
//! Temporal workflow IDs cap at 200 bytes. Use this for small payloads
//! only (IDs, short strings, a handful of fields). Larger payloads belong
//! in activity input.

use crate::error::{Error, Result};

/// Temporal's workflow ID length limit, in bytes.
pub const MAX_WORKFLOW_ID_LEN: usize = 200;

/// Encode `payload` into a workflow ID of the form `{prefix}-{base32}`.
///
/// # Errors
/// * `Error::Configuration` if serialisation fails or the resulting ID
///   exceeds [`MAX_WORKFLOW_ID_LEN`].
pub fn encode<T: serde::Serialize>(prefix: &str, payload: &T) -> Result<String> {
    let bytes = serde_json::to_vec(payload)
        .map_err(|e| Error::Configuration(format!("payload serialise failed: {e}")))?;
    let encoded = altair_base32::encode(&bytes);
    let id = format!("{prefix}-{encoded}");
    if id.len() > MAX_WORKFLOW_ID_LEN {
        return Err(Error::Configuration(format!(
            "workflow id is {} bytes, max {MAX_WORKFLOW_ID_LEN}",
            id.len()
        )));
    }
    Ok(id)
}

/// Decode a workflow ID produced by [`encode`].
///
/// Returns `(prefix, payload)`. The prefix may itself contain `-` —
/// only the last `-` separates prefix from encoded payload.
///
/// # Errors
/// * `Error::Configuration` if the ID has no `-`, the encoded segment
///   is not valid Crockford Base32, or the bytes do not deserialise as `T`.
pub fn decode<T: serde::de::DeserializeOwned>(id: &str) -> Result<(String, T)> {
    let (prefix, encoded) = id.rsplit_once('-').ok_or_else(|| {
        Error::Configuration(format!("workflow id missing '-' separator: {id}"))
    })?;
    let bytes = altair_base32::decode(encoded)
        .map_err(|e| Error::Configuration(format!("workflow id base32 decode failed: {e}")))?;
    let payload: T = serde_json::from_slice(&bytes)
        .map_err(|e| Error::Configuration(format!("workflow id payload deserialise failed: {e}")))?;
    Ok((prefix.to_string(), payload))
}
```

- [ ] **Step 2: Wire into `lib.rs`**

Insert below `mod retry;`:

```rust
pub mod workflow_id;
```

- [ ] **Step 3: Write the test file**

Create `crates/altair-temporal/tests/workflow_id.rs`:

```rust
//! Encode/decode round-trip + edge cases for `workflow_id`.

use altair_temporal::workflow_id::{decode, encode, MAX_WORKFLOW_ID_LEN};
use altair_temporal::Error;

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Payload {
    archive_name: String,
    target_year: u32,
}

#[test]
fn round_trip_simple_prefix() {
    let payload = Payload {
        archive_name: "customer".to_string(),
        target_year: 2026,
    };
    let id = encode("archive", &payload).unwrap();
    let (prefix, out): (String, Payload) = decode(&id).unwrap();
    assert_eq!(prefix, "archive");
    assert_eq!(out, payload);
}

#[test]
fn round_trip_prefix_with_hyphens() {
    let payload = Payload {
        archive_name: "x".to_string(),
        target_year: 1,
    };
    let id = encode("daily-archive-prod", &payload).unwrap();
    let (prefix, out): (String, Payload) = decode(&id).unwrap();
    assert_eq!(prefix, "daily-archive-prod");
    assert_eq!(out, payload);
}

#[test]
fn decode_rejects_missing_separator() {
    let err = decode::<Payload>("noSeparatorHere").unwrap_err();
    assert!(matches!(err, Error::Configuration(_)));
}

#[test]
fn decode_rejects_invalid_base32() {
    let err = decode::<Payload>("archive-not!base32").unwrap_err();
    assert!(matches!(err, Error::Configuration(_)));
}

#[test]
fn decode_rejects_invalid_json() {
    // Real base32 encoding of bytes that aren't valid JSON.
    let bytes = b"not json bytes";
    let encoded = altair_base32::encode(bytes);
    let id = format!("archive-{encoded}");
    let err = decode::<Payload>(&id).unwrap_err();
    assert!(matches!(err, Error::Configuration(_)));
}

#[test]
fn encode_rejects_overlong_payload() {
    let big = Payload {
        archive_name: "x".repeat(500),
        target_year: 0,
    };
    let err = encode("p", &big).unwrap_err();
    assert!(matches!(err, Error::Configuration(_)));
}

#[test]
fn boundary_max_id_len_passes() {
    // Construct a payload just under the limit.
    let body_len = MAX_WORKFLOW_ID_LEN / 2; // base32 ratio ~1.6x; this is well under
    let payload = Payload {
        archive_name: "x".repeat(body_len.saturating_sub(20)),
        target_year: 1,
    };
    let id = encode("p", &payload).unwrap();
    assert!(id.len() <= MAX_WORKFLOW_ID_LEN);
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p altair-temporal --test workflow_id`
Expected: 7 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/altair-temporal/src/workflow_id.rs crates/altair-temporal/src/lib.rs crates/altair-temporal/tests/workflow_id.rs
git commit -m "feat(temporal): add workflow_id encode/decode with Crockford Base32"
```

---

## Phase 5: classify_error helper

### Task 5.1: activity::classify_error

**Files:**
- Create: `crates/altair-temporal/src/activity.rs`
- Modify: `crates/altair-temporal/src/lib.rs`

- [ ] **Step 1: Write `activity.rs`**

```rust
//! Helpers for working with Temporal `ActivityError`.

use temporalio_sdk::activities::ActivityError;

/// Convert an error into an [`ActivityError::application`] failure,
/// marking it `non_retryable` when `is_permanent(&err)` returns `true`.
///
/// The error's `Display` is used as the failure message; the type name
/// (from `std::any::type_name::<E>()`) is used as the failure `type` so
/// it can be matched in `RetryPolicy::non_retryable(...)` lists.
pub fn classify_error<E, F>(err: E, is_permanent: F) -> ActivityError
where
    E: std::error::Error + Send + Sync + 'static,
    F: FnOnce(&E) -> bool,
{
    let permanent = is_permanent(&err);
    let message = err.to_string();
    let type_name = std::any::type_name::<E>().to_string();
    let mut failure = temporalio_sdk::activities::ApplicationFailure::new(
        message,
        Some(type_name),
        vec![],
    );
    if permanent {
        failure = failure.non_retryable();
    }
    ActivityError::application(failure)
}
```

- [ ] **Step 2: Wire into `lib.rs`**

Insert below `pub mod workflow_id;`:

```rust
mod activity;

pub use activity::classify_error;
```

- [ ] **Step 3: API verification**

The names `ActivityError::application`, `ApplicationFailure::new`, and `ApplicationFailure::non_retryable()` come from `temporalio-sdk 0.4`. If the build fails:

```bash
cargo doc -p temporalio-sdk --no-deps --open
```

Find the actual constructors. The public signature of `classify_error` does NOT change. Adjust the body to match the SDK.

- [ ] **Step 4: Verify build**

Run: `cargo build -p altair-temporal`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/altair-temporal/src/activity.rs crates/altair-temporal/src/lib.rs
git commit -m "feat(temporal): add classify_error helper for permanent-vs-transient ActivityError"
```

---

## Phase 6: Schedule

### Task 6.1: Schedule + ScheduleBuilder (no SDK calls yet)

**Files:**
- Create: `crates/altair-temporal/src/schedule.rs`
- Modify: `crates/altair-temporal/src/lib.rs`

- [ ] **Step 1: Write `schedule.rs`**

```rust
//! Schedule builder + helpers.
//!
//! The builder accumulates state; terminal methods (`create`, `update`,
//! `delete`) talk to the SDK client.

use std::time::Duration;

use crate::error::{Error, Result};

/// A schedule ready to be created or updated.
#[derive(Debug, Clone)]
pub struct Schedule {
    pub(crate) cron_strings: Vec<String>,
    pub(crate) interval: Option<Duration>,
    pub(crate) note: Option<String>,
    pub(crate) paused: bool,
    pub(crate) workflow_type: Option<String>,
    pub(crate) task_queue: Option<String>,
    pub(crate) workflow_id: Option<String>,
}

impl Schedule {
    /// Start building a schedule.
    #[must_use]
    pub fn builder() -> ScheduleBuilder {
        ScheduleBuilder { schedule: Schedule {
            cron_strings: Vec::new(),
            interval: None,
            note: None,
            paused: false,
            workflow_type: None,
            task_queue: None,
            workflow_id: None,
        }}
    }
}

/// Builder for [`Schedule`].
#[derive(Debug, Clone)]
pub struct ScheduleBuilder {
    schedule: Schedule,
}

impl ScheduleBuilder {
    /// Add a cron expression. Repeatable (Temporal accepts a list).
    #[must_use]
    pub fn cron(mut self, cron: impl Into<String>) -> Self {
        self.schedule.cron_strings.push(cron.into());
        self
    }

    /// Set the interval between runs. Overwrites any prior interval.
    #[must_use]
    pub fn interval(mut self, d: Duration) -> Self {
        self.schedule.interval = Some(d);
        self
    }

    /// Set a human-readable note (shown in the Temporal UI).
    #[must_use]
    pub fn note(mut self, n: impl Into<String>) -> Self {
        self.schedule.note = Some(n.into());
        self
    }

    /// Whether the schedule starts paused (default `false`).
    #[must_use]
    pub fn paused(mut self, p: bool) -> Self {
        self.schedule.paused = p;
        self
    }

    /// Configure the `StartWorkflow` action.
    #[must_use]
    pub fn start_workflow(
        mut self,
        workflow_type: impl Into<String>,
        task_queue: impl Into<String>,
        workflow_id: impl Into<String>,
    ) -> Self {
        self.schedule.workflow_type = Some(workflow_type.into());
        self.schedule.task_queue = Some(task_queue.into());
        self.schedule.workflow_id = Some(workflow_id.into());
        self
    }

    /// Finalise into a [`Schedule`] without making any RPC.
    #[must_use]
    pub fn build(self) -> Schedule {
        self.schedule
    }

    /// Create the schedule on the server. See module docs.
    pub async fn create(
        self,
        client: &temporalio_client::Client,
        id: impl Into<String>,
    ) -> Result<()> {
        let id = id.into();
        let schedule = self.build();
        validate_schedule(&schedule)?;
        // Implementation deferred to Task 6.2 — verify exact SDK call shape.
        let _ = (client, &id, &schedule);
        Err(Error::Configuration("schedule create not wired yet".to_string()))
    }

    /// Update an existing schedule on the server.
    pub async fn update(
        self,
        client: &temporalio_client::Client,
        id: impl Into<String>,
    ) -> Result<()> {
        let id = id.into();
        let schedule = self.build();
        validate_schedule(&schedule)?;
        let _ = (client, &id, &schedule);
        Err(Error::Configuration("schedule update not wired yet".to_string()))
    }
}

/// Delete a schedule by id.
pub async fn delete(client: &temporalio_client::Client, id: &str) -> Result<()> {
    let _ = (client, id);
    Err(Error::Configuration("schedule delete not wired yet".to_string()))
}

fn validate_schedule(s: &Schedule) -> Result<()> {
    if s.workflow_type.is_none() || s.task_queue.is_none() || s.workflow_id.is_none() {
        return Err(Error::Configuration(
            "schedule requires start_workflow(workflow_type, task_queue, workflow_id)".to_string(),
        ));
    }
    if s.cron_strings.is_empty() && s.interval.is_none() {
        return Err(Error::Configuration(
            "schedule requires at least one cron or interval".to_string(),
        ));
    }
    Ok(())
}
```

- [ ] **Step 2: Wire into `lib.rs`**

Insert below `mod activity;`:

```rust
mod schedule;

pub use schedule::{delete as delete_schedule, Schedule, ScheduleBuilder};
```

- [ ] **Step 3: Write builder unit tests**

Create `crates/altair-temporal/tests/schedule.rs`:

```rust
//! ScheduleBuilder unit tests (no SDK calls).

use std::time::Duration;

use altair_temporal::Schedule;

#[test]
fn cron_repeatable() {
    let s = Schedule::builder()
        .cron("0 9 * * *")
        .cron("0 18 * * *")
        .start_workflow("MyWorkflow", "tq", "wid")
        .build();
    assert_eq!(s.cron_strings.len(), 2);
}

#[test]
fn interval_overwrites() {
    let s = Schedule::builder()
        .interval(Duration::from_secs(60))
        .interval(Duration::from_secs(120))
        .start_workflow("W", "tq", "wid")
        .build();
    assert_eq!(s.interval, Some(Duration::from_secs(120)));
}

#[test]
fn note_and_paused() {
    let s = Schedule::builder()
        .note("daily archive")
        .paused(true)
        .cron("0 0 * * *")
        .start_workflow("W", "tq", "wid")
        .build();
    assert_eq!(s.note.as_deref(), Some("daily archive"));
    assert!(s.paused);
}

#[test]
fn cron_and_interval_coexist() {
    let s = Schedule::builder()
        .cron("0 9 * * MON")
        .interval(Duration::from_secs(300))
        .start_workflow("W", "tq", "wid")
        .build();
    assert_eq!(s.cron_strings, vec!["0 9 * * MON"]);
    assert_eq!(s.interval, Some(Duration::from_secs(300)));
}
```

Note: this requires the `Schedule` fields to be `pub` or `pub(crate)`. They are `pub(crate)` in the implementation. To test from `tests/` (which is an external crate), expose the fields through a small helper module — OR, simplest: temporarily make the fields `pub` (same as Archive-rs does for similar internal state). For v0.1 keeping `pub(crate)` and inspecting via terminal methods is cleaner, but inspecting requires SDK calls. Therefore: move the unit tests into a `#[cfg(test)] mod tests` block at the bottom of `src/schedule.rs` where `pub(crate)` is visible. Drop `tests/schedule.rs`; this file is unit tests.

Adjust: instead of `tests/schedule.rs`, append a `#[cfg(test)] mod tests` block at the bottom of `src/schedule.rs` containing the same test bodies.

- [ ] **Step 4: Run tests**

Run: `cargo test -p altair-temporal --lib schedule::`
Expected: 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/altair-temporal/src/schedule.rs crates/altair-temporal/src/lib.rs
git commit -m "feat(temporal): add Schedule + ScheduleBuilder (no SDK wiring yet)"
```

### Task 6.2: Wire Schedule create / update / delete to SDK

**Files:**
- Modify: `crates/altair-temporal/src/schedule.rs`

- [ ] **Step 1: Replace placeholder bodies with SDK calls**

The exact API for creating/updating/deleting schedules in `temporalio-client 0.4` will be one of:

- `client.create_schedule(id, CreateScheduleOptions::builder()...).await`
- `client.get_schedule_handle(id).update(|u| ...).await`
- `client.get_schedule_handle(id).delete().await`

Or with slight variations. Verify with `cargo doc -p temporalio-client --no-deps --open` and navigate to the `Client` impl.

Replace `ScheduleBuilder::create` body with:

```rust
use temporalio_client::{CreateScheduleOptions, ScheduleAction, ScheduleSpec};
use temporalio_common::protos::temporal::api::schedule::v1 as proto;

let schedule = self.build();
validate_schedule(&schedule)?;

let spec = ScheduleSpec {
    cron_strings: schedule.cron_strings.clone(),
    // intervals translated from Duration to the proto's Interval type
    intervals: schedule.interval.map(duration_to_interval).into_iter().collect(),
    ..Default::default()
};

let action = ScheduleAction::StartWorkflow {
    workflow_type: schedule.workflow_type.unwrap(),
    task_queue: schedule.task_queue.unwrap(),
    workflow_id: schedule.workflow_id.unwrap(),
    // input is intentionally absent — see workflow_id module docs
    ..Default::default()
};

let mut opts = CreateScheduleOptions::builder()
    .action(action)
    .spec(spec);
if let Some(note) = &schedule.note { opts = opts.note(note.clone()); }
if schedule.paused { opts = opts.paused(true); }
let opts = opts.build()
    .map_err(|e| Error::schedule(Box::new(e) as _))?;

client.create_schedule(id, opts).await
    .map(|_| ())
    .map_err(|e| Error::schedule(Box::new(e) as _))
```

Replace `update` body with the analogous `client.get_schedule_handle(id).update(|u| { u.set_spec(spec); u.set_paused(...); u.set_note(...); }).await` shape.

Replace `delete` body with `client.get_schedule_handle(id).delete().await.map_err(|e| Error::schedule(...))`.

Add the helper:

```rust
fn duration_to_interval(d: Duration) -> proto::IntervalSpec {
    proto::IntervalSpec {
        interval: Some(prost_wkt_types::Duration {
            seconds: i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
            nanos: i32::try_from(d.subsec_nanos()).unwrap_or(i32::MAX),
        }),
        phase: None,
    }
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p altair-temporal`
Expected: clean. If types or method names differ, the failure messages tell you what to adjust. Public signatures stay fixed.

- [ ] **Step 3: API verification fallback**

If the actual SDK exposes a substantially different schedule API (e.g. no `CreateScheduleOptions::builder()`, instead a raw `CreateScheduleRequest`), use the raw proto type from `temporalio_common::protos::temporal::api::workflowservice::v1`. Keep the public `ScheduleBuilder` surface unchanged.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-temporal/src/schedule.rs
git commit -m "feat(temporal): wire Schedule create/update/delete to SDK client"
```

---

## Phase 7: Client + Worker (SDK-heavy)

### Task 7.1: Client::from_config

**Files:**
- Create: `crates/altair-temporal/src/client.rs`
- Modify: `crates/altair-temporal/src/lib.rs`

- [ ] **Step 1: Write `client.rs`**

```rust
//! Client factory.

use temporalio_client::{Client as SdkClient, ClientOptions, ConnectionOptions};
use url::Url;

use crate::config::Config;
use crate::error::{Error, Result};

/// Namespace for client construction. Returns the SDK client.
pub struct Client;

impl Client {
    /// Connect a SDK client using the given configuration.
    pub async fn from_config(cfg: &Config) -> Result<SdkClient> {
        let url = Url::parse(&cfg.host)
            .map_err(|e| Error::Configuration(format!("invalid host: {e}")))?;

        let mut conn = ConnectionOptions::new(url);
        conn = conn.identity(&cfg.identity);
        // TLS handled here when feature enabled — Phase 7 followup will
        // add it after the no-TLS path compiles.

        let opts = ClientOptions::new(&cfg.namespace);

        SdkClient::connect(conn, opts)
            .await
            .map_err(|e| Error::connect(cfg.host.clone(), Box::new(e) as _))
    }
}
```

> **Note:** `Client::connect(conn, opts)` is the expected shape; if the SDK uses `ClientOptions::new(...).connect(conn)` or similar, adjust the call. Add `url = "2"` to crate `[dependencies]` and update workspace deps if not already there (url is already in workspace).

- [ ] **Step 2: Wire into `lib.rs`**

Insert below `mod schedule;`:

```rust
mod client;

pub use client::Client;
```

- [ ] **Step 3: Add `url` dep**

Append to `crates/altair-temporal/Cargo.toml` `[dependencies]`:

```toml
url = { workspace = true }
```

- [ ] **Step 4: Build**

Run: `cargo build -p altair-temporal`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/altair-temporal/src/client.rs crates/altair-temporal/src/lib.rs crates/altair-temporal/Cargo.toml
git commit -m "feat(temporal): add Client::from_config factory (plaintext only)"
```

### Task 7.2: TLS wiring in Client::from_config

**Files:**
- Modify: `crates/altair-temporal/src/client.rs`

- [ ] **Step 1: Add TLS handling**

Replace the TLS-deferred comment in `Client::from_config` with real wiring. The exact `temporalio-client 0.4` TLS API will be one of:

- `ConnectionOptions::with_tls(TlsConfig { server_root_ca_cert: Vec<u8>, ... })`
- `ClientOptions::tls(...)` separate method
- A `ConnectionOptionsBuilder::tls(...)` builder fluent step

Verify with `cargo doc -p temporalio-client --no-deps --open` and search for "tls".

Implementation sketch (adjust to actual API):

```rust
if let Some(tls_cfg) = &cfg.tls {
    let ca = std::fs::read(&tls_cfg.server_root_ca_cert)
        .map_err(|e| Error::Configuration(format!("read ca cert: {e}")))?;
    let mut sdk_tls = temporalio_client::TlsConfig {
        server_root_ca_cert: Some(ca),
        domain: tls_cfg.server_name_override.clone(),
        ..Default::default()
    };
    if let (Some(cert_path), Some(key_path)) = (&tls_cfg.client_cert, &tls_cfg.client_key) {
        let cert = std::fs::read(cert_path)
            .map_err(|e| Error::Configuration(format!("read client cert: {e}")))?;
        let key = std::fs::read(key_path)
            .map_err(|e| Error::Configuration(format!("read client key: {e}")))?;
        sdk_tls.client_tls_config = Some(temporalio_client::ClientTlsConfig {
            client_cert: cert,
            client_private_key: key,
        });
    } else if tls_cfg.client_cert.is_some() || tls_cfg.client_key.is_some() {
        return Err(Error::Configuration(
            "client_cert and client_key must both be set or both unset".to_string(),
        ));
    }
    conn = conn.tls_config(sdk_tls);
}
```

If `temporalio_client::TlsConfig` exposes different field names, adjust them. The validation rule "both client_cert + client_key, or neither" stays the same.

- [ ] **Step 2: Build**

Run: `cargo build -p altair-temporal`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-temporal/src/client.rs
git commit -m "feat(temporal): wire TLS configuration in Client::from_config"
```

### Task 7.3: WorkerBuilder + Worker

**Files:**
- Create: `crates/altair-temporal/src/worker.rs`
- Modify: `crates/altair-temporal/src/lib.rs`

- [ ] **Step 1: Write `worker.rs`**

```rust
//! Worker builder + lifecycle.

use std::sync::Arc;

use temporalio_sdk_core::{CoreRuntime, RuntimeOptions};

use crate::client::Client;
use crate::config::Config;
use crate::error::{Error, Result};

/// Builder for [`Worker`].
pub struct WorkerBuilder {
    host: String,
    namespace: String,
    task_queue: String,
    identity: String,
    max_concurrent_activities: u32,
    max_concurrent_workflows: u32,
    tls: Option<crate::config::TlsConfig>,
    registrations: Vec<Box<dyn FnOnce(&mut temporalio_sdk::worker::Worker) + Send>>,
}

impl WorkerBuilder {
    /// Start configuring a worker from a [`Config`].
    #[must_use]
    pub fn new(cfg: &Config) -> Self {
        Self {
            host: cfg.host.clone(),
            namespace: cfg.namespace.clone(),
            task_queue: cfg.task_queue.clone(),
            identity: cfg.identity.clone(),
            max_concurrent_activities: cfg.max_concurrent_activities,
            max_concurrent_workflows: cfg.max_concurrent_workflows,
            tls: cfg.tls.clone(),
            registrations: Vec::new(),
        }
    }

    /// Override identity (default from `Config`).
    #[must_use]
    pub fn identity(mut self, id: impl Into<String>) -> Self {
        self.identity = id.into();
        self
    }

    /// Override max concurrent activities.
    #[must_use]
    pub fn max_concurrent_activities(mut self, n: u32) -> Self {
        self.max_concurrent_activities = n;
        self
    }

    /// Override max concurrent workflows.
    #[must_use]
    pub fn max_concurrent_workflows(mut self, n: u32) -> Self {
        self.max_concurrent_workflows = n;
        self
    }

    /// Register a workflow type for this worker.
    #[must_use]
    pub fn register_workflow<W>(mut self) -> Self
    where
        W: temporalio_sdk::workflow::Workflow + 'static,
    {
        self.registrations.push(Box::new(|w| {
            w.register_workflow::<W>();
        }));
        self
    }

    /// Register an activity implementation instance for this worker.
    #[must_use]
    pub fn register_activities<A>(mut self, instance: Arc<A>) -> Self
    where
        A: temporalio_sdk::activities::ActivityRegistration + Send + Sync + 'static,
    {
        self.registrations.push(Box::new(move |w| {
            w.register_activities(instance);
        }));
        self
    }

    /// Build the underlying worker. Connects the SDK client and registers
    /// every queued workflow/activity.
    pub async fn build(self) -> Result<Worker> {
        let cfg_for_client = Config {
            host: self.host.clone(),
            namespace: self.namespace.clone(),
            task_queue: self.task_queue.clone(),
            identity: self.identity.clone(),
            max_concurrent_activities: self.max_concurrent_activities,
            max_concurrent_workflows: self.max_concurrent_workflows,
            tls: self.tls.clone(),
        };
        let client = Client::from_config(&cfg_for_client).await?;

        let runtime = CoreRuntime::new_assume_tokio(RuntimeOptions::default())
            .map_err(|e| Error::worker(Box::new(e) as _))?;

        let worker_opts = temporalio_sdk::WorkerOptionsBuilder::default()
            .task_queue(self.task_queue)
            .identity(self.identity)
            .max_outstanding_activities(self.max_concurrent_activities as usize)
            .max_outstanding_workflow_tasks(self.max_concurrent_workflows as usize)
            .build()
            .map_err(|e| Error::worker(Box::new(e) as _))?;

        let core_worker = temporalio_sdk_core::init_worker(&runtime, worker_opts, client.clone())
            .map_err(|e| Error::worker(Box::new(e) as _))?;

        let mut sdk_worker = temporalio_sdk::Worker::new(Arc::new(core_worker), self.namespace);

        for reg in self.registrations {
            reg(&mut sdk_worker);
        }

        Ok(Worker { inner: sdk_worker })
    }
}

/// A built Temporal worker ready to poll.
pub struct Worker {
    inner: temporalio_sdk::Worker,
}

impl Worker {
    /// Run until SIGINT (Unix + Windows) / SIGTERM (Unix only).
    pub async fn run(self) -> Result<()> {
        self.run_with_shutdown(shutdown_signal()).await
    }

    /// Run until the given future resolves.
    pub async fn run_with_shutdown<F>(mut self, shutdown: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let runner = self.inner.run();
        tokio::select! {
            res = runner => res.map_err(|e| Error::worker(Box::new(e) as _)),
            () = shutdown => {
                self.inner.initiate_shutdown();
                Ok(())
            }
        }
    }
}

async fn shutdown_signal() {
    use tokio::signal::ctrl_c;
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c() => {},
            _ = term.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = ctrl_c().await;
    }
}
```

- [ ] **Step 2: Wire into `lib.rs`**

Insert below `mod client;`:

```rust
mod worker;

pub use worker::{Worker, WorkerBuilder};
```

- [ ] **Step 3: API verification**

`WorkerOptionsBuilder`, `init_worker`, `Worker::run`, `Worker::initiate_shutdown` come from `temporalio-sdk 0.4` / `temporalio-sdk-core 0.4`. If any name differs:

```bash
cargo doc -p temporalio-sdk --no-deps --open
cargo doc -p temporalio-sdk-core --no-deps --open
```

Find the actual names. Public signatures of `WorkerBuilder` and `Worker` do NOT change.

If `register_workflow::<W>` / `register_activities(instance)` are macro-generated methods rather than trait methods, the trait-bound generics may need to be removed. In that case, expose `register_workflow_fn(|w| w.register_workflow::<MyWorkflow>())` as an escape hatch and document it. Keep the simple `register_workflow::<W>()` if it compiles.

- [ ] **Step 4: Build**

Run: `cargo build -p altair-temporal`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/altair-temporal/src/worker.rs crates/altair-temporal/src/lib.rs
git commit -m "feat(temporal): add WorkerBuilder + Worker with graceful shutdown"
```

---

## Phase 8: Prelude + retry unit tests

### Task 8.1: Prelude

**Files:**
- Create: `crates/altair-temporal/src/prelude.rs`
- Modify: `crates/altair-temporal/src/lib.rs`

- [ ] **Step 1: Write `prelude.rs`**

```rust
//! Convenience re-exports — one `use altair_temporal::prelude::*;` is
//! enough to write straightforward Temporal workflows and activities.

pub use crate::{
    classify_error, delete_schedule, Client, Config, Error, Result, RetryPolicy,
    RetryPolicyBuilder, Schedule, ScheduleBuilder, TlsConfig, Worker, WorkerBuilder,
};

// SDK macros + runtime types that every workflow/activity needs.
pub use temporalio_macros::{activity, activities, workflow, workflow_methods, run};
pub use temporalio_sdk::activities::{ActivityContext, ActivityError, ApplicationFailure};
pub use temporalio_sdk::workflow::{WorkflowContext, WorkflowResult};
```

> **Note:** the macro names (`activity`, `workflow`, `workflow_methods`, `run`) and runtime types come from `temporalio-macros 0.4` and `temporalio-sdk 0.4`. If any name differs, drop the missing one from the prelude — better to have a smaller prelude than break.

- [ ] **Step 2: Wire into `lib.rs`**

After `pub use worker::{Worker, WorkerBuilder};` add:

```rust
pub mod prelude;
```

- [ ] **Step 3: Build**

Run: `cargo build -p altair-temporal`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-temporal/src/prelude.rs crates/altair-temporal/src/lib.rs
git commit -m "feat(temporal): add prelude bundling builders and SDK macros"
```

### Task 8.2: RetryPolicy unit tests

**Files:**
- Modify: `crates/altair-temporal/src/retry.rs`

- [ ] **Step 1: Append `#[cfg(test)] mod tests` to `retry.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_populate_expected_proto_fields() {
        let p = RetryPolicy::builder().build().into_inner();
        assert_eq!(p.backoff_coefficient, 2.0);
        assert_eq!(p.maximum_attempts, 0);
        let initial = p.initial_interval.expect("initial");
        assert_eq!(initial.seconds, 1);
        let max = p.maximum_interval.expect("maximum");
        assert_eq!(max.seconds, 30);
        assert!(p.non_retryable_error_types.is_empty());
    }

    #[test]
    fn overrides_apply() {
        let p = RetryPolicy::builder()
            .initial_interval(Duration::from_millis(500))
            .maximum_interval(Duration::from_secs(60))
            .backoff_coefficient(1.5)
            .max_attempts(7)
            .non_retryable("AuthError")
            .non_retryable("ValidationError")
            .build()
            .into_inner();
        assert_eq!(p.backoff_coefficient, 1.5);
        assert_eq!(p.maximum_attempts, 7);
        let initial = p.initial_interval.expect("initial");
        assert_eq!(initial.seconds, 0);
        assert_eq!(initial.nanos, 500_000_000);
        assert_eq!(p.non_retryable_error_types, vec!["AuthError", "ValidationError"]);
    }

    #[test]
    fn unlimited_attempts_is_zero() {
        let p = RetryPolicy::builder().build().into_inner();
        assert_eq!(p.maximum_attempts, 0);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-temporal --lib retry::`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-temporal/src/retry.rs
git commit -m "test(temporal): unit tests for RetryPolicy proto field mapping"
```

---

## Phase 9: Integration test (gated)

### Task 9.1: testcontainers-based smoke test

**Files:**
- Create: `crates/altair-temporal/tests/integration.rs`

- [ ] **Step 1: Write the gated integration test**

```rust
//! Smoke test: connect a worker to a real Temporal server and run a workflow.
//!
//! Gated behind the `integration-tests` feature + Linux only.

#![cfg(all(feature = "integration-tests", target_os = "linux"))]

use std::sync::Arc;
use std::time::Duration;

use altair_temporal::prelude::*;

// Minimal workflow + activity using the SDK macros directly.
// (Once altair-temporal stabilises, examples flesh this out.)

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worker_runs_a_workflow() {
    // 1. Spin up a Temporal server container.
    //    testcontainers-modules may not have a Temporal module; if not,
    //    use a custom Image impl pointing at `temporalio/temporal:latest`.
    //    Verify during implementation.

    // 2. Build a Config pointing at the container.
    // 3. Build a Worker, register a one-activity workflow, run().
    // 4. With a separate client, start_workflow + wait for completion.
    // 5. Assert the workflow result.

    // Placeholder body — replace with actual implementation once the
    // SDK shapes for register_workflow/register_activities are confirmed
    // in Phase 7.
    panic!("integration test scaffold — flesh out after Phase 7 builds clean");
}
```

> **Note:** this test is intentionally a scaffold. It runs only with `--features integration-tests` and on Linux. The full body is fleshed out once the SDK call shapes are confirmed in Tasks 7.1 + 7.2. For v0.1, treat the worker code path as covered by examples + unit tests of the wrappers.

- [ ] **Step 2: Verify the file compiles without the feature**

Run: `cargo build -p altair-temporal --tests` (default features only)
Expected: clean — integration.rs excluded by `#![cfg(...)]`.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-temporal/tests/integration.rs
git commit -m "test(temporal): gated testcontainers smoke-test scaffold"
```

---

## Phase 10: Examples

### Task 10.1: retry_policy + workflow_id_payload (runnable, no SDK server)

**Files:**
- Create: `crates/altair-temporal/examples/retry_policy.rs`
- Create: `crates/altair-temporal/examples/workflow_id_payload.rs`

- [ ] **Step 1: Write `retry_policy.rs`**

```rust
//! Demonstrate the RetryPolicy builder.
//!
//! Run with: `cargo run -p altair-temporal --example retry_policy`

use std::time::Duration;

use altair_temporal::prelude::*;

fn main() {
    let policy = RetryPolicy::builder()
        .initial_interval(Duration::from_secs(1))
        .maximum_interval(Duration::from_secs(60))
        .backoff_coefficient(2.0)
        .max_attempts(5)
        .non_retryable("AuthError")
        .non_retryable("ValidationError")
        .build();
    let inner = policy.into_inner();
    println!(
        "policy: max_attempts={} backoff={:.1} non_retryable={:?}",
        inner.maximum_attempts, inner.backoff_coefficient, inner.non_retryable_error_types
    );
}
```

- [ ] **Step 2: Write `workflow_id_payload.rs`**

```rust
//! Encode/decode a small payload through a workflow ID.
//!
//! Run with: `cargo run -p altair-temporal --example workflow_id_payload`

use altair_temporal::workflow_id;

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct ArchiveSpec {
    name: String,
    year: u32,
}

fn main() -> anyhow::Result<()> {
    let spec = ArchiveSpec {
        name: "customer".to_string(),
        year: 2026,
    };
    let id = workflow_id::encode("archive", &spec)?;
    println!("workflow id: {id} ({} bytes)", id.len());

    let (prefix, decoded): (String, ArchiveSpec) = workflow_id::decode(&id)?;
    println!("decoded prefix: {prefix}");
    println!("decoded spec:   {decoded:?}");
    assert_eq!(decoded, spec);
    Ok(())
}
```

- [ ] **Step 3: Build + run**

```bash
cargo run -p altair-temporal --example retry_policy
cargo run -p altair-temporal --example workflow_id_payload
```

Expected: both print output and exit 0.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-temporal/examples/retry_policy.rs crates/altair-temporal/examples/workflow_id_payload.rs
git commit -m "docs(temporal): retry_policy and workflow_id_payload examples"
```

### Task 10.2: basic_worker + schedule_cron + with_config + with_otel (`no_run`)

**Files:**
- Create: `crates/altair-temporal/examples/basic_worker.rs`
- Create: `crates/altair-temporal/examples/schedule_cron.rs`
- Create: `crates/altair-temporal/examples/with_config.rs`
- Create: `crates/altair-temporal/examples/with_otel.rs`

- [ ] **Step 1: Write `basic_worker.rs`**

```rust
//! Minimal worker startup. Requires a running Temporal server at
//! `http://localhost:7233` to actually execute; otherwise it errors at
//! connect time.
//!
//! Run with: `cargo run -p altair-temporal --example basic_worker`

use altair_temporal::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut cfg = Config::default();
    cfg.task_queue = "altair-demo".to_string();

    let worker = WorkerBuilder::new(&cfg)
        // .register_workflow::<MyWorkflow>()        // wire your workflow
        // .register_activities(Arc::new(MyActivities))  // wire your activities
        .build()
        .await?;
    println!("worker built; polling task_queue={}", cfg.task_queue);
    worker.run().await?;
    Ok(())
}
```

- [ ] **Step 2: Write `schedule_cron.rs`**

```rust
//! Create a daily cron schedule.
//!
//! Run with: `cargo run -p altair-temporal --example schedule_cron`

use altair_temporal::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut cfg = Config::default();
    cfg.task_queue = "altair-demo".to_string();

    let client = Client::from_config(&cfg).await?;

    Schedule::builder()
        .cron("0 9 * * *")
        .note("daily archive at 09:00 UTC")
        .start_workflow("ArchiveWorkflow", &cfg.task_queue, "archive-daily")
        .create(&client, "daily-archive")
        .await?;

    println!("schedule created: daily-archive");
    Ok(())
}
```

- [ ] **Step 3: Write `with_config.rs`**

```rust
//! Load Config from TOML and connect.
//!
//! Run with: `cargo run -p altair-temporal --example with_config`

use std::io::Write;

use altair_temporal::prelude::*;

#[derive(serde::Deserialize)]
struct AppConfig {
    temporal: Config,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("temporal.toml");
    let mut f = std::fs::File::create(&path)?;
    writeln!(
        f,
        r#"[temporal]
host = "http://localhost:7233"
namespace = "default"
task_queue = "altair-demo"
identity = "altair-temporal-example"
max_concurrent_activities = 50
max_concurrent_workflows = 50
"#
    )?;
    drop(f);

    let raw = std::fs::read_to_string(&path)?;
    let app: AppConfig = toml::from_str(&raw)?;
    println!(
        "loaded config: host={} namespace={} tq={}",
        app.temporal.host, app.temporal.namespace, app.temporal.task_queue
    );

    let _client = Client::from_config(&app.temporal).await?;
    println!("connected.");
    Ok(())
}
```

- [ ] **Step 4: Write `with_otel.rs`**

```rust
//! Cross-crate auto-integration: init altair-otel, then Temporal SDK
//! spans flow through automatically.
//!
//! Run with: `cargo run -p altair-temporal --example with_otel`

use altair_otel::{Config as OtelConfig, Exporter};
use altair_temporal::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    OtelConfig::builder()
        .service_name("temporal-demo")
        .service_version("0.1.0")
        .exporter(Exporter::Stdout)
        .build()
        .init()?;

    let mut cfg = Config::default();
    cfg.task_queue = "altair-demo".to_string();
    let _client = Client::from_config(&cfg).await?;
    println!("connected; SDK spans now flow through altair-otel exporter");

    altair_otel::shutdown();
    Ok(())
}
```

- [ ] **Step 5: Build all four examples**

```bash
cargo build -p altair-temporal --examples
```

Expected: clean build. (Don't try to run them — they require a live Temporal server.)

- [ ] **Step 6: Commit**

```bash
git add crates/altair-temporal/examples/basic_worker.rs crates/altair-temporal/examples/schedule_cron.rs crates/altair-temporal/examples/with_config.rs crates/altair-temporal/examples/with_otel.rs
git commit -m "docs(temporal): basic_worker, schedule_cron, with_config, with_otel examples"
```

---

## Phase 11: README + workspace docs

### Task 11.1: Full crate README

**Files:**
- Modify: `crates/altair-temporal/README.md`

- [ ] **Step 1: Replace the stub**

```markdown
# altair-temporal

[![crates.io](https://img.shields.io/crates/v/altair-temporal.svg)](https://crates.io/crates/altair-temporal)

Stable facade over the pre-1.0 `temporalio-*` Rust SDK: typed `Config`, `Client::from_config`, `WorkerBuilder`, `RetryPolicy` builder, `Schedule` builder, `classify_error` helper, `workflow_id::{encode,decode}`.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Why

The Temporal Rust SDK is pre-1.0. Each minor release is allowed to break public API. `altair-temporal`:

- **Owns** the setup surface — `Config`, `Client`, `Worker`, `RetryPolicy`, `Schedule`, error construction — and keeps it stable across SDK majors.
- **Re-exports** the SDK's runtime types — `WorkflowContext`, `ActivityContext`, the `#[workflow]` / `#[activity]` macros — so consumers depend on `altair-temporal` alone.
- Bumps a major (e.g. `0.1.x` → `0.2.0`) when the underlying SDK breaks. The shielding contract: one changelog to read, not five.

Workflow and activity *bodies* may still need touch-ups across SDK majors — macro-expansion behaviour is intentionally outside the wrap.

## Install

```toml
[dependencies]
altair-temporal = "0.1"
```

## Quick start

```rust
use altair_temporal::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut cfg = Config::default();
    cfg.task_queue = "demo".to_string();

    let worker = WorkerBuilder::new(&cfg)
        // .register_workflow::<MyWorkflow>()
        // .register_activities(std::sync::Arc::new(MyActivities))
        .build()
        .await?;
    worker.run().await?;
    Ok(())
}
```

## What it gives you

- **Typed `Config`** (`serde::Deserialize` for altair-config integration).
- **`Client::from_config`** — async factory returning the SDK client ready for `start_workflow`, schedule ops, etc.
- **`WorkerBuilder`** — fluent builder over `temporalio-sdk-core` worker setup; `run()` polls until SIGINT/SIGTERM; `run_with_shutdown(future)` for custom drivers.
- **`RetryPolicy::builder()`** — replaces hand-built `prost_wkt_types::Duration` proto with a `.max_attempts(5).backoff_coefficient(2.0).non_retryable("X").build()` chain.
- **`Schedule::builder()`** — `cron`/`interval`/`note`/`paused`/`start_workflow` then terminal `create`/`update`/`delete`.
- **`classify_error()`** — `ActivityError` construction with `non_retryable` decided by a predicate.
- **`workflow_id::encode` / `decode`** — pack a small structured payload into a workflow ID (sidestepping the SDK's "scheduled workflows can't carry input" limitation).

## Examples

| File | Demonstrates |
|---|---|
| `basic_worker.rs` | Minimal `WorkerBuilder` → `worker.run()`. |
| `retry_policy.rs` | `RetryPolicy::builder()` (runnable, no server needed). |
| `schedule_cron.rs` | Create a daily cron schedule. |
| `workflow_id_payload.rs` | Encode/decode a struct through a workflow ID (runnable). |
| `with_config.rs` | `Config` loaded from TOML. |
| `with_otel.rs` | Cross-crate auto-integration: SDK spans → altair-otel. |

Run any: `cargo run -p altair-temporal --example <name>`.

## Versioning

altair-temporal pins `temporalio-* = "~0.4"`. When the SDK ships breaking changes (e.g. 0.5.0), altair-temporal bumps to its next major. Consumers stay on the previous altair-temporal major until they choose to migrate. The crate's `Error` type uses boxed source variants specifically so the wrapper's public API doesn't churn when SDK error types do.

## License

Apache-2.0
```

- [ ] **Step 2: Commit**

```bash
git add crates/altair-temporal/README.md
git commit -m "docs(temporal): full crate README with versioning contract"
```

### Task 11.2: Workspace README + porting tracker

**Files:**
- Modify: `README.md` (root)
- Modify: `docs/porting-tracker.md`

- [ ] **Step 1: Update workspace README**

In root `README.md`, find the crate table. Add an `altair-temporal` row consistent with the others (name, version pattern, one-line description).

- [ ] **Step 2: Update `docs/porting-tracker.md`**

- Find the row for `altair-temporal` (currently in "At-Risk / Harder Ports" with status `💤 Deferred`). Move it to the Starter Set table with status `✅ Done`. Notes: "Stable facade over pre-1.0 temporalio-* SDK; Config + builders + workflow_id helpers."
- In the "Published crates" table near the top, add an `altair-temporal` row mirroring `altair-db`'s `0.1.x (TBD)` shape.
- Add a release-notes bullet:

```markdown
- **`altair-temporal` 0.1.x** (date TBD on publish) — Stable facade over pre-1.0 temporalio-* Rust SDK. Typed Config, Client/Worker builders, RetryPolicy + Schedule builders, classify_error, workflow_id encode/decode.
```

- Update the "Last updated:" line at the top to today's date.

- [ ] **Step 3: Commit**

```bash
git add README.md docs/porting-tracker.md
git commit -m "docs: register altair-temporal in workspace README and porting tracker"
```

---

## Phase 12: CI gate + PR + merge

### Task 12.1: Full workspace gate

**Files:** none (verification only)

- [ ] **Step 1: Format**

```bash
cargo fmt --all
cargo fmt --all --check
```

Expected: exit 0.

- [ ] **Step 2: Clippy**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: exit 0, no warnings. Common fixes if any surface: `doc_markdown` (backtick `SDK` / `OTel` / `Temporal`), `must_use_candidate` (add `#[must_use]`), `result_large_err` (box a variant or `#[allow]` on the function).

- [ ] **Step 3: Tests**

```bash
cargo test --workspace
```

Expected: every crate's tests pass; altair-temporal reports unit + workflow_id integration tests green.

- [ ] **Step 4: cargo-deny**

```bash
cargo deny check        # if installed locally
```

Expected: bans/licenses/sources/advisories all `ok`. Likely new transitive crates (tonic, prost, hyper, etc.) — if any new RUSTSEC advisories surface, follow the altair-db precedent: add to `deny.toml`'s `advisories.ignore` with a one-line justification.

- [ ] **Step 5: Publish dry-run**

```bash
cargo publish --dry-run -p altair-temporal
```

Expected: packaging + verification succeed; ends with "aborting upload due to dry run".

- [ ] **Step 6: If anything fails, fix and re-run; no commit yet**

### Task 12.2: Push + PR + foreground-poll CI + squash-merge

**Files:** none

- [ ] **Step 1: Push branch**

```bash
git push -u origin feat/altair-temporal
```

- [ ] **Step 2: Open PR**

```bash
gh pr create --title "feat(temporal): add altair-temporal crate (temporalio-sdk facade)" --body "$(cat <<'EOF'
## Summary

New crate \`altair-temporal\`: a stable facade over the pre-1.0 \`temporalio-*\` Rust SDK.

- One \`Config\` + \`Client::from_config\` factory; \`WorkerBuilder\` with separate slot suppliers + graceful shutdown.
- \`RetryPolicy::builder()\` replaces hand-built proto \`Duration\` structs.
- \`Schedule::builder()\` for cron / interval schedules with create/update/delete.
- \`classify_error()\` helper for permanent-vs-transient \`ActivityError\` construction.
- \`workflow_id::{encode,decode}\` for ID-encoded payloads (sidesteps the SDK's no-input-on-scheduled-workflows limitation).
- All five \`temporalio-*\` crates re-exported at the root.
- Boxed error sources keep the public \`Error\` stable across SDK majors.

Design: \`docs/specs/2026-05-29-altair-temporal-design.md\`
Plan: \`docs/plans/2026-05-29-altair-temporal-implementation.md\`

## Test plan

- [x] Unit tests: Error, Config, Schedule builder, RetryPolicy builder
- [x] Integration tests: workflow_id encode/decode round-trip + edge cases
- [x] All 6 examples build clean
- [x] \`cargo publish --dry-run -p altair-temporal\` succeeds
- [x] Workspace clippy + rustfmt clean
- [ ] CI green on PR
EOF
)"
```

- [ ] **Step 3: Foreground-poll CI**

```bash
until [ "$(gh pr checks --json state,bucket | python3 -c 'import json,sys; d=json.loads(sys.stdin.read()); print("done" if all(c.get("bucket") in ("pass","fail","skipping","cancel") for c in d) else "wait")')" = "done" ]; do sleep 20; done
gh pr checks
```

Expected: all checks pass. If anything fails, push fixes to the same branch and re-poll.

- [ ] **Step 4: Squash-merge + delete branch**

```bash
gh pr merge --squash --delete-branch
```

- [ ] **Step 5: Sync local main**

```bash
git checkout main
git pull
```

- [ ] **Step 6: Handle release-plz PR**

Release-plz will open a `chore: release` PR that publishes altair-temporal for the first time (and bumps the other crates to the same workspace version). Verify the diff includes altair-temporal as a real new entry; if so, merge. Empty churn PRs (no new crate, no consumable code change) get closed.

---

## Self-review notes (for the executor)

- **Boxed error variants are deliberate.** Don't "fix" them to be concrete SDK types. That's the entire shielding contract.
- **`register_workflow::<W>()` generic bound** may need adjustment depending on what the macros emit. If the bound doesn't compile, *try the bound without the trait* first (let the SDK error tell you the real bound), then add what it asks for.
- **`pub(crate)` fields on `Schedule`** are deliberate so unit tests in `src/schedule.rs` can inspect builder state. Don't widen to `pub`.
- **Backend integration test (`tests/integration.rs`)** is scaffold-only at v0.1. Don't extend it to a full feature matrix until SDK API shapes are nailed down — flake on CI from pre-1.0 SDK shifts isn't worth it yet.
- **The `with_otel.rs` example is `no_run` in practice** because the Stdout exporter won't have spans to emit until the worker actually polls (which needs a server). Running it should still init + shutdown cleanly without panicking.
- **If clippy flags `result_large_err` on `Db::connect`-style methods** that return `Result<Worker>`, prefer `#[allow(clippy::result_large_err)]` on the function over boxing the Error variant; the boxed-source design already keeps `Error` slim.
