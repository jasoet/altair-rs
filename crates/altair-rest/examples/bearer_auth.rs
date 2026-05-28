//! Construct an API client with a Bearer token and a default tenant header.
//!
//! Run with: `cargo run --example bearer_auth -p altair-rest`

use altair_rest::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::builder()
        .base_url("https://httpbin.org")?
        .bearer_token("ya29.example-token-value")
        .default_header("x-tenant", "acme")?
        .build()?;

    let response = client.get("/headers").send().await?;
    println!("status: {}", response.status());
    let body: serde_json::Value = response.json().await?;
    println!(
        "headers as seen by server:\n{}",
        serde_json::to_string_pretty(&body)?
    );
    Ok(())
}
