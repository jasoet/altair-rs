# altair-wf

[![crates.io](https://img.shields.io/crates/v/altair-wf.svg)](https://crates.io/crates/altair-wf)

Reusable Temporal workflow patterns: single-task, pipeline (sequential), parallel, loop, parameterized loop, and DAG with cycle detection. Built on [altair-temporal](https://crates.io/crates/altair-temporal).

Spiritual port of the `workflow` module in [`github.com/jasoet/go-wf`](https://github.com/jasoet/go-wf).

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Why

Most real-world Temporal workflows are one of a handful of shapes: do a single thing; do A then B then C; fan out N copies of the same activity; or run a DAG of activities. `altair-wf` ships those shapes as composable async helpers so you can drop them into your own `#[workflow]` definitions without re-implementing the orchestration logic each time.

The patterns are **SDK-agnostic** — each takes an `execute_one` closure that you wire to `WorkflowContext::start_activity` inside your workflow's `run` method. This keeps the orchestration pure (unit-testable without Temporal) and lets your code own all activity dispatch.

## Install

```toml
[dependencies]
altair-wf = "0.1"
altair-temporal = "0.2"
# Required by the Temporal SDK's #[workflow] / #[activities] macros:
futures = "0.3"
futures-util = "0.3"
```

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
| `execute` / `execute_with_timeout` / `pipeline` | `FnMut(I) -> Future<O>` | Closure runs one at a time; mutable state allowed. |
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

## License

Apache-2.0
