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

/// Serde adapter that encodes [`Duration`] as `u64` millis on the wire.
///
/// Round-trips losslessly for any duration `< u64::MAX` millis (~584M
/// years); larger durations saturate at `u64::MAX` on serialize.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datasync::sink::WriteResult;

    #[test]
    fn sync_result_serde_round_trip_pins_millis_duration() {
        let original = SyncResult {
            total_fetched: 42,
            write_result: WriteResult {
                inserted: 30,
                updated: 10,
                skipped: 2,
            },
            processing_time: Duration::from_millis(1500),
        };
        let value = serde_json::to_value(&original).unwrap();
        // Duration encoded as a bare millis u64.
        assert_eq!(
            value
                .get("processingTime")
                .or_else(|| value.get("processing_time"))
                .and_then(serde_json::Value::as_u64),
            Some(1500)
        );
        let back: SyncResult = serde_json::from_value(value).unwrap();
        assert_eq!(back.total_fetched, 42);
        assert_eq!(back.write_result.inserted, 30);
        assert_eq!(back.write_result.updated, 10);
        assert_eq!(back.write_result.skipped, 2);
        assert_eq!(back.processing_time, Duration::from_millis(1500));
    }
}
