//! Encode a small structured payload into a workflow ID.
//!
//! Temporal's `ScheduleAction::StartWorkflow` cannot attach workflow input,
//! so projects encode small payloads into the workflow ID itself. This
//! module standardises the encoding using Crockford Base32 (via
//! [`altair_base32`]) over the JSON bytes of the payload.
//!
//! Format: `{prefix}-{base32}`.
//!
//! # Limits
//!
//! Temporal workflow IDs cap at 200 bytes. Use this for small payloads
//! only (IDs, short strings, a handful of fields). Larger payloads belong
//! in activity input.

use crate::error::{Error, Result};

/// Temporal's workflow ID length limit, in bytes.
pub const MAX_WORKFLOW_ID_LEN: usize = 200;

/// Encode `payload` into a workflow ID of the form `{prefix}-{base32}`.
///
/// # Errors
/// * `Error::Configuration` if serialisation fails or the resulting ID
///   exceeds [`MAX_WORKFLOW_ID_LEN`].
pub fn encode<T: serde::Serialize>(prefix: &str, payload: &T) -> Result<String> {
    let bytes = serde_json::to_vec(payload)
        .map_err(|e| Error::Configuration(format!("payload serialise failed: {e}")))?;
    let encoded = altair_base32::encode(&bytes);
    let id = format!("{prefix}-{encoded}");
    if id.len() > MAX_WORKFLOW_ID_LEN {
        return Err(Error::Configuration(format!(
            "workflow id is {} bytes, max {MAX_WORKFLOW_ID_LEN}",
            id.len()
        )));
    }
    Ok(id)
}

/// Decode a workflow ID produced by [`encode`].
///
/// Returns `(prefix, payload)`. The prefix may itself contain `-` —
/// only the last `-` separates prefix from encoded payload.
///
/// # Errors
/// * `Error::Configuration` if the ID has no `-`, the encoded segment
///   is not valid Crockford Base32, or the bytes do not deserialise as `T`.
pub fn decode<T: serde::de::DeserializeOwned>(id: &str) -> Result<(String, T)> {
    let (prefix, encoded) = id.rsplit_once('-').ok_or_else(|| {
        Error::Configuration(format!("workflow id missing '-' separator: {id}"))
    })?;
    let bytes = altair_base32::decode(encoded)
        .map_err(|e| Error::Configuration(format!("workflow id base32 decode failed: {e}")))?;
    let payload: T = serde_json::from_slice(&bytes)
        .map_err(|e| Error::Configuration(format!("workflow id payload deserialise failed: {e}")))?;
    Ok((prefix.to_string(), payload))
}
