//! Encode/decode a small payload through a workflow ID.
//!
//! Run with: `cargo run -p altair-temporal --example workflow_id_payload`

use altair_temporal::workflow_id;

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct ArchiveSpec {
    name: String,
    year: u32,
}

fn main() -> anyhow::Result<()> {
    let spec = ArchiveSpec {
        name: "customer".to_string(),
        year: 2026,
    };
    let id = workflow_id::encode("archive", &spec)?;
    println!("workflow id: {id} ({} bytes)", id.len());

    let (prefix, decoded): (String, ArchiveSpec) = workflow_id::decode(&id)?;
    println!("decoded prefix: {prefix}");
    println!("decoded spec:   {decoded:?}");
    assert_eq!(decoded, spec);
    Ok(())
}
