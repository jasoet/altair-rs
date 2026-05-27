//! The `retry` entry point.

use crate::config::Config;
use crate::error::{Error, PermanentError, Result};
use backon::{BackoffBuilder, ExponentialBuilder};
use std::future::Future;
use std::time::Instant;
use tracing::{Instrument, info_span};

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// Run `op` with retry per `config`.
///
/// On success, returns the value. On error, retries with exponential backoff.
/// If `op` returns an error that downcasts to [`PermanentError`], retry stops
/// immediately and the wrapped error is returned via [`Error::Permanent`].
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub async fn retry<T, E, F, Fut>(config: Config, mut op: F) -> Result<T>
where
    F: FnMut() -> Fut + Send,
    Fut: Future<Output = std::result::Result<T, E>> + Send,
    E: Into<BoxedError>,
    T: Send,
{
    let mut backoff_builder = ExponentialBuilder::default()
        .with_min_delay(config.initial_interval)
        .with_max_delay(config.max_interval)
        .with_factor(config.multiplier as f32)
        .with_max_times(config.max_retries as usize);
    if config.jitter {
        backoff_builder = backoff_builder.with_jitter();
    }
    let mut delays = backoff_builder.build();

    let start = Instant::now();
    let mut attempt: u32 = 0;

    let span = info_span!(
        "retry",
        retry.name = %config.name,
        retry.max_attempts = config.max_retries + 1,
    );

    async {
        loop {
            attempt += 1;
            let attempt_span = info_span!("retry.attempt", retry.attempt = attempt,);

            let outcome = op().instrument(attempt_span).await;

            match outcome {
                Ok(v) => {
                    tracing::info!(
                        retry.outcome = "success",
                        retry.attempts = attempt,
                        retry.elapsed_ms = start.elapsed().as_millis() as u64,
                        "retry succeeded",
                    );
                    return Ok(v);
                }
                Err(e) => {
                    let boxed: BoxedError = e.into();
                    if let Some(perm) = downcast_permanent(&boxed) {
                        tracing::warn!(
                            retry.outcome = "permanent",
                            retry.attempts = attempt,
                            "permanent error encountered",
                        );
                        return Err(Error::Permanent {
                            name: config.name.clone(),
                            source: perm,
                        });
                    }

                    if let Some(delay) = delays.next() {
                        tracing::debug!(
                            retry.attempt = attempt,
                            retry.delay_ms = delay.as_millis() as u64,
                            "retrying after backoff",
                        );
                        tokio::time::sleep(delay).await;
                    } else {
                        tracing::warn!(
                            retry.outcome = "exhausted",
                            retry.attempts = attempt,
                            "retry attempts exhausted",
                        );
                        return Err(Error::Exhausted {
                            name: config.name.clone(),
                            attempts: attempt,
                            source: boxed,
                        });
                    }
                }
            }
        }
    }
    .instrument(span)
    .await
}

fn downcast_permanent(e: &BoxedError) -> Option<BoxedError> {
    // PermanentError wraps an underlying error; unwrap so callers see the original
    e.downcast_ref::<PermanentError>()
        .map(|p| format!("{p}").into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    fn fast_config(name: &str) -> Config {
        Config::builder()
            .name(name)
            .max_retries(3)
            .initial_interval(Duration::from_millis(1))
            .max_interval(Duration::from_millis(10))
            .jitter(false)
            .build()
    }

    #[tokio::test]
    async fn succeeds_on_first_try() {
        let r: Result<u32> =
            retry(fast_config("ok"), || async { Ok::<_, std::io::Error>(42) }).await;
        assert_eq!(r.unwrap(), 42);
    }

    #[tokio::test]
    async fn retries_then_succeeds() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let r: Result<u32> = retry(fast_config("flaky"), move || {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst) + 1;
                if n < 3 {
                    Err::<u32, _>(std::io::Error::other("flake"))
                } else {
                    Ok(n)
                }
            }
        })
        .await;
        assert_eq!(r.unwrap(), 3);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn exhausts_with_attempts_count() {
        let r: Result<u32> = retry(fast_config("always_fail"), || async {
            Err::<u32, _>(std::io::Error::other("nope"))
        })
        .await;
        match r {
            Err(Error::Exhausted { attempts, .. }) => assert_eq!(attempts, 4),
            other => panic!("expected Exhausted, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn permanent_error_short_circuits() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let r: Result<u32> = retry(fast_config("permanent"), move || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err::<u32, _>(PermanentError::wrap("invalid creds"))
            }
        })
        .await;
        assert!(matches!(r, Err(Error::Permanent { .. })));
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "should not retry on permanent"
        );
    }
}
