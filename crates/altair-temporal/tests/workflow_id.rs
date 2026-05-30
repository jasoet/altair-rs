//! Encode/decode round-trip + edge cases for `workflow_id`.

use altair_temporal::Error;
use altair_temporal::workflow_id::{MAX_WORKFLOW_ID_LEN, decode, encode};

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Payload {
    archive_name: String,
    target_year: u32,
}

#[test]
fn round_trip_simple_prefix() {
    let payload = Payload {
        archive_name: "customer".to_string(),
        target_year: 2026,
    };
    let id = encode("archive", &payload).unwrap();
    let (prefix, out): (String, Payload) = decode(&id).unwrap();
    assert_eq!(prefix, "archive");
    assert_eq!(out, payload);
}

#[test]
fn round_trip_prefix_with_hyphens() {
    let payload = Payload {
        archive_name: "x".to_string(),
        target_year: 1,
    };
    let id = encode("daily-archive-prod", &payload).unwrap();
    let (prefix, out): (String, Payload) = decode(&id).unwrap();
    assert_eq!(prefix, "daily-archive-prod");
    assert_eq!(out, payload);
}

#[test]
fn decode_rejects_missing_separator() {
    let err = decode::<Payload>("noSeparatorHere").unwrap_err();
    assert!(matches!(err, Error::Configuration(_)));
}

#[test]
fn decode_rejects_invalid_base32() {
    let err = decode::<Payload>("archive-not!base32").unwrap_err();
    assert!(matches!(err, Error::Configuration(_)));
}

#[test]
fn decode_rejects_invalid_json() {
    let bytes = b"not json bytes";
    let encoded = altair_base32::encode(bytes);
    let id = format!("archive-{encoded}");
    let err = decode::<Payload>(&id).unwrap_err();
    assert!(matches!(err, Error::Configuration(_)));
}

#[test]
fn encode_rejects_overlong_payload() {
    let big = Payload {
        archive_name: "x".repeat(500),
        target_year: 0,
    };
    let err = encode("p", &big).unwrap_err();
    assert!(matches!(err, Error::Configuration(_)));
}

#[test]
fn decode_rejects_empty_encoded_segment() {
    let err = decode::<Payload>("archive-").unwrap_err();
    assert!(matches!(err, Error::Configuration(_)));
}

#[test]
fn boundary_max_id_len_passes() {
    let body_len = MAX_WORKFLOW_ID_LEN / 2;
    let payload = Payload {
        archive_name: "x".repeat(body_len.saturating_sub(20)),
        target_year: 1,
    };
    let id = encode("p", &payload).unwrap();
    assert!(id.len() <= MAX_WORKFLOW_ID_LEN);
}
