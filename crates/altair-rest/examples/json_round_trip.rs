//! `get_json` / `post_json` helpers — the 80% case of REST consumption.
//!
//! Run with: `cargo run --example json_round_trip -p altair-rest`

use altair_rest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Post {
    id: Option<u64>,
    title: String,
    body: String,
    #[serde(rename = "userId")]
    user_id: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::builder()
        .base_url("https://jsonplaceholder.typicode.com")?
        .build()?;

    // GET → decode into Post
    let post: Post = client.get_json("/posts/1").await?;
    println!("fetched: {post:#?}");

    // POST a new Post → decode the server's response
    let new_post = Post {
        id: None,
        title: "altair-rest demo".into(),
        body: "hello from the example".into(),
        user_id: 1,
    };
    let created: Post = client.post_json("/posts", &new_post).await?;
    println!("created (assigned id {:?}): {created:#?}", created.id);

    Ok(())
}
