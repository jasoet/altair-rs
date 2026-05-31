//! [`Job`] definition + [`SyncJobBuilder`] fluent constructor.

use std::sync::Arc;
use std::time::Duration;

use crate::datasync::mapper::Mapper;
use crate::datasync::sink::Sink;
use crate::datasync::source::Source;
use crate::error::{Error, Result};

/// Complete sync pipeline definition: `Source` → `Mapper` → `Sink` plus
/// schedule and Temporal-side options.
///
/// Constructed via [`SyncJobBuilder`]. The struct is open and `Clone`-free:
/// users typically wrap it in an `Arc` for shared ownership across the
/// worker (which registers the activities) and the workflow body (which
/// reads the schedule + retry options). The expected pattern is:
///
/// ```text
/// let job = std::sync::Arc::new(SyncJobBuilder::new("orders").<...>.build()?);
/// ```
///
/// then hand `job.clone()` to whichever side needs it.
pub struct Job<T, U>
where
    T: Send + 'static,
    U: Send + 'static,
{
    /// Stable identifier used as the workflow type, activity prefix,
    /// and schedule id.
    pub name: String,
    /// Records producer.
    pub source: Arc<dyn Source<T>>,
    /// Source -> sink transformation.
    pub mapper: Arc<dyn Mapper<T, U>>,
    /// Records consumer.
    pub sink: Arc<dyn Sink<U>>,
    /// Scheduling interval (zero == not scheduled / run-on-demand).
    pub schedule: Duration,

    /// Maximum activity execution time. Zero == defer to the SDK
    /// default (no explicit cap).
    pub activity_timeout: Duration,
    /// Heartbeat interval. Zero == defer to the SDK default.
    pub heartbeat_timeout: Duration,
    /// Maximum retry attempts for activity failures.
    pub max_retries: i32,
    /// First retry backoff.
    pub retry_initial_interval: Duration,
    /// Retry backoff multiplier.
    pub retry_backoff_coefficient: f64,
    /// Cap on retry backoff.
    pub retry_max_interval: Duration,
}

/// Fluent builder for [`Job`]. Validates required fields in
/// [`SyncJobBuilder::build`].
pub struct SyncJobBuilder<T, U>
where
    T: Send + 'static,
    U: Send + 'static,
{
    name: String,
    source: Option<Arc<dyn Source<T>>>,
    mapper: Option<Arc<dyn Mapper<T, U>>>,
    sink: Option<Arc<dyn Sink<U>>>,
    schedule: Duration,
    activity_timeout: Duration,
    heartbeat_timeout: Duration,
    max_retries: i32,
    retry_initial_interval: Duration,
    retry_backoff_coefficient: f64,
    retry_max_interval: Duration,
}

