# altair-concurrent

Type-safe parallel execution of named async tasks with cancellation, timeout, and per-task tracing.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Add to your project

```bash
cargo add altair-concurrent
```

## Quick start

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

    println!("{:?}", results);
    Ok(())
}
```

## Features

- **Named tasks** — `HashMap<&'static str, T>` results, not positional tuples
- **Tracing** — each task runs inside a `tracing::info_span!("concurrent.task", task.name = ...)` so it shows up as a separate node in distributed traces
- **Cancellation** — pass a `CancellationToken`; cancelling it aborts all tasks
- **Timeout** — `.with_timeout(Duration)`; expires cancel remaining tasks
- **Fail-fast or partial** — by default, the first error cancels remaining tasks; `with_partial_results()` switches to "run all, surface first error" semantics

## Constraints

- All tasks must return the same `Result<T, E>`. For heterogeneous batches, use `tokio::join!` directly.
- Built on `tokio::task::JoinSet`; tokio is the only supported runtime.

## License

[MIT](../../LICENSE)
