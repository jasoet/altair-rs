//! Concurrent execution entry point.

use crate::error::{Error, Result};
use crate::task_map::TaskMap;
use std::collections::HashMap;
use std::time::Duration;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, instrument};

/// Boxed task error returned in partial-results mode.
pub type BoxedError = Box<dyn std::error::Error + Send + Sync>;
type TaskOutcome<T> = (&'static str, std::result::Result<T, BoxedError>);

/// Per-task result map returned by [`PartialExecutor`].
pub type PartialResults<T> = HashMap<&'static str, std::result::Result<T, BoxedError>>;

/// Fail-fast executor. Constructed by [`execute_concurrently`].
///
/// On the first task error: cancels the rest and returns [`Error::TaskFailed`].
pub struct Executor<T> {
    tasks: TaskMap<T>,
    cancellation: Option<CancellationToken>,
    timeout: Option<Duration>,
}

impl<T> Executor<T>
where
    T: Send + 'static,
{
    /// Attach a cancellation token. Cancelling it requests all running
    /// tasks to stop.
    ///
    /// # Cooperative cancellation
    ///
    /// Each task receives the token via the closure argument and is
    /// responsible for `.cancelled().await`-ing it. Tasks that ignore
    /// the token will not be interrupted — `JoinSet::shutdown` aborts
    /// their `JoinHandle`s, but a CPU-bound task that never yields cannot
    /// be preempted. Design tasks to check the token at await points.
    #[must_use]
    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancellation = Some(token);
        self
    }

    /// Apply an overall timeout. If the timeout elapses, [`Error::Timeout`]
    /// is returned and the internal cancellation token is signalled so
    /// remaining tasks observe cancellation. Tasks that ignore the token
    /// continue running until they yield (see [`Self::with_cancellation`]).
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Switch to partial-results mode: every task is awaited; each task's
    /// `Result` appears in the returned [`PartialResults`] map.
    ///
    /// The outer [`Result`] still reports infrastructure errors ([`Error::Timeout`],
    /// [`Error::Cancelled`], [`Error::Join`]).
    #[must_use]
    pub fn with_partial_results(self) -> PartialExecutor<T> {
        PartialExecutor {
            tasks: self.tasks,
            cancellation: self.cancellation,
            timeout: self.timeout,
        }
    }
}

impl<T> std::future::IntoFuture for Executor<T>
where
    T: Send + 'static,
{
    type Output = Result<HashMap<&'static str, T>>;
    type IntoFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { run_fail_fast(self).await })
    }
}

/// Partial-results executor — every task runs to completion; per-task
/// success/failure exposed in the returned map.
///
/// Constructed via [`Executor::with_partial_results`].
pub struct PartialExecutor<T> {
    tasks: TaskMap<T>,
    cancellation: Option<CancellationToken>,
    timeout: Option<Duration>,
}

impl<T> PartialExecutor<T>
where
    T: Send + 'static,
{
    /// Attach a cancellation token. Cancelling it causes all tasks to abort.
    #[must_use]
    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancellation = Some(token);
        self
    }

    /// Apply an overall timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

impl<T> std::future::IntoFuture for PartialExecutor<T>
where
    T: Send + 'static,
{
    type Output = Result<PartialResults<T>>;
    type IntoFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { run_partial(self).await })
    }
}

fn spawn_tasks<T>(tasks: TaskMap<T>, token: &CancellationToken) -> JoinSet<TaskOutcome<T>>
where
    T: Send + 'static,
{
    let mut set: JoinSet<TaskOutcome<T>> = JoinSet::new();
    for (name, task_fn) in tasks.tasks {
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
    set
}

#[instrument(skip(executor), fields(task_count = executor.tasks.len()))]
async fn run_fail_fast<T>(executor: Executor<T>) -> Result<HashMap<&'static str, T>>
where
    T: Send + 'static,
{
    let token = executor.cancellation.unwrap_or_default();
    let mut set = spawn_tasks(executor.tasks, &token);
    let mut results: HashMap<&'static str, T> = HashMap::new();
    let timeout = executor.timeout;

    loop {
        let outcome = next_outcome(&mut set, &token, timeout).await?;
        match outcome {
            None => break,
            Some((name, Ok(v))) => {
                results.insert(name, v);
            }
            Some((name, Err(e))) => {
                token.cancel();
                set.shutdown().await;
                return Err(Error::TaskFailed { name, source: e });
            }
        }
        if token.is_cancelled() && set.is_empty() {
            return Err(Error::Cancelled);
        }
    }

    Ok(results)
}

#[instrument(skip(executor), fields(task_count = executor.tasks.len()))]
async fn run_partial<T>(executor: PartialExecutor<T>) -> Result<PartialResults<T>>
where
    T: Send + 'static,
{
    let token = executor.cancellation.unwrap_or_default();
    let mut set = spawn_tasks(executor.tasks, &token);
    let mut results: PartialResults<T> = HashMap::new();
    let timeout = executor.timeout;

    loop {
        let outcome = next_outcome(&mut set, &token, timeout).await?;
        match outcome {
            None => break,
            Some((name, result)) => {
                results.insert(name, result);
            }
        }
    }

    Ok(results)
}

async fn next_outcome<T>(
    set: &mut JoinSet<TaskOutcome<T>>,
    token: &CancellationToken,
    timeout: Option<Duration>,
) -> Result<Option<TaskOutcome<T>>>
where
    T: Send + 'static,
{
    let next = async { set.join_next().await };
    let raw = if let Some(d) = timeout {
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

    match raw {
        None => Ok(None),
        Some(Ok(outcome)) => Ok(Some(outcome)),
        Some(Err(e)) => Err(Error::Join(e)),
    }
}

/// Run a [`TaskMap`] concurrently in fail-fast mode.
///
/// Returns an [`Executor`] that resolves to a `HashMap<&'static str, T>` when awaited.
/// Call [`Executor::with_partial_results`] to switch to "run all, return per-task results" mode.
#[must_use]
pub fn execute_concurrently<T>(tasks: TaskMap<T>) -> Executor<T> {
    Executor {
        tasks,
        cancellation: None,
        timeout: None,
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

    #[tokio::test]
    async fn partial_results_returns_per_task_results() {
        let m: TaskMap<u32> = TaskMap::new()
            .insert("ok", |_| async { Ok::<_, std::io::Error>(1) })
            .insert("bad", |_| async {
                Err::<u32, std::io::Error>(std::io::Error::other("boom"))
            })
            .insert("also_ok", |_| async { Ok::<_, std::io::Error>(2) });
        let r = execute_concurrently(m)
            .with_partial_results()
            .await
            .unwrap();
        assert_eq!(r.len(), 3);
        assert!(r["ok"].is_ok());
        assert!(r["bad"].is_err());
        assert!(r["also_ok"].is_ok());
    }

    #[tokio::test]
    async fn partial_timeout_still_propagates() {
        let m: TaskMap<u32> = TaskMap::new().insert("slow", |_| async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok::<_, std::io::Error>(1)
        });
        let err = execute_concurrently(m)
            .with_partial_results()
            .with_timeout(Duration::from_millis(20))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Timeout));
    }
}
