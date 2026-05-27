# altair-concurrent

Type-safe parallel execution of named async tasks with cancellation, timeout, and per-task tracing.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace — see [porting tracker](../../docs/porting-tracker.md) for status of other crates.

## Quick start

```rust,no_run
use altair_concurrent::{execute_concurrently, TaskMap};

# async fn run() -> altair_concurrent::Result<()> {
let tasks: TaskMap<String> = TaskMap::new()
    .insert("fetch_user", |_ctx| async { fetch_user(42).await })
    .insert("fetch_orders", |_ctx| async { fetch_orders(42).await });

let results = execute_concurrently(tasks).await?;
assert!(results.contains_key("fetch_user"));
# Ok(()) }
# async fn fetch_user(_: u64) -> Result<String, std::io::Error> { Ok("u".into()) }
# async fn fetch_orders(_: u64) -> Result<String, std::io::Error> { Ok("o".into()) }
```

## License

MIT
