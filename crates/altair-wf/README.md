# altair-wf

[![crates.io](https://img.shields.io/crates/v/altair-wf.svg)](https://crates.io/crates/altair-wf)

Reusable Temporal workflow patterns plus two opt-in feature modules:

- **Core patterns** (always on): single-task, pipeline (sequential), parallel, loop, parameterized loop, and DAG with cycle detection. Ports the `workflow` module of [`github.com/jasoet/go-wf`](https://github.com/jasoet/go-wf).
- **`function` feature**: named-handler registry + a single Temporal activity that dispatches by name. Lets a workflow run "this batch of named jobs" without declaring a typed activity per handler.
- **`datasync` feature**: a `Source` → `Mapper` → `Sink` pipeline (in-process `Runner` and a Temporal workflow shape) plus a `chunk` submodule that adds partitioned, resumable orchestration with continue-as-new support.

Built on [altair-temporal](https://crates.io/crates/altair-temporal). Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Why

Most real-world Temporal workflows are one of a handful of shapes: do a single thing; do A then B then C; fan out N copies of the same activity; or run a DAG of activities. `altair-wf` ships those shapes as composable async helpers so you can drop them into your own `#[workflow]` definitions without re-implementing the orchestration logic each time.

The patterns are **SDK-agnostic** — each takes an `execute_one` closure that you wire to `WorkflowContext::start_activity` inside your workflow's `run` method. This keeps the orchestration pure (unit-testable without Temporal) and lets your code own all activity dispatch.

## Install

```toml
[dependencies]
altair-wf = "0.2"
altair-temporal = "0.2"
# Required by the Temporal SDK's #[workflow] / #[activities] macros:
futures = "0.3"
futures-util = "0.3"
```

Opt-in features:

```toml
[dependencies]
altair-wf = { version = "0.2", features = ["function", "datasync"] }
```

## Runnable examples

End-to-end examples for every feature surface live in
[`examples/`](examples/) — see [`examples/README.md`](examples/README.md)
for the table and run instructions. Highlights:

- `cargo run -p altair-wf --example hello_pipeline` — pipeline against
  a local Temporal dev server.
- `cargo run -p altair-wf --features datasync --example datasync_runner`
  — in-process `Source → Mapper → Sink`, no Temporal needed.
- `cargo run -p altair-wf --features datasync --example datasync_chunked`
  — partitioned + cursor + continue-as-new (the load-bearing path).

Most examples need `temporal server start-dev` running on
`localhost:7233`.

## Patterns

### Single task

```rust,no_run
use altair_wf::prelude::*;

# async fn ex() -> altair_wf::Result<()> {
# #[derive(Clone, serde::Serialize, serde::Deserialize)]
# struct MyTask { name: String }
# impl TaskInput for MyTask {}
# #[derive(serde::Serialize, serde::Deserialize)]
# struct MyResult { ok: bool }
# impl TaskOutput for MyResult { fn is_success(&self) -> bool { self.ok } }
let result: MyResult = execute(MyTask { name: "demo".into() }, |task| async move {
    // ctx.start_activity(MyActivities::run, task, opts).await.map_err(...)
    Ok(MyResult { ok: true })
}).await?;
# Ok(()) }
```

### Pipeline (sequential)

```rust,no_run
let input = PipelineInput {
    tasks: vec![step_a, step_b, step_c],
    stop_on_error: true,
    cleanup: false,
};
let out: PipelineOutput<StepResult> = pipeline(input, dispatch).await?;
```

### Parallel

```rust,no_run
let input = ParallelInput {
    tasks: vec![worker_1, worker_2, worker_3],
    failure_strategy: FailureStrategy::FailFast,
    max_in_flight: 0, // 0 = no cap; set e.g. 50 to bound memory under fan-out
};
let out: ParallelOutput<StepResult> = parallel(input, dispatch).await?;
```

### Loop (per-item)

```rust,no_run
use std::sync::Arc;

let substitutor: Substitutor<MyTask> = Arc::new(|template, item, index, _params| {
    let mut t = template.clone();
    t.name = format!("{}-{item}-{index}", template.name);
    t
});
let input = LoopInput {
    items: vec!["a".into(), "b".into(), "c".into()],
    template: my_task_template,
    parallel: true,
    failure_strategy: FailureStrategy::Continue,
    max_in_flight: 0,
};
let out: LoopOutput<MyResult> = run_loop(input, substitutor, dispatch).await?;
```

### Parameterized loop (cartesian product)

```rust,no_run
let mut params = std::collections::HashMap::new();
params.insert("region".into(), vec!["us-east-1".into(), "eu-west-1".into()]);
params.insert("tier".into(),   vec!["standard".into(), "premium".into()]);
let input = ParameterizedLoopInput {
    parameters: params,
    template,
    parallel: false,
    failure_strategy: FailureStrategy::FailFast,
    max_in_flight: 0,
};
let out = parameterized_loop(input, substitutor, dispatch).await?; // 4 iterations
```

### DAG

```rust,no_run
let input = DAGInput {
    nodes: vec![
        DAGNode { name: "build".into(),  input: build_task,  dependencies: vec![] },
        DAGNode { name: "test".into(),   input: test_task,   dependencies: vec!["build".into()] },
        DAGNode { name: "lint".into(),   input: lint_task,   dependencies: vec!["build".into()] },
        DAGNode { name: "deploy".into(), input: deploy_task, dependencies: vec!["test".into(), "lint".into()] },
    ],
    fail_fast: true,
    max_parallel: 0,
};
let out: DAGOutput<StepResult> = run_dag(input, dispatch).await?;
```

The DAG runner dispatches in topological layers — independent nodes in a layer run in parallel.

## Workflow context and closure bounds

The patterns are SDK-agnostic, but they bind the dispatch closure
differently depending on whether work runs sequentially or in
parallel:

| Pattern | Closure bound | Reason |
|---|---|---|
| `execute` / `pipeline` | `FnMut(I) -> Future<O>` | Closure runs one at a time; mutable state allowed. |
| `parallel` / `run_loop` / `parameterized_loop` / `run_dag` | `Fn(I) -> Future<O>` | Closure is reused across many in-flight futures (`join_all`). Capture via `Arc<Mutex<_>>` if you need mutation. |

When you call a `Fn`-bound pattern from inside a `#[run]` method, the
SDK gives you `ctx: &mut WorkflowContext<Self>` — a *mutable* reference,
which can't be captured into an `Fn` closure. The fix is a one-line
shared reborrow:

```rust,ignore
// Inside #[workflow_methods] impl ... { #[run] pub async fn run(ctx: &mut WorkflowContext<Self>, ...) }
let ctx_ref: &WorkflowContext<Self> = ctx;     // <-- the reborrow
let opts = altair_wf::default_activity_options();

let out = altair_wf::parallel(input, |step| {
    let opts = opts.clone();
    async move {
        ctx_ref
            .start_activity(EchoActivities::echo, step, opts)
            .await
            .map_err(|e| altair_wf::Error::Activity {
                activity: "EchoActivities::echo".into(),
                source: Box::new(e),
            })
    }
})
.await?;
```

`WorkflowContext::start_activity` takes `&self`, so reborrowing as
shared is sound — the closure cannot outlive the workflow scope.

## Prelude gotcha: `Result<T>` shadowing

The crate prelude re-exports `Result<T>` as `altair_wf::Result<T>` (a
1-arg alias). The Temporal SDK's `#[activity]` and `#[workflow_methods]`
macros expand to code containing `Result<T, ActivityError>` (two
arguments). If both are in scope at the same site, the prelude's alias
**swallows the second generic** and the compiler emits
`type alias takes 1 generic argument but 2 were supplied`.

The safe pattern, used by this crate's own integration tests:

```rust,no_run
// In a module that hosts #[activity] or #[workflow_methods]:
use altair_wf::{PipelineInput, PipelineOutput, pipeline, TaskInput, TaskOutput};
// (skip `use altair_wf::prelude::*;`)
```

The prelude itself is fine for code that doesn't host the SDK macros
(plain helper modules, examples, scripts).

## Plugging into a Temporal workflow

```rust,no_run
use altair_temporal::prelude::*;
use altair_wf::prelude::*;

#[workflow]
#[derive(Default)]
pub struct DeployWorkflow;

#[workflow_methods]
impl DeployWorkflow {
    #[run]
    pub async fn run(ctx: &mut WorkflowContext<Self>, input: PipelineInput<DeployStep>) -> WorkflowResult<PipelineOutput<DeployResult>> {
        let opts = altair_wf::default_activity_options();
        let result = pipeline(input, |step| async {
            // `ctx` isn't Send across all closures yet — see TODO in altair-temporal SDK
            // notes. For now, dispatch via the activity reference directly:
            ctx.start_activity(DeployActivities::run_step, step, opts.clone())
                .await
                .map_err(|e| altair_wf::Error::Activity {
                    activity: "DeployActivities::run_step".into(),
                    source: Box::new(e),
                })
        }).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}
```

## `function` feature — named-handler dispatch

When you have many small jobs whose differences are data-only, declaring a typed activity for each one becomes tedious. The `function` feature adds a thread-safe `Registry<String, Handler>` and a single Temporal activity (`FunctionActivities::execute_function`) that looks the handler up by name and runs it. Combine with the core patterns (`pipeline`, `parallel`) to dispatch a batch of named jobs.

```rust,no_run
# #[cfg(feature = "function")] {
use altair_wf::function::{FunctionInput, FunctionOutput, Registry};

# async fn ex() -> anyhow::Result<()> {
let mut reg = Registry::new();
reg.register("greet", |input: FunctionInput| async move {
    let who = input.args.get("name").cloned().unwrap_or_default();
    Ok::<_, std::io::Error>(FunctionOutput::with_result([
        ("msg".to_string(), format!("hello {who}"))
    ]))
})?;
# Ok(()) }
# }
```

Handler errors are reported as `FunctionExecutionOutput { success: false, ... }`, **not** as activity failures — so the pattern aggregations (`PipelineOutput::total_success` etc.) stay accurate. Infrastructure errors (validation, registry miss) become activity errors so Temporal can retry them.

## `datasync` feature — Source → Mapper → Sink (+ chunk)

Models data-sync jobs as a pipeline: a `Source<T>` produces records, a `Mapper<T, U>` transforms them, and a `Sink<U>` writes them. The in-process `Runner` drives one fetch-map-write cycle without Temporal; for production, wire the trio into a `#[workflow]` body.

```rust,no_run
# #[cfg(feature = "datasync")] {
use std::sync::Arc;
use altair_wf::datasync::{IdentityMapper, Runner, Sink, Source, WriteResult};
use async_trait::async_trait;

struct VecSource(Vec<i32>);
#[async_trait]
impl Source<i32> for VecSource {
    fn name(&self) -> &str { "vec" }
    async fn fetch(&self) -> altair_wf::Result<Vec<i32>> { Ok(self.0.clone()) }
}

struct CounterSink;
#[async_trait]
impl Sink<i32> for CounterSink {
    fn name(&self) -> &str { "counter" }
    async fn write(&self, records: Vec<i32>) -> altair_wf::Result<WriteResult> {
        Ok(WriteResult { inserted: records.len(), ..Default::default() })
    }
}

# async fn ex() -> altair_wf::Result<()> {
let runner: Runner<i32, i32> = Runner::new(
    Arc::new(VecSource(vec![1, 2, 3])),
    Arc::new(IdentityMapper::new()),
    Arc::new(CounterSink),
);
let out = runner.run().await?;
assert_eq!(out.total_fetched, 3);
# Ok(()) }
# }
```

### `datasync::chunk` — partitioned + resumable

For jobs whose record count would overflow a single Temporal history, the `chunk` submodule walks an ordered list of `Partition<K>` ranges, optionally remembers progress via a `ProgressTracker<K>`, and uses continue-as-new to hand the rest off to a fresh execution.

The `chunked_sync_run` helper is SDK-agnostic and takes async closures for each step. Inside a `#[workflow]` body the closures wrap activity calls; outside (tests, scripts) they can call services directly. The caller checks `result.deferred` and issues `continue_as_new` at the workflow boundary — see `crates/altair-wf/src/datasync/chunk/mod.rs` for a sketch.

## Validation

Every input carries a `validate()` method:

- `PipelineInput`, `ParallelInput`, `LoopInput`, `ParameterizedLoopInput`: non-empty tasks/items + each payload's own `TaskInput::validate`
- `DAGInput`: non-empty nodes, unique names, all dependencies exist, no cycles (DFS-based), each payload validates

Patterns call `validate()` at entry; failures surface as `Error::InvalidInput`.

## Defaults

`default_activity_options()` returns 10-minute start-to-close timeout with 3 retries, 1s → 60s exponential backoff (factor 2.0). Use it as a starting point and customize via the `altair_temporal::temporalio_sdk::ActivityOptions` builder.

## Error reference

| Variant | When |
|---|---|
| `Error::InvalidInput` | A `validate()` call rejected the input or pattern invariants (cycle, missing dep) |
| `Error::PatternStopped` | A step failed and the pattern was configured with `fail_fast` / `stop_on_error` |
| `Error::Activity` | The underlying SDK call failed (network, panic, timeout, retry exhaustion) — wrap from your `execute_one` closure when needed |

## Phase status

The Go [`go-wf`](https://github.com/jasoet/go-wf) port is shipped in three phases inside this crate:

- ✅ Phase 1 — core workflow patterns (single / pipeline / parallel / loop / DAG), shipped + deep-reviewed twice.
- ✅ Phase 2 — `function` module (registry + named-handler activity), shipped + deep-reviewed twice.
- ✅ Phase 3 — `datasync` core + `chunk` submodule (Source / Mapper / Sink + partitioned resumable orchestration). Some `chunk` extras (date-range adapter, rate-limit retry decorator, `InsertIfAbsentSink`) are deferred to follow-up PRs to keep the surface reviewable; track in [docs/porting-tracker.md](../../docs/porting-tracker.md).

## License

Apache-2.0
