//! Append your own middleware to the chain. Custom middleware runs AFTER
//! the built-in retry and tracing middleware.
//!
//! Run with: `cargo run --example custom_middleware -p altair-rest`

use altair_rest::Client;
use altair_rest::reqwest_middleware::{Middleware, Next};
use async_trait::async_trait;
use http::Extensions;
use reqwest::{Request, Response};

struct LoggingMiddleware;

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        println!("[middleware] {} {}", req.method(), req.url());
        let response = next.run(req, extensions).await?;
        println!("[middleware] -> {}", response.status());
        Ok(response)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::builder()
        .base_url("https://httpbin.org")?
        .with_middleware(LoggingMiddleware)
        .build()?;

    let _ = client.get("/get").send().await?;
    let _ = client.get("/uuid").send().await?;
    Ok(())
}
