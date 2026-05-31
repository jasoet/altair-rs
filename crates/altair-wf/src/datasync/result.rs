//! Result types: [`SyncResult`] for runner outcomes, [`MapResult`] for
//! detailed mapper outcomes.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::datasync::sink::WriteResult;

/// Outcome of a single fetch-map-write cycle.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    /// Records returned by [`Source::fetch`](crate::datasync::Source::fetch).
    #[serde(default)]
    pub total_fetched: usize,
    /// Aggregated sink-side tally.
    #[serde(default)]
    pub write_result: WriteResult,
    /// Wall-clock duration of the cycle (millis on the wire).
    #[serde(default, with = "duration_millis_compat")]
    pub processing_time: Duration,
}

/// Mapper output enriched with skip tracking — useful when the mapper
/// chooses to drop records (e.g. on per-record validation errors) but
/// the cycle should not fail.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MapResult<U> {
    /// Records that mapped successfully.
    #[serde(default = "Vec::new")]
    pub records: Vec<U>,
    /// How many input records were dropped.
    #[serde(default)]
    pub skipped: usize,
    /// Human-readable reasons for the skips, one per skipped record.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip_reasons: Vec<String>,
}

mod duration_millis_compat {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_u64(d.as_millis().try_into().unwrap_or(u64::MAX))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Duration, D::Error> {
        u64::deserialize(de).map(Duration::from_millis)
    }
}