impl<T, U> SyncJobBuilder<T, U>
where
    T: Send + 'static,
    U: Send + 'static,
{
    /// Start a builder for a job with the given `name`.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: None,
            mapper: None,
            sink: None,
            schedule: Duration::ZERO,
            activity_timeout: Duration::ZERO,
            heartbeat_timeout: Duration::ZERO,
            max_retries: 0,
            retry_initial_interval: Duration::ZERO,
            retry_backoff_coefficient: 0.0,
            retry_max_interval: Duration::ZERO,
        }
    }

    /// Set the data source.
    #[must_use]
    pub fn source(mut self, source: Arc<dyn Source<T>>) -> Self {
        self.source = Some(source);
        self
    }

    /// Set the mapper.
    #[must_use]
    pub fn mapper(mut self, mapper: Arc<dyn Mapper<T, U>>) -> Self {
        self.mapper = Some(mapper);
        self
    }

    /// Set the sink.
    #[must_use]
    pub fn sink(mut self, sink: Arc<dyn Sink<U>>) -> Self {
        self.sink = Some(sink);
        self
    }

    /// Set the scheduling interval. Required (must be `> 0`).
    #[must_use]
    pub fn schedule(mut self, schedule: Duration) -> Self {
        self.schedule = schedule;
        self
    }

    /// Set the activity start-to-close timeout.
    #[must_use]
    pub fn activity_timeout(mut self, timeout: Duration) -> Self {
        self.activity_timeout = timeout;
        self
    }

    /// Set the activity heartbeat timeout.
    #[must_use]
    pub fn heartbeat_timeout(mut self, timeout: Duration) -> Self {
        self.heartbeat_timeout = timeout;
        self
    }

    /// Set the maximum retry attempts.
    #[must_use]
    pub fn max_retries(mut self, max_retries: i32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set the initial retry backoff.
    #[must_use]
    pub fn retry_initial_interval(mut self, interval: Duration) -> Self {
        self.retry_initial_interval = interval;
        self
    }

    /// Set the retry backoff coefficient.
    #[must_use]
    pub fn retry_backoff_coefficient(mut self, coeff: f64) -> Self {
        self.retry_backoff_coefficient = coeff;
        self
    }

    /// Set the maximum retry interval.
    #[must_use]
    pub fn retry_max_interval(mut self, interval: Duration) -> Self {
        self.retry_max_interval = interval;
        self
    }

    /// Validate and build the [`Job`].
    ///
    /// # Errors
    ///
    /// [`Error::InvalidInput`] if `name` is empty, `source`/`mapper`/`sink`
    /// is missing, or `schedule` is zero.
    pub fn build(self) -> Result<Job<T, U>> {
        if self.name.trim().is_empty() {
            return Err(Error::InvalidInput("job name is required".into()));
        }
        let source = self
            .source
            .ok_or_else(|| Error::InvalidInput("source is required".into()))?;
        let mapper = self
            .mapper
            .ok_or_else(|| Error::InvalidInput("mapper is required".into()))?;
        let sink = self
            .sink
            .ok_or_else(|| Error::InvalidInput("sink is required".into()))?;
        if self.schedule.is_zero() {
            return Err(Error::InvalidInput("schedule must be > 0".into()));
        }
        Ok(Job {
            name: self.name,
            source,
            mapper,
            sink,
            schedule: self.schedule,
            activity_timeout: self.activity_timeout,
            heartbeat_timeout: self.heartbeat_timeout,
            max_retries: self.max_retries,
            retry_initial_interval: self.retry_initial_interval,
            retry_backoff_coefficient: self.retry_backoff_coefficient,
            retry_max_interval: self.retry_max_interval,
        })
    }
}

impl<T, U> Default for SyncJobBuilder<T, U>
where
    T: Send + 'static,
    U: Send + 'static,
{
    /// Build with an empty name. `build()` will reject it — useful only
    /// for derive-`Default` integration with parent structs.
    fn default() -> Self {
        Self::new(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datasync::mapper::IdentityMapper;
    use crate::datasync::sink::WriteResult;
    use async_trait::async_trait;

    struct EmptySource;
    #[async_trait]
    impl Source<i32> for EmptySource {
        fn name(&self) -> &'static str {
            "empty"
        }
        async fn fetch(&self) -> Result<Vec<i32>> {
            Ok(vec![])
        }
    }

    struct NullSink;
    #[async_trait]
    impl Sink<i32> for NullSink {
        fn name(&self) -> &'static str {
            "null"
        }
        async fn write(&self, _records: Vec<i32>) -> Result<WriteResult> {
            Ok(WriteResult::default())
        }
    }

    #[test]
    fn builder_rejects_empty_name() {
        let b: SyncJobBuilder<i32, i32> = SyncJobBuilder::new("")
            .source(Arc::new(EmptySource))
            .mapper(Arc::new(IdentityMapper::new()))
            .sink(Arc::new(NullSink))
            .schedule(Duration::from_mins(1));
        assert!(matches!(b.build(), Err(Error::InvalidInput(_))));
    }

    #[test]
    fn builder_rejects_missing_source() {
        let b: SyncJobBuilder<i32, i32> = SyncJobBuilder::new("j")
            .mapper(Arc::new(IdentityMapper::new()))
            .sink(Arc::new(NullSink))
            .schedule(Duration::from_mins(1));
        assert!(matches!(b.build(), Err(Error::InvalidInput(_))));
    }

    #[test]
    fn builder_rejects_zero_schedule() {
        let b: SyncJobBuilder<i32, i32> = SyncJobBuilder::new("j")
            .source(Arc::new(EmptySource))
            .mapper(Arc::new(IdentityMapper::new()))
            .sink(Arc::new(NullSink));
        assert!(matches!(b.build(), Err(Error::InvalidInput(_))));
    }

    #[test]
    fn builder_accepts_minimum_valid_config() {
        let job: Job<i32, i32> = SyncJobBuilder::new("j")
            .source(Arc::new(EmptySource))
            .mapper(Arc::new(IdentityMapper::new()))
            .sink(Arc::new(NullSink))
            .schedule(Duration::from_mins(1))
            .build()
            .unwrap();
        assert_eq!(job.name, "j");
        assert_eq!(job.schedule, Duration::from_mins(1));
    }
}
