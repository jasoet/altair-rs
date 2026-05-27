# altair-concurrent

Type-safe parallel execution of named async tasks with cancellation, timeout, and per-task tracing.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Add to your project

```bash
cargo add altair-concurrent
```

`tokio` and `tokio-util` types you need are re-exported — you do not have to add them yourself.

## Quick start — parallel fetch with timeout

```rust,no_run
use altair_concurrent::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> altair_concurrent::Result<()> {
    let tasks: TaskMap<String> = TaskMap::new()
        .insert("fetch_user", |_| async { Ok::<_, std::io::Error>("alice".into()) })
        .insert("fetch_orders", |_| async { Ok::<_, std::io::Error>("3 open".into()) });

    let results = execute_concurrently(tasks)
        .with_timeout(Duration::from_secs(5))
        .await?;

    println!("user: {}", results["fetch_user"]);
    println!("orders: {}", results["fetch_orders"]);
    Ok(())
}
```

`results` is a `HashMap<&'static str, T>` — look tasks up by the name you gave them at `insert`.

## Fail-fast (default) — first error cancels the rest

```rust,no_run
use altair_concurrent::prelude::*;

# async fn run() -> altair_concurrent::Result<()> {
let tasks: TaskMap<u32> = TaskMap::new()
    .insert("ok", |_| async { Ok::<_, std::io::Error>(1) })
    .insert("bad", |_| async {
        Err::<u32, _>(std::io::Error::other("nope"))
    });

match execute_concurrently(tasks).await {
    Ok(map) => println!("all succeeded: {map:?}"),
    Err(Error::TaskFailed { name, source }) => {
        eprintln!("task '{name}' failed: {source}");
        // remaining tasks already cancelled
    }
    Err(e) => eprintln!("infrastructure error: {e}"),
}
# Ok(()) }
```

## Partial results — run everything, inspect each task

When you want **every** task to run regardless of failures and see each one's outcome:

```rust,no_run
use altair_concurrent::prelude::*;

# async fn run() -> altair_concurrent::Result<()> {
let tasks: TaskMap<u32> = TaskMap::new()
    .insert("a", |_| async { Ok::<_, std::io::Error>(1) })
    .insert("b", |_| async { Err::<u32, _>(std::io::Error::other("transient")) })
    .insert("c", |_| async { Ok::<_, std::io::Error>(3) });

// PartialResults<u32> = HashMap<&'static str, Result<u32, BoxedError>>
let results: PartialResults<u32> = execute_concurrently(tasks)
    .with_partial_results()
    .await?;

for (name, outcome) in &results {
    match outcome {
        Ok(v) => println!("{name} -> {v}"),
        Err(e) => eprintln!("{name} -> failed: {e}"),
    }
}
# Ok(()) }
```

Only infrastructure errors (Timeout / Cancelled / Join) bubble up through the outer `Result`; task failures live in the inner map values.

## External cancellation

```rust,no_run
use altair_concurrent::prelude::*;
use std::time::Duration;

# async fn run() -> altair_concurrent::Result<()> {
let cancel = CancellationToken::new();
let tasks: TaskMap<()> = TaskMap::new().insert("worker", |ct| async move {
    // Tasks receive the same token — make them cooperative.
    tokio::select! {
        () = ct.cancelled() => Err::<(), _>(std::io::Error::other("cancelled")),
        () = tokio::time::sleep(Duration::from_secs(30)) => Ok(()),
    }
});

let handle = tokio::spawn({
    let cancel = cancel.clone();
    async move { execute_concurrently(tasks).with_cancellation(cancel).await }
});

// Some condition triggers cancel from elsewhere (signal handler, timeout, etc.)
tokio::time::sleep(Duration::from_millis(50)).await;
cancel.cancel();

let _ = handle.await;
# Ok(()) }
```

## Features

- **Named tasks** — `HashMap<&'static str, T>` results, not positional tuples
- **Tracing** — each task runs inside `tracing::info_span!("concurrent.task", task.name = ...)`; aggregate batch span shows `task_count`
- **Cancellation** — pass a `CancellationToken` via `.with_cancellation(...)`; tasks receive their own clone so they can cooperate
- **Timeout** — `.with_timeout(Duration)`; expiry cancels remaining tasks and returns `Error::Timeout`
- **Fail-fast or partial** — `Executor` (default) returns the first error; `PartialExecutor` (via `.with_partial_results()`) returns every task's outcome

## Constraints

- All tasks in a single batch must return the same `Result<T, E>`. For heterogeneous batches, use `tokio::join!` directly.
- Built on `tokio::task::JoinSet`; tokio is the only supported runtime.

## License

[MIT](../../LICENSE)
