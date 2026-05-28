//! Load config from any `std::io::Read`. Useful when config comes from an
//! embedded binary, a network response, or an in-memory buffer.
//!
//! Run with: `cargo run --example from_reader -p altair-config`

use altair_config::prelude::*;
use std::io::Cursor;

#[derive(Debug, Deserialize, Validate)]
struct AppConfig {
    #[validate(range(min = 1, max = 65535))]
    port: u16,
    #[validate(length(min = 1))]
    region: String,
}

fn main() -> anyhow::Result<()> {
    // Imagine this came from include_bytes!(), a network response, or
    // anywhere else producing bytes.
    let embedded: &'static [u8] = br#"
port = 7000
region = "us-east-1"
"#;

    let cfg: AppConfig = from_reader(Cursor::new(embedded), "APP_NONE_XYZ")?;
    println!("loaded from in-memory reader: {cfg:#?}");

    Ok(())
}
