//! Retry configuration.

use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Retry policy.
#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) name: String,
    pub(crate) max_retries: u32,
    pub(crate) initial_interval: Duration,
    pub(crate) max_interval: Duration,
    pub(crate) multiplier: f64,
    pub(crate) jitter: bool,
    pub(crate) cancellation_token: Option<CancellationToken>,
}

impl Config {
    /// Start building a new config.
    #[must_use]
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }

    /// Return a default config with the given name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: "unnamed".to_string(),
            max_retries: 5,
            initial_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(30),
            multiplier: 1.5,
            jitter: true,
            cancellation_token: None,
        }
    }
}

/// Builder for [`Config`].
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    inner: Config,
}

impl ConfigBuilder {
    /// Set the operation name (appears in spans + error messages).
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.inner.name = name.into();
        self
    }

    /// Maximum number of retry attempts after the initial call.
    #[must_use]
    pub fn max_retries(mut self, n: u32) -> Self {
        self.inner.max_retries = n;
        self
    }

    /// Initial backoff interval.
    #[must_use]
    pub fn initial_interval(mut self, d: Duration) -> Self {
        self.inner.initial_interval = d;
        self
    }

    /// Maximum backoff interval (caps exponential growth).
    #[must_use]
    pub fn max_interval(mut self, d: Duration) -> Self {
        self.inner.max_interval = d;
        self
    }

    /// Exponential growth factor (e.g., 1.5, 2.0).
    #[must_use]
    pub fn multiplier(mut self, m: f64) -> Self {
        self.inner.multiplier = m;
        self
    }

    /// Toggle backoff jitter.
    #[must_use]
    pub fn jitter(mut self, on: bool) -> Self {
        self.inner.jitter = on;
        self
    }

    /// Attach a [`CancellationToken`] — when triggered, retry returns [`crate::Error::Cancelled`].
    #[must_use]
    pub fn cancellation_token(mut self, token: CancellationToken) -> Self {
        self.inner.cancellation_token = Some(token);
        self
    }

    /// Finalize the config.
    #[must_use]
    pub fn build(self) -> Config {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn default_has_sensible_values() {
        let c = Config::default();
        assert_eq!(c.max_retries, 5);
        assert_eq!(c.initial_interval, Duration::from_millis(100));
        assert!(c.jitter);
    }

    #[test]
    fn builder_overrides_defaults() {
        let c = Config::builder()
            .name("test")
            .max_retries(2)
            .initial_interval(Duration::from_millis(10))
            .jitter(false)
            .build();
        assert_eq!(c.name, "test");
        assert_eq!(c.max_retries, 2);
        assert!(!c.jitter);
    }
}
