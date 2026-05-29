# altair-temporal — Design

**Date:** 2026-05-29
**Status:** Draft — awaiting review before implementation planning
**Author:** Jasoet
**Spec type:** Brainstorming output → input to writing-plans

---

## 1. Overview

`altair-temporal` is a thin convenience layer over the five `temporalio-*` Rust SDK crates (`sdk`, `sdk-core`, `client`, `common`, `macros`). It owns a typed `Config`, a `ClientBuilder`, a `WorkerBuilder`, a `RetryPolicy` builder, a `Schedule` builder, a `classify_error` helper for permanent-vs-transient activity errors, and a `workflow_id::{encode,decode}` helper for sidestepping the SDK's "scheduled workflows can't carry input" limitation. All five `temporalio-*` crates are re-exported at the crate root so consumers depend on `altair-temporal` alone.

**One-line product goal:** Stop hand-building Temporal worker/client bootstrap, retry-policy proto structs, and schedule specs in every project — and shield consumers from pre-1.0 SDK churn behind a versioned facade.

**Insulation contract.** altair-temporal owns the *setup and helper surface* — config, builders, error construction. It re-exports the SDK's runtime types (`WorkflowContext`, `ActivityContext`, the `#[workflow]` / `#[activity]` macros). When the SDK ships breaking changes:

- altair-temporal bumps a major (e.g. 0.1.x → 0.2.0) tied to the SDK majors it supports.
- The wrapper's owned API stays stable across SDK majors when feasible; changes only at altair-temporal majors.
- Workflow and activity *bodies* may still need touch-ups across SDK majors — macro-expansion behaviour is intentionally out of scope to wrap.

Bootstrap, retry construction, schedule CRUD, and error classification are the surfaces consumers paid the most attention to in Archive-rs (the first consumer); those are the surfaces this crate shields most strongly.

## 2. Decisions Locked

| Decision | Choice |
|---|---|
| Crate name | `altair-temporal` (verified available on crates.io 2026-05-29) |
| SDK pinning | `temporalio-* = "~0.4"` in workspace deps (minor-locked) |
| Re-exports | All five `temporalio-*` crates: `sdk`, `sdk-core`, `client`, `common`, `macros` |
| Async runtime | tokio; `CoreRuntime::new_assume_tokio(...)` |
| Config | Typed `Config` for altair-config (host, namespace, task_queue, identity, concurrency limits, optional TLS) |
| Owned helpers | `Client::from_config`, `WorkerBuilder`, `RetryPolicy::builder()`, `Schedule::builder()`, `classify_error()`, `workflow_id::{encode,decode}` |
| Error type | `thiserror` enum with **boxed sources** (`Connect`, `Worker`, `Client`, `Schedule`, `Configuration`) to keep the API stable across SDK majors |
| Tracing | SDK emits `tracing` events natively; consumers initialise `altair-otel` and spans flow through automatically |
| Default features | `tls` on, `integration-tests` off |
| Edition / MSRV | Inherit workspace (Edition 2024, Rust 1.95) |

## 3. Architecture

### 3.1 File layout

```
crates/altair-temporal/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs          # crate root: lints, mod decls, re-exports (5 temporalio-* crates)
│   ├── error.rs        # Error enum + Result alias (thiserror, boxed sources)
│   ├── config.rs       # Config struct + TlsConfig + Default + serde
│   ├── client.rs       # Client::from_config() — returns SDK client connected to namespace
│   ├── worker.rs       # WorkerBuilder + Worker wrapper, register_workflow / register_activities / run()
│   ├── retry.rs        # RetryPolicy newtype + RetryPolicyBuilder
│   ├── schedule.rs     # Schedule newtype + ScheduleBuilder; create / update / delete helpers
│   ├── activity.rs     # classify_error() helper
│   ├── workflow_id.rs  # encode<T: Serialize> / decode<T: DeserializeOwned>
│   └── prelude.rs      # one-import bundle
├── tests/
│   ├── retry.rs        # RetryPolicy builder unit tests
│   ├── schedule.rs     # ScheduleBuilder unit tests
│   ├── workflow_id.rs  # encode/decode round-trip + edge cases
│   └── integration.rs  # gated; testcontainers-based worker/client smoke test
└── examples/
    ├── basic_worker.rs
    ├── retry_policy.rs
    ├── schedule_cron.rs
    ├── workflow_id_payload.rs
    ├── with_config.rs
    └── with_otel.rs
```

