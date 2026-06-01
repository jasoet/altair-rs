//! In-process runner: drives a single source -> mapper -> sink cycle
//! with no Temporal coupling. Useful for tests, scripts, and quick
//! prototypes.

use std::sync::Arc;
use std::time::Instant;

use crate::datasync::mapper::Mapper;
use crate::datasync::result::SyncResult;
use crate::datasync::sink::Sink;
use crate::datasync::source::Source;
use crate::error::{Error, Result};

/// Drives a single fetch-map-write cycle in the current process.
///
/// Use for testing, in-process tools, and to validate the source/mapper/sink
/// trio before wrapping it in a Temporal workflow. Errors at any step are
/// returned with the source/sink name in their context.
pub struct Runner<T, U>
where
    T: Send + 'static,
    U: Send + 'static,
{
    source: Arc<dyn Source<T>>,
    mapper: Arc<dyn Mapper<T, U>>,
    sink: Arc<dyn Sink<U>>,
}

impl<T, U> Runner<T, U>
where
    T: Send + 'static,
    U: Send + 'static,
{
    /// Build a new runner wrapping the given trio.
    #[must_use]
    pub fn new(
        source: Arc<dyn Source<T>>,
        mapper: Arc<dyn Mapper<T, U>>,
        sink: Arc<dyn Sink<U>>,
    ) -> Self {
        Self {
            source,
            mapper,
            sink,
        }
    }

    /// Build a runner from a fully-validated [`Job`]. Convenience for
    /// the common "I already built a `Job` via the builder — run it
    /// in-process" path. The runner clones the job's `Arc<dyn ...>`
    /// trio; the [`Job`]'s schedule + retry/timeout fields are
    /// ignored here (they're workflow-side metadata) and remain
    /// available on the job for the caller's workflow wiring.
    ///
    /// [`Job`]: crate::datasync::Job
    #[must_use]
    pub fn from_job(job: &crate::datasync::Job<T, U>) -> Self {
        Self::new(
            Arc::clone(&job.source),
            Arc::clone(&job.mapper),
            Arc::clone(&job.sink),
        )
    }

    /// Run one fetch-map-write cycle. Returns a [`SyncResult`] with the
    /// fetch count, sink tally, and wall-clock processing time.
    ///
    /// # Errors
    ///
    /// Each stage failure is wrapped as [`Error::Activity`] so the
    /// taxonomy is symmetric across the trio (fetch, map, write). The
    /// `activity` field carries the stage label plus the source/sink
    /// name where applicable.
    pub async fn run(&self) -> Result<SyncResult> {
        // `Runner::run` is an in-process driver — NOT called from inside
        // a Temporal workflow body — so `Instant::now()` is fine here.
        let started = Instant::now();

        let records = self
            .source
            .fetch()
            .await
            .map_err(|e| Error::activity(format!("source {}: fetch", self.source.name()), e))?;

        let mut result = SyncResult {
            total_fetched: records.len(),
            ..SyncResult::default()
        };

        if records.is_empty() {
            result.processing_time = started.elapsed();
            return Ok(result);
        }

        let mapped = self
            .mapper
            .map(records)
            .await
            .map_err(|e| Error::activity("mapper: map".to_string(), e))?;

        let write_result = self
            .sink
            .write(mapped)
            .await
            .map_err(|e| Error::activity(format!("sink {}: write", self.sink.name()), e))?;

        result.write_result = write_result;
        result.processing_time = started.elapsed();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datasync::mapper::IdentityMapper;
    use crate::datasync::sink::WriteResult;
    use async_trait::async_trait;

    struct VecSource(Vec<i32>);
    #[async_trait]
    impl Source<i32> for VecSource {
        fn name(&self) -> &'static str {
            "vec"
        }
        async fn fetch(&self) -> Result<Vec<i32>> {
            Ok(self.0.clone())
        }
    }

    struct CountingSink {
        inner: std::sync::Mutex<Vec<i32>>,
    }
    #[async_trait]
    impl Sink<i32> for CountingSink {
        fn name(&self) -> &'static str {
            "count"
        }
        async fn write(&self, records: Vec<i32>) -> Result<WriteResult> {
            let mut g = self.inner.lock().unwrap();
            let n = records.len();
            g.extend(records);
            Ok(WriteResult {
                inserted: n,
                ..WriteResult::default()
            })
        }
    }

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

    #[tokio::test]
    async fn runner_executes_fetch_map_write() {
        let sink = Arc::new(CountingSink {
            inner: std::sync::Mutex::new(Vec::new()),
        });
        let runner: Runner<i32, i32> = Runner::new(
            Arc::new(VecSource(vec![1, 2, 3])),
            Arc::new(IdentityMapper::new()),
            sink.clone(),
        );
        let out = runner.run().await.unwrap();
        assert_eq!(out.total_fetched, 3);
        assert_eq!(out.write_result.inserted, 3);
        assert_eq!(*sink.inner.lock().unwrap(), vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn runner_from_job_reuses_jobs_trio() {
        use crate::datasync::SyncJobBuilder;
        use std::time::Duration;
        let sink = Arc::new(CountingSink {
            inner: std::sync::Mutex::new(Vec::new()),
        });
        let job = SyncJobBuilder::<i32, i32>::new("j")
            .source(Arc::new(VecSource(vec![10, 20, 30])))
            .mapper(Arc::new(IdentityMapper::new()))
            .sink(sink.clone())
            .schedule(Duration::from_mins(1))
            .build()
            .unwrap();
        let runner = Runner::from_job(&job);
        let out = runner.run().await.unwrap();
        assert_eq!(out.total_fetched, 3);
        assert_eq!(out.write_result.inserted, 3);
        assert_eq!(*sink.inner.lock().unwrap(), vec![10, 20, 30]);
    }

    #[tokio::test]
    async fn runner_short_circuits_on_empty_fetch() {
        let sink = Arc::new(CountingSink {
            inner: std::sync::Mutex::new(Vec::new()),
        });
        let runner: Runner<i32, i32> = Runner::new(
            Arc::new(EmptySource),
            Arc::new(IdentityMapper::new()),
            sink.clone(),
        );
        let out = runner.run().await.unwrap();
        assert_eq!(out.total_fetched, 0);
        assert_eq!(out.write_result.total(), 0);
        assert!(sink.inner.lock().unwrap().is_empty());
    }
}
