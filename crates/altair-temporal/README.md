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
    let cfg = Config {
        task_queue: "demo".to_string(),
        ..Config::default()
    };

    let worker = WorkerBuilder::new(&cfg)
        // .register_workflow::<MyWorkflow>()
        // .register_activities(MyActivities)
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
- **`Schedule::builder()`** — `cron`/`interval`/`note`/`paused`/`start_workflow` then terminal `create`/`update`/`delete_schedule(client, id)`.
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