Workspace + repo edits:
- `Cargo.toml`: add the five `temporalio-*` crates plus `prost-wkt-types` to `[workspace.dependencies]`; add `"crates/altair-temporal"` to `members`.
- `docs/porting-tracker.md`: move `altair-temporal` from "At-Risk" → published crates; add release-notes bullet.
- `README.md`: add `altair-temporal` row to the crates table.

### 3.2 Module responsibilities

- **`error.rs`** — sole owner of `Error` and `Result<T>`. Variants `Connect { host, source }`, `Client(source)`, `Worker(source)`, `Schedule(source)`, `Configuration(String)`. All non-`Configuration` source fields use `Box<dyn std::error::Error + Send + Sync>` so the public type doesn't churn when SDK error types do. Consumers downcast via `err.source()` when needed.
- **`config.rs`** — `Config` and `TlsConfig`. `serde::Deserialize` with `#[serde(default)]`; defaults match Archive-rs's working numbers. No runtime validation beyond "task_queue is non-empty" — TLS file existence is validated at connect time.
- **`client.rs`** — `pub async fn Client::from_config(cfg: &Config) -> Result<temporalio_client::Client>`. Pure factory; returns the SDK client directly so consumers retain its full surface for `start_workflow`, `get_schedule_handle`, etc.
- **`worker.rs`** — `WorkerBuilder::new(cfg)` → tuning methods → registration methods → `build().await` → `Worker` → `run().await` / `run_with_shutdown(fut).await`. Internally builds `CoreRuntime::new_assume_tokio`, `WorkerOptions` with `FixedSizeSlotSupplier` (separate suppliers per slot type, sized from the config's concurrency limits).
- **`retry.rs`** — `RetryPolicy` newtype around `temporalio_common::protos::temporal::api::common::v1::RetryPolicy`. `RetryPolicyBuilder` accepts `std::time::Duration` and converts to `prost_wkt_types::Duration` internally. `into_inner()` peels for SDK calls.
- **`schedule.rs`** — `Schedule` + `ScheduleBuilder` with `cron(&str)`, `interval(Duration)`, `note(s)`, `paused(b)`, `start_workflow(workflow_type, task_queue, workflow_id)`. Calling `cron` and `interval` both populates *both* in the resulting `ScheduleSpec` (Temporal accepts and runs the union — useful for "every 5 minutes AND also at 9 AM Mondays"); callers wanting strictly one ensure they call only that one. Terminal methods `create(client, id).await`, `update(client, id).await`; standalone `delete(client, id).await`. **No upsert** — keep policy decisions in consumer code.
- **`activity.rs`** — `pub fn classify_error<E, F>(err: E, is_permanent: F) -> ActivityError`. One helper, narrow surface.
- **`workflow_id.rs`** — `encode(prefix, &payload)` produces `{prefix}-{base32_jsonbytes}` using Crockford Base32 (via `altair-base32`). `decode` reverses. Errors on overlong payloads (Temporal's 200-byte workflow-ID limit).
- **`prelude.rs`** — `pub use crate::{Config, Client, Worker, WorkerBuilder, RetryPolicy, Schedule, Error, Result, classify_error};` plus the SDK macros consumers always need.

### 3.3 Public API surface

```rust
// crate root
pub use config::{Config, TlsConfig};
pub use client::Client;
pub use worker::{Worker, WorkerBuilder};
pub use retry::{RetryPolicy, RetryPolicyBuilder};
pub use schedule::{Schedule, ScheduleBuilder};
pub use activity::classify_error;
pub use error::{Error, Result};

pub mod workflow_id;
pub mod prelude;

// Underlying-lib re-exports
pub use ::temporalio_sdk;
pub use ::temporalio_sdk_core;
pub use ::temporalio_client;
pub use ::temporalio_common;
pub use ::temporalio_macros;
```

```rust
// config.rs
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub host: String,                       // "http://localhost:7233"
    pub namespace: String,                  // "default"
    pub task_queue: String,                 // required
    pub identity: String,                   // "altair-temporal"
    pub max_concurrent_activities: u32,     // 100
    pub max_concurrent_workflows: u32,      // 100
    pub tls: Option<TlsConfig>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TlsConfig {
    pub server_root_ca_cert: PathBuf,
    pub client_cert: Option<PathBuf>,
    pub client_key: Option<PathBuf>,
    pub server_name_override: Option<String>,
}
```

```rust
// client.rs
pub struct Client;

impl Client {
    pub async fn from_config(cfg: &Config) -> Result<temporalio_client::Client>;
}
```

```rust
// worker.rs
pub struct WorkerBuilder { /* private */ }

impl WorkerBuilder {
    pub fn new(cfg: &Config) -> Self;
    pub fn identity(self, id: impl Into<String>) -> Self;
    pub fn max_concurrent_activities(self, n: u32) -> Self;
    pub fn max_concurrent_workflows(self, n: u32) -> Self;
    pub fn register_workflow<W>(self) -> Self
    where
        W: temporalio_sdk::workflow::Workflow + 'static;
    pub fn register_activities<A>(self, instance: std::sync::Arc<A>) -> Self
    where
        A: temporalio_sdk::activities::ActivityRegistration + Send + Sync + 'static;
    pub async fn build(self) -> Result<Worker>;
}

pub struct Worker { /* private */ }

impl Worker {
    pub async fn run(self) -> Result<()>;
    pub async fn run_with_shutdown<F>(self, shutdown: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static;
}
```

```rust
// retry.rs
pub struct RetryPolicy(temporalio_common::protos::temporal::api::common::v1::RetryPolicy);

impl RetryPolicy {
    pub fn builder() -> RetryPolicyBuilder;
    pub fn into_inner(self) -> temporalio_common::protos::temporal::api::common::v1::RetryPolicy;
}

#[derive(Debug, Clone)]
pub struct RetryPolicyBuilder { /* private */ }

impl RetryPolicyBuilder {
    pub fn initial_interval(self, d: Duration) -> Self;       // default 1s
    pub fn maximum_interval(self, d: Duration) -> Self;       // default 30s
    pub fn backoff_coefficient(self, c: f64) -> Self;         // default 2.0
    pub fn max_attempts(self, n: u32) -> Self;                // default 0 (unlimited)
    pub fn non_retryable(self, error_type: impl Into<String>) -> Self; // repeatable
    pub fn build(self) -> RetryPolicy;
}
```

```rust
// schedule.rs
pub struct Schedule { /* private */ }

impl Schedule { pub fn builder() -> ScheduleBuilder; }

#[derive(Debug, Clone)]
pub struct ScheduleBuilder { /* private */ }

impl ScheduleBuilder {
    pub fn cron(self, cron: impl Into<String>) -> Self;
    pub fn interval(self, d: Duration) -> Self;
    pub fn note(self, n: impl Into<String>) -> Self;
    pub fn paused(self, p: bool) -> Self;
    pub fn start_workflow(
        self,
        workflow_type: impl Into<String>,
        task_queue: impl Into<String>,
        workflow_id: impl Into<String>,
    ) -> Self;

    pub async fn create(self, client: &temporalio_client::Client, id: impl Into<String>) -> Result<()>;
    pub async fn update(self, client: &temporalio_client::Client, id: impl Into<String>) -> Result<()>;
}

pub async fn delete(client: &temporalio_client::Client, id: &str) -> Result<()>;
```

```rust
// activity.rs
pub fn classify_error<E, F>(err: E, is_permanent: F) -> temporalio_sdk::activities::ActivityError
where
    E: std::error::Error + Send + Sync + 'static,
    F: FnOnce(&E) -> bool;
```

```rust
// workflow_id.rs
pub fn encode<T: serde::Serialize>(prefix: &str, payload: &T) -> Result<String>;
pub fn decode<T: serde::de::DeserializeOwned>(id: &str) -> Result<(String, T)>;
```

```rust
// error.rs
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to connect to temporal at {host}")]
    Connect { host: String, #[source] source: BoxError },

    #[error("temporal client error")]
    Client(#[source] BoxError),

    #[error("temporal worker error")]
    Worker(#[source] BoxError),

    #[error("temporal schedule error")]
    Schedule(#[source] BoxError),

    #[error("invalid configuration: {0}")]
    Configuration(String),
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;
```

**Notes on the choices**

- Boxed error sources are deliberate — see §1. The single shielding promise that costs the most across SDK majors is "consumer code mentions SDK error types"; we minimise that by keeping our `Error` opaque at the source.
- `Client::from_config` returns the SDK client (not an altair wrapper) because client surface — `start_workflow`, `get_schedule_handle`, `list_schedules`, signals (future) — is too broad to mirror without re-implementing the whole client. Re-export + factory is the right tradeoff.
- `register_workflow` and `register_activities` use the SDK's trait bounds verbatim; the wrapper accepts whatever the macros produce. This is a known concession to the macro shielding limit.

## 4. Behaviour details

### 4.1 Worker lifecycle

`WorkerBuilder::build()` does, in order:

1. Build `ClientOptions` from `Config` (host, namespace, identity, optional TLS).
2. Connect the SDK client. Failure → `Error::Connect { host, source }`.
3. Build `CoreRuntime::new_assume_tokio(RuntimeOptions::default())` (the only valid form inside a tokio app).
4. Build `WorkerConfig` with `task_queue`, `identity`, and two `FixedSizeSlotSupplier`s sized from `max_concurrent_activities` and `max_concurrent_workflows` (separate suppliers, not the combined-cap shortcut Archive-rs uses today).
5. Register every queued workflow and activity.
6. Return `Worker` holding the underlying core worker plus the client (kept alive for the worker's lifetime).

`Worker::run()` installs `SIGINT + SIGTERM` (Unix) / `SIGINT` (Windows) handlers and polls until one fires, then calls the SDK's `initiate_shutdown` + `await_workflow_completions`. `Worker::run_with_shutdown(fut)` accepts a custom shutdown driver.

### 4.2 TLS

When `Config.tls` is `Some(TlsConfig)`, the client is configured with:

- `server_root_ca_cert = read(&server_root_ca_cert)?`
- If both `client_cert` and `client_key` are present → mutual TLS via the SDK's client-identity TLS hook.
- If `server_name_override` is set → forwarded as gRPC SNI/server-name override.

When `tls` is `None`, the connection is plaintext (matches the local dev pattern). Validation errors at config-time (missing files, mismatched key/cert pair) surface as `Error::Configuration`.

### 4.3 `RetryPolicy` defaults

`RetryPolicyBuilder` starts with: `initial_interval = 1s`, `maximum_interval = 30s`, `backoff_coefficient = 2.0`, `max_attempts = 0` (Temporal convention for unlimited), `non_retryable_error_types = []`. A bare `RetryPolicy::builder().build()` matches the SDK's documented defaults exactly.

`.non_retryable("HskError::Forbidden")` appends to the list — repeatable for multiple error type names. The list is matched against the `type` field of `ApplicationFailure` by Temporal at runtime, so strings must match what the activity's `classify_error` helper actually emits.

### 4.4 Schedule create / update semantics

`create` calls `CreateSchedule` — fails with `Error::Schedule` if a schedule with the same ID exists. `update` calls `UpdateSchedule` via `get_schedule_handle(id).update(|u| ...)`, replacing spec / paused / note. There is no upsert helper in v0.1 — callers compose `create` + fallback-to-`update` themselves.

Note + paused are applied separately from spec so updates can flip pause state without rewriting the schedule.

### 4.5 `workflow_id` encoding

`encode(prefix, payload)`:

1. Serialise `payload` to JSON bytes (`serde_json::to_vec`).
2. Encode bytes via Crockford Base32 (`altair_base32::encode`).
3. Return `format!("{prefix}-{encoded}")`.
4. Error with `Error::Configuration` if the result exceeds 200 bytes (Temporal's workflow-ID limit).

`decode(id)`:

1. Split on the last `-` (allowing prefix to contain hyphens).
2. Decode the right half via `altair_base32::decode`.
3. Deserialise via `serde_json::from_slice`.
4. Return `(prefix, payload)` or `Error::Configuration` on any failure.

Suitable only for small structured payloads. Larger ones belong in activity input. The README documents the limit and recommended pattern.

### 4.6 Tracing + OTel

The Temporal SDK emits `tracing` events on its standard span hierarchy (worker poll, workflow execution, activity execution). When the host initialises `altair-otel`, those spans flow to the configured exporter — no extra wiring in altair-temporal. The `with_otel.rs` example demonstrates this against a one-activity workflow.

### 4.7 altair-config integration

```toml
[temporal]
host = "https://temporal.prod.internal:7233"
namespace = "archive"
task_queue = "archive-tq"
identity = "archive-rs-worker"
max_concurrent_activities = 50
max_concurrent_workflows = 50

[temporal.tls]
server_root_ca_cert = "/etc/temporal/ca.pem"
client_cert = "/etc/temporal/client.crt"
client_key = "/etc/temporal/client.key"
```

```rust
#[derive(serde::Deserialize)]
struct AppConfig { temporal: altair_temporal::Config }

let app: AppConfig = altair_config::load("config", "ARCHIVE")?;
let client = altair_temporal::Client::from_config(&app.temporal).await?;
```

## 5. Testing

### 5.1 Unit tests (in `src/`)

- `retry.rs`: builder defaults round-trip via `into_inner()` and back; `non_retryable("X").non_retryable("Y")` produces the expected list; `Duration` → `prost_wkt_types::Duration` carries the right seconds/nanos.
- `schedule.rs`: builder populates the right proto fields for cron vs interval; `paused(true)` and `note(s)` set their fields.
- `workflow_id.rs`: encode/decode round-trip; prefixes containing hyphens parse correctly; oversized payloads error; invalid Base32 errors; invalid JSON errors.
- `config.rs`: minimal TOML round-trip; nested `[temporal.tls]` deserialises into `Some(TlsConfig)`.
- `activity.rs`: `classify_error(e, |_| true)` yields `ActivityError` with `non_retryable = true`; `false` yields it without.

All run on every commit, no infrastructure required.

### 5.2 Integration tests (`tests/integration.rs`, gated)

- `#[cfg(feature = "integration-tests")]` + `target_os = "linux"`.
- `testcontainers-modules` spins up a Temporal server image. Connect client + start a one-activity workflow + assert completion.
- One smoke test, not a full feature matrix. Unit tests cover owned surface; this proves the wrapper actually talks to a real server.

### 5.3 Doc-tests

Every public function gets at least one doc-test per the workspace rule. Examples needing a live server use `no_run`; pure types (RetryPolicy, ScheduleBuilder, workflow_id) get real doc-tests.

### 5.4 CI matrix

The existing `test` job runs all unit + doc tests on every commit. The integration test is excluded from default CI (gated feature) until the SDK stabilises further.

## 6. Examples — what each demonstrates

| File | Demonstrates |
|---|---|
| `basic_worker.rs` | Minimal: `Config::default()` (with `task_queue`) → `WorkerBuilder` → register one activity → `worker.run().await`. `no_run` doc-block. |
| `retry_policy.rs` | `RetryPolicy::builder().max_attempts(5).backoff_coefficient(2.0).non_retryable("Forbidden").build()` plugged into `ActivityOptions`. Compile-only example. |
| `schedule_cron.rs` | `Schedule::builder().cron("0 9 * * *").start_workflow(...).create(client, "daily-archive").await?`. `no_run`. |
| `workflow_id_payload.rs` | `let id = workflow_id::encode("archive", &MyPayload { ... })?; let (prefix, p) = workflow_id::decode::<MyPayload>(&id)?;`. Fully runnable. |
| `with_config.rs` | Load `Config` from TOML via serde; `Client::from_config(&cfg)`. `no_run` on the connect call. |
| `with_otel.rs` | `altair_otel::init` + cross-crate auto-instrumentation. `no_run`. |

## 7. Out of Scope for v0.1

- Child workflows, signals, queries, sagas — not used by Archive-rs and significant added API surface. Add when a real consumer needs them.
- Upsert helper for schedules — callers compose create + fallback-to-update; opinionated policy stays out.
- Custom slot suppliers beyond `FixedSizeSlotSupplier` — consumers drop down via the re-exported `temporalio_sdk_core` for finer control.
- A wrapping `#[workflow]` / `#[activity]` macro — incompatible with the shielding-cost tradeoff (see §1).
- Worker metrics shipping — `temporalio_sdk_core` exposes metrics traits; consumers wire those directly.
- Replay-testing helpers (the SDK has `WorkflowReplayer`) — own scope; add later.
- Workflow versioning helpers — add if/when needed.
- `temporal-cli` integration / dev-server bootstrap — out of scope for a library crate.

## 8. Dependencies (workspace + crate)

Workspace `Cargo.toml` additions:

```toml
# Temporal
temporalio-sdk = "~0.4"
temporalio-sdk-core = "~0.4"
temporalio-client = "~0.4"
temporalio-common = "~0.4"
temporalio-macros = "~0.4"
prost-wkt-types = "0.7"
```

Crate `Cargo.toml`:

```toml
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
altair-otel = { path = "../altair-otel", version = "0.1" }
altair-config = { path = "../altair-config", version = "0.1" }
```

## 9. Implementation-time Verifications

These are settled at planning time, not design time — they don't change the architecture.

1. Exact `Workflow` and `ActivityRegistration` trait bounds in `temporalio-sdk 0.4` — the macros may use slightly different names; adjust the `register_*` generic bounds to match without changing public signatures.
2. The `prost_wkt_types::Duration` field shape (`seconds: i64`, `nanos: i32`) — confirmed standard but verify the crate's path before constructing.
3. Whether `temporalio_client::Client::create_schedule` requires the proto enum directly or accepts a builder. Adjust `ScheduleBuilder::create` body accordingly.
4. `testcontainers-modules` Temporal server image existence and port shape — verify or fall back to a custom Image impl using `temporalio/temporal:latest`.
