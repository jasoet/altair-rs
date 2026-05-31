//! End-to-end behaviour tests against a flaky in-process operation.
//!
//! These tests exercise the public surface — `retry`, `Config::builder`,
//! `PermanentError`, `CancellationToken` — the way a consumer would,
//! avoiding any internals.

use altair_retry::prelude::*;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// Fast retry config so tests don't slow CI down.
fn fast(name: &str) -> Config {
    Config::builder()
        .name(name)
        .max_retries(5)
        .initial_interval(Duration::from_millis(1))
        .max_interval(Duration::from_millis(20))
        .jitter(false)
        .build()
}

#[tokio::test]
async fn flaky_op_eventually_succeeds() {
    let attempts = Arc::new(AtomicU32::new(0));
    let attempts_clone = attempts.clone();

    let result: Result<u32> = retry(fast("flaky-eventually"), move || {
        let a = attempts_clone.clone();
        async move {
            let n = a.fetch_add(1, Ordering::SeqCst) + 1;
            if n < 4 {
                Err::<u32, _>(std::io::Error::other(format!("transient #{n}")))
            } else {
                Ok(n)
            }
        }
    })
    .await;

    assert_eq!(result.unwrap(), 4);
    assert_eq!(attempts.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn always_failing_op_returns_exhausted() {
    let attempts = Arc::new(AtomicU32::new(0));
    let attempts_clone = attempts.clone();

    let result: Result<u32> = retry(fast("always-fails"), move || {
        let a = attempts_clone.clone();
        async move {
            a.fetch_add(1, Ordering::SeqCst);
            Err::<u32, _>(std::io::Error::other("always broken"))
        }
    })
    .await;

    match result {
        Err(Error::Exhausted { attempts: n, .. }) => {
            assert_eq!(n, 6, "1 initial + 5 retries = 6 attempts");
        }
        other => panic!("expected Exhausted, got {other:?}"),
    }
    assert_eq!(attempts.load(Ordering::SeqCst), 6);
}

#[tokio::test]
async fn permanent_error_short_circuits_on_first_attempt() {
    let attempts = Arc::new(AtomicU32::new(0));
    let attempts_clone = attempts.clone();

    let result: Result<u32> = retry(fast("permanent"), move || {
        let a = attempts_clone.clone();
        async move {
            a.fetch_add(1, Ordering::SeqCst);
            Err::<u32, _>(PermanentError::wrap("auth rejected"))
        }
    })
    .await;

    assert!(
        matches!(result, Err(Error::Permanent { .. })),
        "expected Permanent, got {result:?}"
    );
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        1,
        "should not retry permanent errors",
    );
}

#[tokio::test]
async fn permanent_error_preserves_source_chain() {
    // The fix in v0.1.4 ensured PermanentError extraction moved the inner
    // boxed error rather than stringifying it via Display. Verify the
    // source chain is intact after retry returns.
    #[derive(Debug, thiserror::Error)]
    #[error("auth: {0}")]
    struct AuthError(String);

    let original = AuthError("token expired".to_string());
    let original_display = original.to_string();

    let result: Result<()> = retry(fast("preserves-source"), move || {
        let msg = original_display.clone();
        async move { Err::<(), _>(PermanentError::wrap(AuthError(msg))) }
    })
    .await;

    let Err(Error::Permanent { source, .. }) = result else {
        panic!("expected Permanent, got something else");
    };
    // Walking source must produce the AuthError message, not a generic
    // String wrapper.
    let rendered = source.to_string();
    assert!(
        rendered.contains("token expired"),
        "source must preserve the original error message; got {rendered:?}",
    );
}

#[tokio::test]
async fn cancellation_during_backoff_returns_cancelled() {
    let token = CancellationToken::new();
    let cfg = Config::builder()
        .name("backoff-cancel")
        .max_retries(10)
        .initial_interval(Duration::from_millis(100))
        .max_interval(Duration::from_secs(2))
        .jitter(false)
        .cancellation_token(token.clone())
        .build();

    // Cancel shortly after the first retry's backoff starts.
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        token.cancel();
    });

    let result: Result<u32> = retry(cfg, || async {
        Err::<u32, _>(std::io::Error::other("transient"))
    })
    .await;

    assert!(
        matches!(result, Err(Error::Cancelled { .. })),
        "expected Cancelled, got {result:?}",
    );
}

#[tokio::test]
async fn pre_cancelled_token_aborts_before_first_attempt() {
    let token = CancellationToken::new();
    token.cancel();
    let cfg = Config::builder()
        .name("pre-cancelled")
        .max_retries(3)
        .initial_interval(Duration::from_millis(1))
        .jitter(false)
        .cancellation_token(token)
        .build();

    let attempts = Arc::new(AtomicU32::new(0));
    let attempts_clone = attempts.clone();
    let result: Result<u32> = retry(cfg, move || {
        let a = attempts_clone.clone();
        async move {
            a.fetch_add(1, Ordering::SeqCst);
            Ok::<u32, std::io::Error>(0)
        }
    })
    .await;

    assert!(matches!(result, Err(Error::Cancelled { .. })));
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        0,
        "first attempt must not run when token is pre-cancelled",
    );
}
