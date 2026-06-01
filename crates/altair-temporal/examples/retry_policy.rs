//! Demonstrate the [`RetryPolicy`] builder.
//!
//! Run with: `cargo run -p altair-temporal --example retry_policy`

use std::time::Duration;

use altair_temporal::prelude::*;

fn main() -> altair_temporal::Result<()> {
    let policy = RetryPolicy::builder()
        .initial_interval(Duration::from_secs(1))
        .maximum_interval(Duration::from_mins(1))
        .backoff_coefficient(2.0)
        .max_attempts(5)
        .non_retryable("AuthError")
        .non_retryable("ValidationError")
        .build()?;
    let inner = policy.into_inner();
    println!(
        "policy: max_attempts={} backoff={:.1} non_retryable={:?}",
        inner.maximum_attempts, inner.backoff_coefficient, inner.non_retryable_error_types
    );
    Ok(())
}
