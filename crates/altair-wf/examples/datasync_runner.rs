//! `altair-wf` — `datasync` core, in-process `Runner` (no Temporal).
//!
//! This example shows the `Source` -> `Mapper` -> `Sink` trio driven by
//! `Runner::run` directly. Use this as a quick prototype, a test
//! scaffold, or a CLI tool — no Temporal server needed.
//!
//! For the partitioned + resumable Temporal variant, see
//! `datasync_chunked.rs`.
//!
//! Run:
//! ```bash
//! cargo run -p altair-wf --features datasync --example datasync_runner
//! ```

#![allow(missing_docs)]

use std::sync::Arc;

use async_trait::async_trait;

use altair_wf::Result as WfResult;
use altair_wf::datasync::{
    DetailedMapper, MapResult, Mapper, RecordMapper, Runner, Sink, Source, WriteResult,
};

#[derive(Debug, Clone)]
pub struct RawRecord {
    pub id: u32,
    pub raw_name: String,
    pub will_skip: bool,
}

#[derive(Debug, Clone)]
pub struct CleanedRecord {
    pub id: u32,
    pub name: String,
}

/// Static source — returns a fixed batch on each call.
pub struct VecSource {
    pub records: Vec<RawRecord>,
}

#[async_trait]
impl Source<RawRecord> for VecSource {
    fn name(&self) -> &'static str {
        "vec-source"
    }

    async fn fetch(&self) -> WfResult<Vec<RawRecord>> {
        Ok(self.records.clone())
    }
}

/// Stdout sink — prints every record and reports them as `inserted`.
pub struct StdoutSink;

#[async_trait]
impl Sink<CleanedRecord> for StdoutSink {
    fn name(&self) -> &'static str {
        "stdout-sink"
    }

    async fn write(&self, records: Vec<CleanedRecord>) -> WfResult<WriteResult> {
        for r in &records {
            println!("  sink: id={:<3} name={}", r.id, r.name);
        }
        Ok(WriteResult {
            inserted: records.len(),
            ..Default::default()
        })
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> anyhow::Result<()> {
    let source = Arc::new(VecSource {
        records: vec![
            RawRecord {
                id: 1,
                raw_name: "  alice  ".into(),
                will_skip: false,
            },
            RawRecord {
                id: 2,
                raw_name: String::new(),
                will_skip: true,
            },
            RawRecord {
                id: 3,
                raw_name: " BOB ".into(),
                will_skip: false,
            },
            RawRecord {
                id: 4,
                raw_name: "carol".into(),
                will_skip: false,
            },
        ],
    });

    // `RecordMapper` applies a per-record function. Records whose
    // function returns `Err` are skipped (tracked in `MapResult::skipped`)
    // rather than failing the whole batch.
    let mapper: Arc<dyn Mapper<RawRecord, CleanedRecord>> = Arc::new(RecordMapper::new(
        "clean-names",
        |r: &RawRecord| -> Result<CleanedRecord, std::io::Error> {
            if r.will_skip {
                return Err(std::io::Error::other(format!("skipping id={}", r.id)));
            }
            Ok(CleanedRecord {
                id: r.id,
                name: r.raw_name.trim().to_lowercase(),
            })
        },
    ));

    // Demonstrate the detailed-mapper variant separately so we can see
    // the skip reasons; the Runner itself uses `Mapper::map`.
    let detailed: RecordMapper<RawRecord, CleanedRecord, _, std::io::Error> = RecordMapper::new(
        "preview",
        |r: &RawRecord| -> Result<CleanedRecord, std::io::Error> {
            if r.will_skip {
                Err(std::io::Error::other(format!("skipping id={}", r.id)))
            } else {
                Ok(CleanedRecord {
                    id: r.id,
                    name: r.raw_name.trim().to_lowercase(),
                })
            }
        },
    );
    let preview: MapResult<CleanedRecord> = detailed.map_detailed(source.records.clone());
    println!("preview (DetailedMapper):");
    println!(
        "  records = {} success, {} skipped",
        preview.records.len(),
        preview.skipped,
    );
    for reason in &preview.skip_reasons {
        println!("  skip: {reason}");
    }
    println!();

    let sink = Arc::new(StdoutSink);
    let runner: Runner<RawRecord, CleanedRecord> = Runner::new(source, mapper, sink);

    println!("running fetch -> map -> write cycle:");
    let result = runner.run().await?;
    println!();
    println!(
        "result: fetched={}, inserted={}, updated={}, skipped={}, elapsed={:?}",
        result.total_fetched,
        result.write_result.inserted,
        result.write_result.updated,
        result.write_result.skipped,
        result.processing_time,
    );
    Ok(())
}
