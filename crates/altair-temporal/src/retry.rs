//! Retry-policy builder over the SDK's proto `RetryPolicy`.

use std::time::Duration;

use crate::error::{Error, Result};

/// A Temporal `RetryPolicy` ready to plug into `ActivityOptions`.
///
/// Constructed via [`RetryPolicy::builder`]; converted to the SDK type
/// via [`RetryPolicy::into_inner`].
#[derive(Debug, Clone)]
pub struct RetryPolicy(temporalio_common::protos::temporal::api::common::v1::RetryPolicy);

impl RetryPolicy {
    /// Start building a retry policy.
    #[must_use]
    pub fn builder() -> RetryPolicyBuilder {
        RetryPolicyBuilder::default()
    }

    /// Yield the underlying proto type for SDK calls.
    #[must_use]
    pub fn into_inner(self) -> temporalio_common::protos::temporal::api::common::v1::RetryPolicy {
        self.0
    }
}

/// Builder for [`RetryPolicy`].
#[derive(Debug, Clone)]
pub struct RetryPolicyBuilder {
    initial_interval: Duration,
    maximum_interval: Duration,
    backoff_coefficient: f64,
    max_attempts: u32,
    non_retryable_error_types: Vec<String>,
}

impl Default for RetryPolicyBuilder {
    fn default() -> Self {
        Self {
            initial_interval: Duration::from_secs(1),
            maximum_interval: Duration::from_secs(30),
            backoff_coefficient: 2.0,
            max_attempts: 0,
            non_retryable_error_types: Vec::new(),
        }
    }
}

impl RetryPolicyBuilder {
    /// Initial backoff interval (default `1s`).
    #[must_use]
    pub fn initial_interval(mut self, d: Duration) -> Self {
        self.initial_interval = d;
        self
    }

    /// Maximum backoff interval (default `30s`).
    #[must_use]
    pub fn maximum_interval(mut self, d: Duration) -> Self {
        self.maximum_interval = d;
        self
    }

    /// Exponential backoff multiplier (default `2.0`).
    #[must_use]
    pub fn backoff_coefficient(mut self, c: f64) -> Self {
        self.backoff_coefficient = c;
        self
    }

    /// Maximum number of attempts. `0` = unlimited (Temporal convention).
    #[must_use]
    pub fn max_attempts(mut self, n: u32) -> Self {
        self.max_attempts = n;
        self
    }

    /// Append an error type name that should never be retried.
    ///
    /// Matched against the `type` field of `ApplicationFailure` at runtime.
    /// Call repeatedly to add more.
    #[must_use]
    pub fn non_retryable(mut self, error_type: impl Into<String>) -> Self {
        self.non_retryable_error_types.push(error_type.into());
        self
    }

    /// Finalise into a [`RetryPolicy`].
    ///
    /// Returns [`Error::Configuration`] when any of these invariants
    /// are violated, since they'd produce a policy the Temporal server
    /// rejects at runtime:
    ///
    /// - `backoff_coefficient` must be finite and `> 0.0`
    /// - `initial_interval` and `maximum_interval` must be non-zero
    /// - `initial_interval <= maximum_interval`
    ///
    /// Earlier versions of this method panicked instead. That was a
    /// production hazard: the Temporal SDK macros expand to code that
    /// calls `.build()` inside the workflow body, so a panic there
    /// crashed the workflow task and the server would retry it
    /// indefinitely. `Result` lets the caller bubble validation
    /// failures up to startup instead.
    ///
    /// # Errors
    ///
    /// See above.
    pub fn build(self) -> Result<RetryPolicy> {
        use temporalio_common::protos::temporal::api::common::v1::RetryPolicy as Proto;
        if !(self.backoff_coefficient.is_finite() && self.backoff_coefficient > 0.0) {
            return Err(Error::Configuration(format!(
                "backoff_coefficient must be finite and > 0.0, got {}",
                self.backoff_coefficient,
            )));
        }
        if self.initial_interval.is_zero() {
            return Err(Error::Configuration(
                "initial_interval must be > 0".to_string(),
            ));
        }
        if self.maximum_interval.is_zero() {
            return Err(Error::Configuration(
                "maximum_interval must be > 0".to_string(),
            ));
        }
        if self.initial_interval > self.maximum_interval {
            return Err(Error::Configuration(format!(
                "initial_interval ({:?}) must be <= maximum_interval ({:?})",
                self.initial_interval, self.maximum_interval,
            )));
        }
        Ok(RetryPolicy(Proto {
            initial_interval: Some(duration_to_proto(self.initial_interval)),
            backoff_coefficient: self.backoff_coefficient,
            maximum_interval: Some(duration_to_proto(self.maximum_interval)),
            maximum_attempts: i32::try_from(self.max_attempts).unwrap_or(i32::MAX),
            non_retryable_error_types: self.non_retryable_error_types,
        }))
    }
}

fn duration_to_proto(d: Duration) -> prost_wkt_types::Duration {
    prost_wkt_types::Duration {
        seconds: i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
        nanos: i32::try_from(d.subsec_nanos()).unwrap_or(i32::MAX),
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::duration_suboptimal_units)]
mod tests {
    use super::*;

    #[test]
    fn defaults_populate_expected_proto_fields() {
        let p = RetryPolicy::builder().build().unwrap().into_inner();
        assert_eq!(p.backoff_coefficient, 2.0);
        assert_eq!(p.maximum_attempts, 0);
        let initial = p.initial_interval.expect("initial");
        assert_eq!(initial.seconds, 1);
        let max = p.maximum_interval.expect("maximum");
        assert_eq!(max.seconds, 30);
        assert!(p.non_retryable_error_types.is_empty());
    }

    #[test]
    fn build_rejects_nan_coefficient() {
        let res = RetryPolicy::builder().backoff_coefficient(f64::NAN).build();
        assert!(matches!(res, Err(Error::Configuration(_))));
    }

    #[test]
    fn build_rejects_zero_coefficient() {
        let res = RetryPolicy::builder().backoff_coefficient(0.0).build();
        assert!(matches!(res, Err(Error::Configuration(_))));
    }

    #[test]
    fn build_rejects_zero_initial() {
        let res = RetryPolicy::builder()
            .initial_interval(Duration::ZERO)
            .build();
        assert!(matches!(res, Err(Error::Configuration(_))));
    }

    #[test]
    fn build_rejects_inverted_intervals() {
        let res = RetryPolicy::builder()
            .initial_interval(Duration::from_secs(60))
            .maximum_interval(Duration::from_secs(1))
            .build();
        let err = res.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("must be <="),
            "expected inversion message, got {msg}",
        );
    }

    #[test]
    fn overrides_apply() {
        let p = RetryPolicy::builder()
            .initial_interval(Duration::from_millis(500))
            .maximum_interval(Duration::from_secs(60))
            .backoff_coefficient(1.5)
            .max_attempts(7)
            .non_retryable("AuthError")
            .non_retryable("ValidationError")
            .build()
            .unwrap()
            .into_inner();
        assert_eq!(p.backoff_coefficient, 1.5);
        assert_eq!(p.maximum_attempts, 7);
        let initial = p.initial_interval.expect("initial");
        assert_eq!(initial.seconds, 0);
        assert_eq!(initial.nanos, 500_000_000);
        assert_eq!(
            p.non_retryable_error_types,
            vec!["AuthError", "ValidationError"]
        );
    }
}
