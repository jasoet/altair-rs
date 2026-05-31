//! Data-sync pipeline: `Source` -> `Mapper` -> `Sink` with optional
//! partitioning + continue-as-new.
//!
//! Two layers:
//!
//! 1. **Core primitives** (this module): the [`Source`], [`Mapper`], and
//!    [`Sink`] traits plus the [`Runner`] in-process orchestrator.
//!    Pure Rust, no Temporal coupling — wire them into your own
//!    workflow / activity types, or call [`Runner::run`] directly from
//!    a test / script.
//! 2. **Chunk submodule** ([`chunk`]): partitioned, resumable
//!    orchestration on top of the primitives — useful for jobs whose
//!    record count would overflow a single Temporal history.
//!
//! ```no_run
//! use std::sync::Arc;
//! use altair_wf::datasync::{IdentityMapper, Runner, Sink, Source, WriteResult};
//! use async_trait::async_trait;
//!
//! struct VecSource(Vec<i32>);
//! #[async_trait]
//! impl Source<i32> for VecSource {
//!     fn name(&self) -> &str { "vec" }
//!     async fn fetch(&self) -> altair_wf::Result<Vec<i32>> { Ok(self.0.clone()) }
//! }
//!
//! struct CounterSink;
//! #[async_trait]
//! impl Sink<i32> for CounterSink {
//!     fn name(&self) -> &str { "counter" }
//!     async fn write(&self, records: Vec<i32>) -> altair_wf::Result<WriteResult> {
//!         Ok(WriteResult { inserted: records.len(), ..Default::default() })
//!     }
//! }
//!
//! # async fn ex() -> altair_wf::Result<()> {
//! let runner: Runner<i32, i32> = Runner::new(
//!     Arc::new(VecSource(vec![1, 2, 3])),
//!     Arc::new(IdentityMapper::new()),
//!     Arc::new(CounterSink),
//! );
//! let out = runner.run().await?;
//! assert_eq!(out.total_fetched, 3);
//! # Ok(()) }
//! ```

pub mod chunk;
mod job;
mod mapper;
mod result;
mod runner;
mod sink;
mod source;

pub use job::{Job, SyncJobBuilder};
pub use mapper::{DetailedMapper, IdentityMapper, Mapper, RecordMapper};
pub use result::{MapResult, SyncResult};
pub use runner::Runner;
pub use sink::{Sink, WriteResult};
pub use source::Source;
