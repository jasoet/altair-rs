# altair-wf examples

Runnable end-to-end examples covering every feature surface of
`altair-wf`. Each example is a single file you can copy and adapt.

## Prerequisites

All examples that mention "real Temporal" need a server reachable at
`http://localhost:7233` with the `default` namespace. The easiest path
is the Temporal CLI dev server:

```bash
# Install once
brew install temporal

# Start
temporal server start-dev
```

The Web UI is then at <http://localhost:8233>.

The in-process `datasync_runner` example needs **nothing** — it drives
an in-memory pipeline with no Temporal coupling.

## Examples

| # | File | Feature | Needs Temporal | Pattern |
|---|---|---|---|---|
| 1 | [`hello_execute.rs`](hello_execute.rs) | — | ✅ | `execute` — single task |
| 2 | [`hello_pipeline.rs`](hello_pipeline.rs) | — | ✅ | `pipeline` — sequential |
| 3 | [`hello_parallel.rs`](hello_parallel.rs) | — | ✅ | `parallel` + `FailureStrategy` |
| 4 | [`hello_loop.rs`](hello_loop.rs) | — | ✅ | `run_loop` with substitutor |
| 5 | [`hello_parameterized_loop.rs`](hello_parameterized_loop.rs) | — | ✅ | `parameterized_loop` — cartesian product |
| 6 | [`hello_dag.rs`](hello_dag.rs) | — | ✅ | `run_dag` — diamond shape, topological layers |
| 7 | [`function_registry.rs`](function_registry.rs) | `function` | ✅ | named-handler dispatch + pipeline |
| 8 | [`datasync_runner.rs`](datasync_runner.rs) | `datasync` | ❌ | in-process `Source → Mapper → Sink` |
| 9 | [`datasync_chunked.rs`](datasync_chunked.rs) | `datasync` | ✅ | partitioned + cursor + continue-as-new |

## Running

The default-feature ones:

```bash
cargo run -p altair-wf --example hello_execute
cargo run -p altair-wf --example hello_pipeline
cargo run -p altair-wf --example hello_parallel
cargo run -p altair-wf --example hello_loop
cargo run -p altair-wf --example hello_parameterized_loop
cargo run -p altair-wf --example hello_dag
```

The feature-gated ones:

```bash
cargo run -p altair-wf --features function --example function_registry
cargo run -p altair-wf --features datasync --example datasync_runner
cargo run -p altair-wf --features datasync --example datasync_chunked
```

## Shape

Each Temporal-backed example follows the same skeleton:

1. Define payloads (`MyIn`, `MyOut`) + impl `TaskInput` / `TaskOutput`.
2. Define a `MyActivities` struct + `#[activities]` impl.
3. Define a `MyWf` workflow type + `#[workflow_methods]` impl whose
   `#[run]` body calls into the relevant `altair_wf::*` helper.
4. In `main`: build a `Config`, build a worker, spawn it on a
   background task with a oneshot shutdown channel, connect a
   `Client`, start the workflow, await its result, then signal
   shutdown and join the worker.

That last step (graceful shutdown + join) is what separates these from
the integration-tests harness — useful template for production worker
binaries.
