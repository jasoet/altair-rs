//! `iterate_partitions` — pure-Rust partition walker for non-Temporal
//! callers (utilities, scripts, tests). Inside a Temporal workflow,
//! [`chunked_sync_run`](super::sync::chunked_sync_run) substitutes the
//! deterministic workflow clock for the sleep callback.

use std::future::Future;
use std::time::Duration;

use crate::datasync::chunk::partition::Partition;
use crate::error::Result;

/// Walk `parts` in order, awaiting `body(p)` for each. Between
/// successful partitions, await `sleeper(sleep)`. The sleep is skipped
/// after the last partition and when `sleep == Duration::ZERO` or
/// `sleeper` is `None`.
///
/// Iteration stops on the first error returned by `body`.
///
/// Intended for non-Temporal callers. Inside a Temporal workflow,
/// `chunked_sync_run` already does the equivalent with workflow-side
/// sleep so the deterministic clock is used.
pub async fn iterate_partitions<K, B, BFut, S, SFut>(
    parts: Vec<Partition<K>>,
    sleep: Duration,
    sleeper: Option<S>,
    mut body: B,
) -> Result<()>
where
    K: Send + Sync + 'static,
    B: FnMut(Partition<K>) -> BFut,
    BFut: Future<Output = Result<()>>,
    S: FnMut(Duration) -> SFut,
    SFut: Future<Output = ()>,
{
    let n = parts.len();
    let mut sleeper = sleeper;
    for (i, p) in parts.into_iter().enumerate() {
        body(p).await?;
        if i < n - 1
            && !sleep.is_zero()
            && let Some(s) = sleeper.as_mut()
        {
            s(sleep).await;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn iterates_in_order_without_sleeper() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = seen.clone();
        let parts = vec![Partition::new(0, 1), Partition::new(1, 2)];
        let body = move |p: Partition<i32>| {
            let captured = captured.clone();
            async move {
                captured.lock().unwrap().push(p.start);
                Ok(())
            }
        };
        // No sleeper at all.
        let sleeper: Option<fn(Duration) -> std::future::Ready<()>> = None;
        iterate_partitions(parts, Duration::ZERO, sleeper, body)
            .await
            .unwrap();
        assert_eq!(*seen.lock().unwrap(), vec![0, 1]);
    }

    #[tokio::test]
    async fn sleeps_between_but_not_after_last() {
        let calls = Arc::new(AtomicUsize::new(0));
        let cap_calls = calls.clone();
        let parts = vec![
            Partition::new(0, 1),
            Partition::new(1, 2),
            Partition::new(2, 3),
        ];
        let sleeper = move |_d: Duration| {
            let cap_calls = cap_calls.clone();
            async move {
                cap_calls.fetch_add(1, Ordering::SeqCst);
            }
        };
        iterate_partitions(parts, Duration::from_millis(1), Some(sleeper), |_p| async {
            Ok(())
        })
        .await
        .unwrap();
        // 3 partitions -> 2 sleeps (between each pair).
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn sleeper_provided_but_sleep_zero_is_short_circuited() {
        // Pins the `sleep.is_zero()` optimisation: even when a sleeper
        // is supplied, no calls are made when the configured sleep is
        // zero. Avoids a regression that would awake every iteration
        // unnecessarily.
        let calls = Arc::new(AtomicUsize::new(0));
        let cap = calls.clone();
        let parts = vec![Partition::new(0, 1), Partition::new(1, 2)];
        let sleeper = move |_d: Duration| {
            let cap = cap.clone();
            async move {
                cap.fetch_add(1, Ordering::SeqCst);
            }
        };
        iterate_partitions(parts, Duration::ZERO, Some(sleeper), |_p| async { Ok(()) })
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn body_error_short_circuits_iteration() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let cap = seen.clone();
        let parts = vec![
            Partition::new(0, 1),
            Partition::new(1, 2),
            Partition::new(2, 3),
        ];
        let body = move |p: Partition<i32>| {
            let cap = cap.clone();
            async move {
                cap.lock().unwrap().push(p.start);
                if p.start == 1 {
                    Err(crate::error::Error::InvalidInput("nope".into()))
                } else {
                    Ok(())
                }
            }
        };
        let sleeper: Option<fn(Duration) -> std::future::Ready<()>> = None;
        let res = iterate_partitions(parts, Duration::ZERO, sleeper, body).await;
        assert!(res.is_err());
        // Reached partitions 0 and 1, halted before 2.
        assert_eq!(*seen.lock().unwrap(), vec![0, 1]);
    }
}
