# altair-db

[![crates.io](https://img.shields.io/crates/v/altair-db.svg)](https://crates.io/crates/altair-db)

Sea-ORM + sqlx convenience layer with smart pool defaults, file-based migrations, OTel-aware query tracing, and a closure-style transaction helper. Three backends: PostgreSQL, MySQL, SQLite.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Why

Most Rust services want both:

- An **ORM** (sea-orm) for the 80% of CRUD — type-safe entities, `find_by_id`, derived migrations.
- **Raw sqlx** for the 20% that the ORM doesn't model well — materialised views, `LISTEN/NOTIFY`, `COPY`, advanced CTEs.

`altair-db` gives you both from a single `Db` handle, backed by a single connection pool.

## Install

```toml
[dependencies]
altair-db = "0.1"
```

By default all three backends compile in. To shrink to just one:

```toml
altair-db = { version = "0.1", default-features = false, features = ["postgres"] }
```

## Quick start

```rust
use altair_db::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = Db::connect(Config::from_url("postgres://app:secret@localhost/app")).await?;
    db.migrate("./migrations").await?;

    // ORM CRUD via SeaORM
    let orm = db.orm();
    // users::Entity::find().one(orm).await?;

    // Raw sqlx for power-user queries
    let pool = db.pg_pool().expect("postgres");
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM users").fetch_one(pool).await?;
    println!("users: {count}");

    Ok(())
}
```

## What it gives you

- **Single pool** shared by SeaORM and raw sqlx (SeaORM owns it; we expose the inner pool).
- **Pool defaults**: `max_connections=10`, `min=1`, `acquire_timeout=30s`, `idle_timeout=10m`, `max_lifetime=30m`. All overridable via `Config`.
- **Migrations**: `db.migrate("./migrations")` runs sqlx's filesystem migrator with `_sqlx_migrations` tracking.
- **Health probe**: `db.ping().await` issues `SELECT 1`. Plug into altair-server's `/health`.
- **Transactions**: `db.transaction(|tx| async { ... })` — closure-style commit/rollback over SeaORM's `DatabaseTransaction`.
- **Tracing**: sqlx's per-statement spans flow to whatever `tracing` subscriber you install (e.g. `altair-otel`). Slow statements log at WARN above `sqlx_slow_query_threshold`.

## Examples

| File | Demonstrates |
|---|---|
| `basic_orm.rs` | SeaORM CRUD round-trip against SQLite. |
| `raw_sqlx.rs` | `db.sqlite_pool()` + parameterised raw query. |
| `materialized_view.rs` | Postgres-only `CREATE MATERIALIZED VIEW` + `REFRESH` via raw sqlx. |
| `migrations.rs` | `db.migrate(path)` over an in-memory migrations dir. |
| `transaction.rs` | Two-write transaction inside a closure. |
| `with_config.rs` | `Config` loaded from a TOML file via `serde`. |
| `with_otel.rs` | Cross-crate auto-integration: every query becomes an OTel span. |

Run any example: `cargo run --example <name> -p altair-db`.

## Backends + features

| Feature | Effect |
|---|---|
| `postgres` (default) | Enables `db.pg_pool()`. |
| `mysql` (default) | Enables `db.mysql_pool()`. |
| `sqlite` (default) | Enables `db.sqlite_pool()`. |

Backend selection is **runtime** — the URL scheme (`postgres://`, `mysql://`, `sqlite://`) determines which backend the pool opens. Per-backend accessors return `Option` because at compile time multiple backends may be on, but at runtime only one is connected.

## Config

```toml
[database]
url = "postgres://app:secret@localhost/app"
max_connections = 20
min_connections = 2
acquire_timeout = "30s"
idle_timeout = "10m"
max_lifetime = "30m"
sqlx_logging = true
sqlx_slow_query_threshold = "1s"
```

Every field has a sensible default; only `url` is required.

## License

Apache-2.0
