//! Make a single GET request with default settings.
//!
//! Run with: `cargo run --example basic -p altair-rest`

use altair_rest::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::builder().build()?;
    let response = client.get("https://httpbin.org/get").send().await?;
    println!("status: {}", response.status());
    let body = response.text().await?;
    println!("body (first 200 chars):\n{}", &body[..body.len().min(200)]);
    Ok(())
}
