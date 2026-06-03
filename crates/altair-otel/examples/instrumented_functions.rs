//! `#[instrument]` produces a span automatically for any async or sync
//! function — with arguments captured as fields. Use `skip(...)` or
//! `skip_all` for arguments you don't want logged.
//!
//! Run with: `cargo run --example instrumented_functions -p altair-otel`

use altair_otel::Exporter;
use altair_otel::prelude::*;
use std::collections::HashMap;

struct Database {
    users: HashMap<u64, String>,
}

#[derive(Debug)]
struct User {
    name: String,
}

impl Database {
    fn find_user(&self, id: u64) -> Option<User> {
        self.users.get(&id).map(|name| User { name: name.clone() })
    }
}

/// Captures `user_id` as a span field. `&self` (the db handle) is too big
/// to log usefully, so we `skip` it.
#[instrument(skip(db))]
fn fetch_user(db: &Database, user_id: u64) -> Option<User> {
    info!("fetching user");
    let user = db.find_user(user_id);
    if user.is_some() {
        info!("user found");
    } else {
        warn!("user not found");
    }
    user
}

/// Async functions work the same way; the span covers the .await points.
#[instrument(skip(db), fields(user.id = id))]
async fn fetch_user_async(db: &Database, id: u64) -> Option<User> {
    info!("starting async fetch");
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    db.find_user(id)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Config::builder()
        .service_name("instrumented-demo")
        .exporter(Exporter::Stdout)
        .build()
        .unwrap()
        .init()?;

    let db = Database {
        users: HashMap::from([(42, "alice".to_string()), (99, "bob".to_string())]),
    };

    if let Some(u) = fetch_user(&db, 42) {
        info!(user.name = %u.name, "got user (sync)");
    }
    // Try an unknown id to demonstrate the warn! branch.
    let _missing = fetch_user(&db, 7);

    if let Some(u) = fetch_user_async(&db, 99).await {
        info!(user.name = %u.name, "got user (async)");
    }

    shutdown();
    Ok(())
}
