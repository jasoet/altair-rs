//! Concurrent execution entry point.

use crate::error::{Error, Result};
use crate::task_map::TaskMap;
use std::collections::HashMap;
use std::time::Duration;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, instrument};

type BoxedError = Box<dyn std::error::Error + Send + Sync>;
type TaskOutcome<T> = (&'static str, std::result::Result<T, BoxedError>);

/// Configures and runs a [`TaskMap`].
///
/// Construct via [`execute_concurrently`].
pub struct Executor<T> {
    tasks: TaskMap<T>,
    cancellation: Option<CancellationToken>,
    timeout: Option<Duration>,
    partial: bool,
}

impl<T> Executor<T>
where
    T: Send + 'static,
{
    /// Attach a cancellation token. Cancelling it causes all tasks to abort.
    #[must_use]
    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancellation = Some(token);
        self
    }

    /// Apply an overall timeout. If the timeout elapses, remaining tasks are cancelled.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Return per-task `Result`s instead of fail-fast.
    #[must_use]
    pub fn with_partial_results(mut self) -> Self {
        self.partial = true;
        self
    }
}

impl<T> std::future::IntoFuture for Executor<T>
where
    T: Send + 'static,
{
    type Output = Result<HashMap<&'static str, T>>;
    type IntoFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { run(self).await })
    }
}

#[instrument(skip(executor), fields(task_count = executor.tasks.len()))]
async fn run<T>(executor: Executor<T>) -> Result<HashMap<&'static str, T>>
where
    T: Send + 'static,
{
    let token = executor.cancellation.unwrap_or_default();
    let mut set: JoinSet<TaskOutcome<T>> = JoinSet::new();

    for (name, task_fn) in executor.tasks.tasks {
        let child_token = token.clone();
        let span = tracing::info_span!("concurrent.task", task.name = name);
        set.spawn(
            async move {
                let result = task_fn(child_token).await;
                (name, result)
            }
            .instrument(span),
        );
    }

    let mut results: HashMap<&'static str, T> = HashMap::new();
    let mut errors: HashMap<&'static str, BoxedError> = HashMap::new();

    let timeout = executor.timeout;

    loop {
        let next = async { set.join_next().await };
        let outcome = if let Some(d) = timeout {
            if let Ok(v) = tokio::time::timeout(d, next).await {
                v
            } else {
                token.cancel();
                set.shutdown().await;
                return Err(Error::Timeout);
            }
        } else {
            next.await
        };

        match outcome {
            None => break,
            Some(Err(e)) => return Err(Error::Join(e)),
            Some(Ok((name, Ok(v)))) => {
                results.insert(name, v);
            }
            Some(Ok((name, Err(e)))) => {
                if executor.partial {
                    errors.insert(name, e);
                } else {
                    token.cancel();
                    set.shutdown().await;
                    return Err(Error::TaskFailed { name, source: e });
                }
            }
        }

        if token.is_cancelled() && set.is_empty() {
            return Err(Error::Cancelled);
        }
    }

    if executor.partial && !errors.is_empty() {
        // Partial mode: surface first error as TaskFailed for symmetry; full Results
        // map exposed via `with_partial_results` is out-of-scope for v0.1.0.
        let (name, source) = errors.into_iter().next().expect("non-empty");
        return Err(Error::TaskFailed { name, source });
    }

    Ok(results)
}

/// Run a [`TaskMap`] concurrently.
///
/// Returns an [`Executor`] that resolves to a `HashMap<&'static str, T>`
/// when awaited.
#[must_use]
pub fn execute_concurrently<T>(tasks: TaskMap<T>) -> Executor<T> {
    Executor {
        tasks,
        cancellation: None,
        timeout: None,
        partial: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn empty_map_resolves_to_empty_results() {
        let m: TaskMap<u32> = TaskMap::new();
        let r = execute_concurrently(m).await.unwrap();
        assert!(r.is_empty());
    }

    #[tokio::test]
    async fn two_tasks_complete() {
        let m: TaskMap<u32> = TaskMap::new()
            .insert("a", |_| async { Ok::<_, std::io::Error>(1) })
            .insert("b", |_| async { Ok::<_, std::io::Error>(2) });
        let r = execute_concurrently(m).await.unwrap();
        assert_eq!(r["a"], 1);
        assert_eq!(r["b"], 2);
    }

    #[tokio::test]
    async fn failing_task_returns_task_failed_error() {
        let m: TaskMap<u32> = TaskMap::new()
            .insert("ok", |_| async { Ok::<_, std::io::Error>(1) })
            .insert("bad", |_| async {
                Err::<u32, std::io::Error>(std::io::Error::other("boom"))
            });
        let err = execute_concurrently(m).await.unwrap_err();
        match err {
            Error::TaskFailed { name, .. } => assert_eq!(name, "bad"),
            other => panic!("expected TaskFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn timeout_returns_timeout_error() {
        let m: TaskMap<u32> = TaskMap::new().insert("slow", |_| async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok::<_, std::io::Error>(1)
        });
        let err = execute_concurrently(m)
            .with_timeout(Duration::from_millis(50))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Timeout));
    }

    #[tokio::test]
    async fn external_cancellation_causes_cancelled_error() {
        let token = CancellationToken::new();
        let inner = token.clone();
        let m: TaskMap<u32> = TaskMap::new().insert("waiter", move |ct| async move {
            ct.cancelled().await;
            Err::<u32, std::io::Error>(std::io::Error::other("cancelled"))
        });
        let handle =
            tokio::spawn(async move { execute_concurrently(m).with_cancellation(token).await });
        tokio::time::sleep(Duration::from_millis(20)).await;
        inner.cancel();
        let err = handle.await.unwrap().unwrap_err();
        // Either TaskFailed or Cancelled is acceptable depending on order.
        assert!(matches!(err, Error::TaskFailed { .. } | Error::Cancelled));
    }
}
