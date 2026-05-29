# altair-db Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build, test, document, and publish `altair-db` — a sea-orm + sqlx convenience layer with three backends (Postgres + MySQL + SQLite), smart pool defaults, file-based migrations, OTel-aware query tracing, a closure-style transaction helper, and a `ping()` health probe — to crates.io at the current workspace version.

**Architecture:** Single crate under `crates/altair-db/`. Five source files (`lib.rs`, `error.rs`, `config.rs`, `connect.rs`, `db.rs`, `prelude.rs`). `Db` owns a `sea_orm::DatabaseConnection`; SeaORM owns the sqlx pool. Public accessors expose both layers from a single handle. Backend selection is runtime, based on DSN scheme; per-backend pool accessors are feature-gated and return `Option`.

**Tech Stack:**
- Rust 2024, MSRV 1.95 (inherit from workspace)
- `sea-orm = "1"` (default-features off) with features `runtime-tokio-rustls`, `sqlx-postgres`, `sqlx-mysql`, `sqlite`, `macros`
- `sqlx = "0.8"` (default-features off) with features `runtime-tokio-rustls`, `postgres`, `mysql`, `sqlite`, `migrate`
- `humantime-serde = "1"` for `Duration` (de)serialisation as `"30s"` strings
- `tokio` (workspace) — async runtime
- `tracing = "0.1"` (workspace)
- `thiserror = "2"` (workspace)
- `serde = "1"` (workspace, with derive)

Dev-deps:
- `testcontainers = "0.23"` + `testcontainers-modules = "0.11"` with `postgres`, `mysql` features — for backend integration tests
- `tempfile` (workspace) — migrations directory fixtures
- `tokio` with `macros` + `rt-multi-thread`
- `anyhow`, `pretty_assertions` (workspace)
- `altair-otel = { path = "../altair-otel", version = "0.1" }` — for the `with_otel.rs` example
- `altair-config = { path = "../altair-config", version = "0.1" }` — for the `with_config.rs` example

---

## File Structure

```
crates/altair-db/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs        # crate root: lints, mod decls, re-exports (sea_orm, sqlx)
│   ├── error.rs      # Error enum + Result alias
│   ├── config.rs     # Config, Backend, Config::from_url, Config::backend, Default
│   ├── connect.rs    # build_options() pure mapping function
│   ├── db.rs         # Db struct + connect/orm/pools/backend/ping/migrate/transaction/close
│   └── prelude.rs    # one-import bundle
├── tests/
│   ├── sqlite.rs     # always-on integration via sqlite::memory: + tempfile
│   ├── postgres.rs   # gated by `postgres` feature + testcontainers
│   └── mysql.rs      # gated by `mysql` feature + testcontainers
└── examples/
    ├── basic_orm.rs
    ├── raw_sqlx.rs
    ├── materialized_view.rs
    ├── with_config.rs
    ├── with_otel.rs
    ├── migrations.rs
    └── transaction.rs
```

Workspace + repo edits:
- `Cargo.toml`: add `sea-orm`, `sqlx`, `humantime-serde`, `testcontainers`, `testcontainers-modules` to `[workspace.dependencies]`; add `"crates/altair-db"` to `members`
- `docs/porting-tracker.md`: move `altair-db` from "Awaiting Demand" → "Published crates" + Starter Set; add release-notes bullet
- `README.md`: add `altair-db` row to crate table
- `INSTRUCTION.md`: add a one-line entry for `altair-db` if the file lists crates

---

## Phase 1: Crate Scaffold

### Task 1.1: Add workspace dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root, `[workspace.dependencies]`)

- [ ] **Step 1: Add the new dependency block**

In root `Cargo.toml`, inside `[workspace.dependencies]`, append a new "# Database" section after the "# HTTP server" block:

```toml
# Database
sea-orm = { version = "1", default-features = false, features = [
    "runtime-tokio-rustls",
    "sqlx-postgres",
    "sqlx-mysql",
    "sqlx-sqlite",
    "macros",
] }
sqlx = { version = "0.8", default-features = false, features = [
    "runtime-tokio-rustls",
    "postgres",
    "mysql",
    "sqlite",
    "migrate",
] }
humantime-serde = "1"
testcontainers = "0.23"
testcontainers-modules = { version = "0.11", features = ["postgres", "mysql"] }
```

- [ ] **Step 2: Verify workspace parses**

Run: `cargo metadata --format-version=1 > /dev/null`
Expected: exit 0, no errors.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add sea-orm + sqlx + testcontainers to workspace dependencies"
```

### Task 1.2: Create crate skeleton

**Files:**
- Create: `crates/altair-db/Cargo.toml`
- Create: `crates/altair-db/src/lib.rs`
- Create: `crates/altair-db/README.md` (stub)
- Modify: `Cargo.toml` (workspace `members`)

- [ ] **Step 1: Create directories**

```bash
mkdir -p crates/altair-db/src crates/altair-db/tests crates/altair-db/examples
```

- [ ] **Step 2: Write `crates/altair-db/Cargo.toml`**

```toml
[package]
name = "altair-db"
description = "Sea-ORM + sqlx convenience layer with smart defaults, migrations, and OTel-aware tracing"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
homepage.workspace = true
readme = "README.md"
keywords = ["database", "sql", "sqlx", "sea-orm", "postgres"]
categories = ["database"]

[features]
default = ["postgres", "mysql", "sqlite"]
postgres = []
mysql = []
sqlite = []

[dependencies]
sea-orm = { workspace = true }
sqlx = { workspace = true }
serde = { workspace = true }
humantime-serde = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
anyhow = { workspace = true }
pretty_assertions = { workspace = true }
tempfile = { workspace = true }
testcontainers = { workspace = true }
testcontainers-modules = { workspace = true }
# Sibling crates for examples only.
altair-otel = { path = "../altair-otel", version = "0.1" }
altair-config = { path = "../altair-config", version = "0.1" }

[[example]]
name = "materialized_view"
required-features = ["postgres"]

[lints]
workspace = true
```

- [ ] **Step 3: Write minimal `crates/altair-db/src/lib.rs`**

This is a compile-only stub that allows the crate to be added to the workspace before any modules exist. Each subsequent task will add real code.

```rust
//! Sea-ORM + sqlx convenience layer.
//!
//! Wraps a `sea_orm::DatabaseConnection` (and its underlying sqlx pool) with
//! smart pool defaults, file-based migrations, OTel-aware query tracing, and
//! a closure-style transaction helper. Three backends (Postgres + MySQL +
//! SQLite) are supported behind cargo features.
//!
//! See the crate README for usage.

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

// Re-exports for one-dep ergonomics
pub use ::sea_orm;
pub use ::sqlx;
```

- [ ] **Step 4: Write stub README**

```markdown
# altair-db

Sea-ORM + sqlx convenience layer with smart defaults, migrations, and OTel-aware tracing.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

(Full README added in a later task.)
```

- [ ] **Step 5: Register in workspace `members`**

In root `Cargo.toml`, append `"crates/altair-db"` to the `members` list. After the edit the list should contain nine entries:

```toml
members = [
    "crates/altair-concurrent",
    "crates/altair-retry",
    "crates/altair-config",
    "crates/altair-otel",
    "crates/altair-base32",
    "crates/altair-compress",
    "crates/altair-rest",
    "crates/altair-server",
    "crates/altair-db",
]
```

- [ ] **Step 6: Verify the empty crate compiles**

Run: `cargo build -p altair-db`
Expected: clean build (only warnings from clippy::pedantic on the lone re-exports, if any).

- [ ] **Step 7: Commit**

```bash
git add crates/altair-db Cargo.toml
git commit -m "feat(db): scaffold altair-db crate"
```

---

## Phase 2: Error + Config (pure, no DB)

### Task 2.1: Error enum

**Files:**
- Create: `crates/altair-db/src/error.rs`
- Modify: `crates/altair-db/src/lib.rs` (add `mod error;` + re-exports)

- [ ] **Step 1: Write failing test for `From` conversions**

Append to `crates/altair-db/src/error.rs` (creating it):

```rust
//! Error type for altair-db.

use thiserror::Error;

/// All errors that may surface from `altair-db`.
#[derive(Debug, Error)]
pub enum Error {
    /// Could not establish a connection to the database.
    #[error("failed to connect to database")]
    Connect(#[source] sea_orm::DbErr),

    /// A schema migration could not be applied.
    #[error("migration failed")]
    Migration(#[from] sqlx::migrate::MigrateError),

    /// The supplied `Config` is invalid (e.g. empty URL, unsupported scheme).
    #[error("invalid configuration: {0}")]
    Configuration(String),

    /// A raw sqlx query failed.
    #[error("sql error")]
    Sql(#[from] sqlx::Error),

    /// An ORM operation failed at runtime.
    #[error("orm error")]
    Orm(#[source] sea_orm::DbErr),
}

/// Shorthand `Result` parameterised over the crate's `Error`.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_from_sqlx_migrate_error() {
        let raw = sqlx::migrate::MigrateError::Source(Box::<dyn std::error::Error + Send + Sync>::from(
            "boom".to_string(),
        ));
        let err: Error = raw.into();
        assert!(matches!(err, Error::Migration(_)));
    }

    #[test]
    fn sql_from_sqlx_error() {
        let raw = sqlx::Error::Protocol("oops".to_string());
        let err: Error = raw.into();
        assert!(matches!(err, Error::Sql(_)));
    }

    #[test]
    fn configuration_carries_message() {
        let err = Error::Configuration("url is required".to_string());
        assert_eq!(err.to_string(), "invalid configuration: url is required");
    }
}
```

- [ ] **Step 2: Wire into `lib.rs`**

Insert above the existing re-exports:

```rust
mod error;

pub use error::{Error, Result};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p altair-db --lib`
Expected: 3 tests in `error::tests` pass.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-db/src/error.rs crates/altair-db/src/lib.rs
git commit -m "feat(db): add Error enum with Connect, Migration, Configuration, Sql, Orm variants"
```

### Task 2.2: Config + Backend

**Files:**
- Create: `crates/altair-db/src/config.rs`
- Modify: `crates/altair-db/src/lib.rs` (add `mod config;` + re-exports)

- [ ] **Step 1: Write failing tests for `Backend` parsing and defaults**

Create `crates/altair-db/src/config.rs`:

```rust
//! Configuration types for altair-db.

use std::time::Duration;

use crate::error::{Error, Result};

/// Which database backend a `Config` refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// PostgreSQL.
    Postgres,
    /// MySQL / MariaDB.
    MySql,
    /// SQLite.
    Sqlite,
}

/// Connection + pool configuration for `Db`.
///
/// Field defaults: `max_connections=10`, `min_connections=1`,
/// `acquire_timeout=30s`, `idle_timeout=10m`, `max_lifetime=30m`,
/// `sqlx_logging=true`, `sqlx_slow_query_threshold=1s`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    /// Connection URL, e.g. `postgres://user:pass@host/db`.
    pub url: String,
    /// Maximum number of pooled connections.
    pub max_connections: u32,
    /// Minimum number of pooled connections kept warm.
    pub min_connections: u32,
    /// How long to wait for a connection from the pool before failing.
    #[serde(with = "humantime_serde")]
    pub acquire_timeout: Duration,
    /// Close idle connections after this duration (`None` = never).
    #[serde(with = "humantime_serde", default)]
    pub idle_timeout: Option<Duration>,
    /// Recycle connections older than this (`None` = never).
    #[serde(with = "humantime_serde", default)]
    pub max_lifetime: Option<Duration>,
    /// Whether sqlx should emit `tracing` events for every query.
    pub sqlx_logging: bool,
    /// Statements slower than this are logged at WARN level.
    #[serde(with = "humantime_serde")]
    pub sqlx_slow_query_threshold: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            url: String::new(),
            max_connections: 10,
            min_connections: 1,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(600)),
            max_lifetime: Some(Duration::from_secs(1800)),
            sqlx_logging: true,
            sqlx_slow_query_threshold: Duration::from_secs(1),
        }
    }
}

impl Config {
    /// Build a `Config` with default pool tuning and the given URL.
    pub fn from_url(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..Self::default()
        }
    }

    /// Parse the URL scheme into a `Backend`.
    ///
    /// Errors with `Error::Configuration` if the URL is empty or the scheme
    /// is not one of `postgres://`, `postgresql://`, `mysql://`, `sqlite://`.
    pub fn backend(&self) -> Result<Backend> {
        let url = self.url.trim();
        if url.is_empty() {
            return Err(Error::Configuration("url is required".to_string()));
        }
        let scheme = url
            .split_once(':')
            .map(|(s, _)| s)
            .unwrap_or(url)
            .to_ascii_lowercase();
        match scheme.as_str() {
            "postgres" | "postgresql" => Ok(Backend::Postgres),
            "mysql" => Ok(Backend::MySql),
            "sqlite" => Ok(Backend::Sqlite),
            other => Err(Error::Configuration(format!(
                "unsupported url scheme: {other}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_url_keeps_defaults() {
        let c = Config::from_url("postgres://localhost/x");
        assert_eq!(c.url, "postgres://localhost/x");
        assert_eq!(c.max_connections, 10);
        assert_eq!(c.acquire_timeout, Duration::from_secs(30));
        assert_eq!(c.idle_timeout, Some(Duration::from_secs(600)));
    }

    #[test]
    fn backend_postgres() {
        assert_eq!(
            Config::from_url("postgres://x/y").backend().unwrap(),
            Backend::Postgres
        );
        assert_eq!(
            Config::from_url("postgresql://x/y").backend().unwrap(),
            Backend::Postgres
        );
    }

    #[test]
    fn backend_mysql() {
        assert_eq!(
            Config::from_url("mysql://x/y").backend().unwrap(),
            Backend::MySql
        );
    }

    #[test]
    fn backend_sqlite() {
        assert_eq!(
            Config::from_url("sqlite::memory:").backend().unwrap(),
            Backend::Sqlite
        );
        assert_eq!(
            Config::from_url("sqlite:///tmp/x.db").backend().unwrap(),
            Backend::Sqlite
        );
    }

    #[test]
    fn backend_rejects_unknown_scheme() {
        let err = Config::from_url("mongodb://x").backend().unwrap_err();
        assert!(matches!(err, Error::Configuration(_)));
    }

    #[test]
    fn backend_rejects_empty_url() {
        let err = Config::default().backend().unwrap_err();
        assert!(matches!(err, Error::Configuration(_)));
    }

    #[test]
    fn humantime_round_trip() {
        let toml_src = r#"
url = "postgres://localhost/x"
acquire_timeout = "45s"
idle_timeout = "5m"
max_lifetime = "1h"
sqlx_slow_query_threshold = "750ms"
"#;
        let cfg: Config = toml::from_str(toml_src).unwrap();
        assert_eq!(cfg.acquire_timeout, Duration::from_secs(45));
        assert_eq!(cfg.idle_timeout, Some(Duration::from_secs(300)));
        assert_eq!(cfg.max_lifetime, Some(Duration::from_secs(3600)));
        assert_eq!(
            cfg.sqlx_slow_query_threshold,
            Duration::from_millis(750)
        );
    }
}
```

- [ ] **Step 2: Add `toml` to dev-dependencies for the test**

In `crates/altair-db/Cargo.toml` under `[dev-dependencies]`, add:

```toml
toml = { workspace = true }
```

- [ ] **Step 3: Wire `config` into `lib.rs`**

Insert just below `mod error;`:

```rust
mod config;

pub use config::{Backend, Config};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p altair-db --lib config::`
Expected: 7 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/altair-db/src/config.rs crates/altair-db/src/lib.rs crates/altair-db/Cargo.toml
git commit -m "feat(db): add Config + Backend with serde, humantime, and scheme parsing"
```

### Task 2.3: `build_options` pure mapping

**Files:**
- Create: `crates/altair-db/src/connect.rs`
- Modify: `crates/altair-db/src/lib.rs` (add `mod connect;`)

- [ ] **Step 1: Write failing test for `build_options`**

Create `crates/altair-db/src/connect.rs`:

```rust
//! Pure mapping from `Config` to `sea_orm::ConnectOptions`.

use sea_orm::ConnectOptions;

use crate::config::Config;

/// Translate a `Config` into a `sea_orm::ConnectOptions`.
///
/// Pure function — does not open a connection. Centralises the
/// "every Config knob maps to the right ConnectOptions field" logic so it
/// can be unit-tested without a live database.
pub(crate) fn build_options(config: &Config) -> ConnectOptions {
    let mut opt = ConnectOptions::new(config.url.clone());
    opt.max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(config.acquire_timeout)
        .sqlx_logging(config.sqlx_logging)
        .sqlx_slow_statements_logging_threshold(config.sqlx_slow_query_threshold);
    if let Some(d) = config.idle_timeout {
        opt.idle_timeout(d);
    }
    if let Some(d) = config.max_lifetime {
        opt.max_lifetime(d);
    }
    opt
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn sample_config() -> Config {
        Config {
            url: "sqlite::memory:".to_string(),
            max_connections: 25,
            min_connections: 3,
            acquire_timeout: Duration::from_secs(7),
            idle_timeout: Some(Duration::from_secs(120)),
            max_lifetime: Some(Duration::from_secs(900)),
            sqlx_logging: false,
            sqlx_slow_query_threshold: Duration::from_millis(250),
        }
    }

    #[test]
    fn maps_url() {
        let opts = build_options(&sample_config());
        assert_eq!(opts.get_url(), "sqlite::memory:");
    }

    #[test]
    fn maps_pool_sizes() {
        let opts = build_options(&sample_config());
        assert_eq!(opts.get_max_connections(), Some(25));
        assert_eq!(opts.get_min_connections(), Some(3));
    }

    #[test]
    fn maps_timeouts() {
        let opts = build_options(&sample_config());
        assert_eq!(opts.get_acquire_timeout(), Some(Duration::from_secs(7)));
        assert_eq!(opts.get_idle_timeout(), Some(Duration::from_secs(120)));
        assert_eq!(opts.get_max_lifetime(), Some(Duration::from_secs(900)));
    }

    #[test]
    fn maps_logging_flag() {
        let opts = build_options(&sample_config());
        assert!(!opts.get_sqlx_logging());
    }

    #[test]
    fn optional_timeouts_default_to_none_when_unset() {
        let mut cfg = sample_config();
        cfg.idle_timeout = None;
        cfg.max_lifetime = None;
        let opts = build_options(&cfg);
        assert_eq!(opts.get_idle_timeout(), None);
        assert_eq!(opts.get_max_lifetime(), None);
    }
}
```

> **Note:** the exact getter method names on `sea_orm::ConnectOptions` (`get_url`, `get_max_connections`, etc.) come from sea-orm 1.x. If the API differs in the resolved version, replace each assertion with an equivalent inspection (e.g. pattern-match on a `Debug` snapshot or call `clone()` and re-extract). The test scope is "every Config field is mapped"; the assertion mechanism is incidental.

- [ ] **Step 2: Wire `connect` into `lib.rs`**

Insert below `mod config;`:

```rust
mod connect;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p altair-db --lib connect::`
Expected: 5 tests pass (`maps_url`, `maps_pool_sizes`, `maps_timeouts`, `maps_logging_flag`, `optional_timeouts_default_to_none_when_unset`).

- [ ] **Step 4: Commit**

```bash
git add crates/altair-db/src/connect.rs crates/altair-db/src/lib.rs
git commit -m "feat(db): add build_options pure mapping from Config to ConnectOptions"
```

---

## Phase 3: Db handle

### Task 3.1: `Db::connect` + `orm()` + `backend()`

**Files:**
- Create: `crates/altair-db/src/db.rs`
- Modify: `crates/altair-db/src/lib.rs` (add `mod db;` + re-export `Db`)

- [ ] **Step 1: Write `db.rs` with `connect`, `orm`, and `backend`**

```rust
//! `Db`: the single handle that owns a SeaORM connection and exposes both
//! the ORM layer and the raw sqlx pool.

use sea_orm::{Database, DatabaseConnection};

use crate::config::{Backend, Config};
use crate::connect::build_options;
use crate::error::{Error, Result};

/// A connected database handle.
#[derive(Debug, Clone)]
pub struct Db {
    conn: DatabaseConnection,
}

impl Db {
    /// Open a connection pool from a [`Config`].
    ///
    /// Validates the URL is non-empty, builds `ConnectOptions`, then calls
    /// `sea_orm::Database::connect`. The returned `Db` owns the pool.
    pub async fn connect(config: Config) -> Result<Self> {
        if config.url.trim().is_empty() {
            return Err(Error::Configuration("url is required".to_string()));
        }
        let opts = build_options(&config);
        let conn = Database::connect(opts).await.map_err(Error::Connect)?;
        Ok(Self { conn })
    }

    /// The SeaORM connection — use for ORM-style CRUD.
    #[must_use]
    pub fn orm(&self) -> &DatabaseConnection {
        &self.conn
    }

    /// Which backend this `Db` is connected to.
    #[must_use]
    pub fn backend(&self) -> Backend {
        match self.conn.get_database_backend() {
            sea_orm::DatabaseBackend::Postgres => Backend::Postgres,
            sea_orm::DatabaseBackend::MySql => Backend::MySql,
            sea_orm::DatabaseBackend::Sqlite => Backend::Sqlite,
        }
    }
}
```

- [ ] **Step 2: Wire `db` into `lib.rs`**

Insert below `mod connect;`:

```rust
mod db;

pub use db::Db;
```

- [ ] **Step 3: Write an in-memory smoke test**

Append to `crates/altair-db/src/db.rs`:

```rust
#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connect_to_sqlite_memory() {
        let db = Db::connect(Config::from_url("sqlite::memory:")).await.unwrap();
        assert_eq!(db.backend(), Backend::Sqlite);
        assert!(matches!(
            db.orm().get_database_backend(),
            sea_orm::DatabaseBackend::Sqlite
        ));
    }

    #[tokio::test]
    async fn connect_rejects_empty_url() {
        let err = Db::connect(Config::default()).await.unwrap_err();
        assert!(matches!(err, Error::Configuration(_)));
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p altair-db --lib db::`
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/altair-db/src/db.rs crates/altair-db/src/lib.rs
git commit -m "feat(db): add Db with connect, orm, and backend accessors"
```

### Task 3.2: Pool accessors

**Files:**
- Modify: `crates/altair-db/src/db.rs`

- [ ] **Step 1: Add per-backend pool accessors**

Insert into the `impl Db` block in `crates/altair-db/src/db.rs`, after `backend()`:

```rust
    /// Raw sqlx Postgres pool, if this `Db` is connected to Postgres.
    #[cfg(feature = "postgres")]
    #[must_use]
    pub fn pg_pool(&self) -> Option<&sqlx::PgPool> {
        match &self.conn {
            DatabaseConnection::SqlxPostgresPoolConnection(c) => Some(c.get_postgres_connection_pool()),
            _ => None,
        }
    }

    /// Raw sqlx MySQL pool, if this `Db` is connected to MySQL.
    #[cfg(feature = "mysql")]
    #[must_use]
    pub fn mysql_pool(&self) -> Option<&sqlx::MySqlPool> {
        match &self.conn {
            DatabaseConnection::SqlxMySqlPoolConnection(c) => Some(c.get_mysql_connection_pool()),
            _ => None,
        }
    }

    /// Raw sqlx SQLite pool, if this `Db` is connected to SQLite.
    #[cfg(feature = "sqlite")]
    #[must_use]
    pub fn sqlite_pool(&self) -> Option<&sqlx::SqlitePool> {
        match &self.conn {
            DatabaseConnection::SqlxSqlitePoolConnection(c) => Some(c.get_sqlite_connection_pool()),
            _ => None,
        }
    }
```

> **Note:** the inner connection variants and their `get_*_connection_pool` accessors are from sea-orm 1.x. If the version resolved during `cargo build` exposes them differently (e.g. as methods on `DatabaseConnection` itself rather than on inner variants), keep the same public signature on `Db` and adjust the match/expression accordingly. The public surface (`Option<&PgPool>` etc.) is the contract.

- [ ] **Step 2: Add accessor smoke test**

Extend the existing `mod tests` block in `db.rs`:

```rust
    #[tokio::test]
    async fn sqlite_pool_present_postgres_absent() {
        let db = Db::connect(Config::from_url("sqlite::memory:")).await.unwrap();
        assert!(db.sqlite_pool().is_some());
        #[cfg(feature = "postgres")]
        assert!(db.pg_pool().is_none());
        #[cfg(feature = "mysql")]
        assert!(db.mysql_pool().is_none());
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p altair-db --lib db::`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-db/src/db.rs
git commit -m "feat(db): add per-backend sqlx pool accessors"
```

### Task 3.3: `ping()` + `close()`

**Files:**
- Modify: `crates/altair-db/src/db.rs`

- [ ] **Step 1: Add `ping` and `close`**

Insert into `impl Db`:

```rust
    /// Run `SELECT 1` to verify the pool is alive.
    pub async fn ping(&self) -> Result<()> {
        use sea_orm::{ConnectionTrait, Statement};
        let backend = self.conn.get_database_backend();
        let stmt = Statement::from_string(backend, "SELECT 1".to_string());
        self.conn.execute(stmt).await.map_err(Error::Orm)?;
        Ok(())
    }

    /// Close the connection pool. Subsequent calls to other methods will fail.
    pub async fn close(self) -> Result<()> {
        self.conn.close().await.map_err(Error::Orm)?;
        Ok(())
    }
```

- [ ] **Step 2: Add tests**

In the `mod tests` block:

```rust
    #[tokio::test]
    async fn ping_sqlite_memory() {
        let db = Db::connect(Config::from_url("sqlite::memory:")).await.unwrap();
        db.ping().await.unwrap();
    }

    #[tokio::test]
    async fn close_consumes_db() {
        let db = Db::connect(Config::from_url("sqlite::memory:")).await.unwrap();
        db.close().await.unwrap();
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p altair-db --lib db::`
Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-db/src/db.rs
git commit -m "feat(db): add ping() health probe and close() lifecycle method"
```

### Task 3.4: `migrate()`

**Files:**
- Modify: `crates/altair-db/src/db.rs`

- [ ] **Step 1: Add `migrate`**

Insert into `impl Db`, below `close()`:

```rust
    /// Run all sqlx migrations under the given directory against the
    /// underlying pool. Tracks applied versions in `_sqlx_migrations`.
    ///
    /// Files must follow sqlx's naming convention:
    /// `<timestamp>_<name>.sql`, e.g. `20260101000000_create_users.sql`.
    pub async fn migrate(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let migrator = sqlx::migrate::Migrator::new(path.as_ref()).await?;
        match self.backend() {
            #[cfg(feature = "postgres")]
            Backend::Postgres => {
                let pool = self
                    .pg_pool()
                    .ok_or_else(|| Error::Configuration("postgres pool missing".to_string()))?;
                migrator.run(pool).await?;
            }
            #[cfg(feature = "mysql")]
            Backend::MySql => {
                let pool = self
                    .mysql_pool()
                    .ok_or_else(|| Error::Configuration("mysql pool missing".to_string()))?;
                migrator.run(pool).await?;
            }
            #[cfg(feature = "sqlite")]
            Backend::Sqlite => {
                let pool = self
                    .sqlite_pool()
                    .ok_or_else(|| Error::Configuration("sqlite pool missing".to_string()))?;
                migrator.run(pool).await?;
            }
        }
        Ok(())
    }
```

- [ ] **Step 2: Add a tempdir-based smoke test**

In the `mod tests` block, append:

```rust
    #[tokio::test]
    async fn migrate_applies_files() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let mut f = std::fs::File::create(dir.path().join("20260101000000_init.sql")).unwrap();
        writeln!(f, "CREATE TABLE widgets (id INTEGER PRIMARY KEY, name TEXT NOT NULL);").unwrap();
        drop(f);

        let db = Db::connect(Config::from_url("sqlite::memory:")).await.unwrap();
        db.migrate(dir.path()).await.unwrap();

        // Verify the migration ran by writing a row.
        use sea_orm::{ConnectionTrait, Statement};
        let stmt = Statement::from_string(
            db.orm().get_database_backend(),
            "INSERT INTO widgets (name) VALUES ('test')".to_string(),
        );
        db.orm().execute(stmt).await.unwrap();
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p altair-db --lib db::`
Expected: 6 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-db/src/db.rs
git commit -m "feat(db): add migrate() over sqlx::migrate::Migrator with backend dispatch"
```

### Task 3.5: `transaction()`

**Files:**
- Modify: `crates/altair-db/src/db.rs`

- [ ] **Step 1: Add `transaction`**

Insert into `impl Db`, below `migrate()`:

```rust
    /// Run a closure inside a SeaORM transaction.
    ///
    /// The closure receives `&DatabaseTransaction`, which implements
    /// `ConnectionTrait`, so all SeaORM operations work. Returning `Ok`
    /// commits; returning `Err` rolls back.
    ///
    /// The error type is `sea_orm::TransactionError<E>`, re-exported by
    /// SeaORM and preserving the closure's `E` faithfully.
    pub async fn transaction<F, T, E>(
        &self,
        f: F,
    ) -> std::result::Result<T, sea_orm::TransactionError<E>>
    where
        F: for<'c> FnOnce(
                &'c sea_orm::DatabaseTransaction,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = std::result::Result<T, E>> + Send + 'c>,
            > + Send,
        T: Send,
        E: std::error::Error + Send + Sync + 'static,
    {
        self.conn.transaction(f).await
    }
```

- [ ] **Step 2: Add commit + rollback tests**

In the `mod tests` block, append:

```rust
    #[tokio::test]
    async fn transaction_commit() {
        use sea_orm::{ConnectionTrait, Statement};

        let db = Db::connect(Config::from_url("sqlite::memory:")).await.unwrap();
        let backend = db.orm().get_database_backend();
        db.orm()
            .execute(Statement::from_string(
                backend,
                "CREATE TABLE k (n INTEGER)".to_string(),
            ))
            .await
            .unwrap();

        db.transaction(|tx| {
            Box::pin(async move {
                tx.execute(Statement::from_string(
                    sea_orm::DatabaseBackend::Sqlite,
                    "INSERT INTO k (n) VALUES (1)".to_string(),
                ))
                .await
                .map(|_| ())
            })
        })
        .await
        .unwrap();

        let res = db
            .orm()
            .query_one(Statement::from_string(
                backend,
                "SELECT COUNT(*) AS c FROM k".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        let count: i64 = res.try_get("", "c").unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn transaction_rollback_on_err() {
        use sea_orm::{ConnectionTrait, Statement};

        let db = Db::connect(Config::from_url("sqlite::memory:")).await.unwrap();
        let backend = db.orm().get_database_backend();
        db.orm()
            .execute(Statement::from_string(
                backend,
                "CREATE TABLE k (n INTEGER)".to_string(),
            ))
            .await
            .unwrap();

        let res: std::result::Result<(), sea_orm::TransactionError<sea_orm::DbErr>> = db
            .transaction(|tx| {
                Box::pin(async move {
                    tx.execute(Statement::from_string(
                        sea_orm::DatabaseBackend::Sqlite,
                        "INSERT INTO k (n) VALUES (1)".to_string(),
                    ))
                    .await?;
                    Err(sea_orm::DbErr::Custom("simulated failure".to_string()))
                })
            })
            .await;
        assert!(res.is_err());

        let res = db
            .orm()
            .query_one(Statement::from_string(
                backend,
                "SELECT COUNT(*) AS c FROM k".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        let count: i64 = res.try_get("", "c").unwrap();
        assert_eq!(count, 0, "row inserted before err should have rolled back");
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p altair-db --lib db::`
Expected: 8 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-db/src/db.rs
git commit -m "feat(db): add transaction() closure helper over SeaORM transaction"
```

---

## Phase 4: Prelude

### Task 4.1: Prelude module

**Files:**
- Create: `crates/altair-db/src/prelude.rs`
- Modify: `crates/altair-db/src/lib.rs` (add `pub mod prelude;`)

- [ ] **Step 1: Write `prelude.rs`**

```rust
//! Convenience re-exports — one `use altair_db::prelude::*;` is enough
//! to write straightforward CRUD against the database.

pub use crate::{Backend, Config, Db, Error, Result};

pub use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
};
```

- [ ] **Step 2: Wire into `lib.rs`**

After the `pub use db::Db;` line, add:

```rust
pub mod prelude;
```

- [ ] **Step 3: Verify**

Run: `cargo build -p altair-db`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add crates/altair-db/src/prelude.rs crates/altair-db/src/lib.rs
git commit -m "feat(db): add prelude bundling Db, Config, Error, and SeaORM CRUD traits"
```

---

## Phase 5: Integration tests

### Task 5.1: SQLite integration tests (always-on)

**Files:**
- Create: `crates/altair-db/tests/sqlite.rs`

- [ ] **Step 1: Write `tests/sqlite.rs`**

```rust
//! Integration tests that exercise the full `Db` surface over SQLite.
//!
//! Always runs in CI (no infrastructure required).

#![cfg(feature = "sqlite")]

use std::io::Write;

use altair_db::prelude::*;
use sea_orm::{ConnectionTrait, Statement};

async fn connect() -> Db {
    Db::connect(Config::from_url("sqlite::memory:"))
        .await
        .expect("sqlite::memory: should connect")
}

#[tokio::test]
async fn connects_and_reports_backend() {
    let db = connect().await;
    assert_eq!(db.backend(), Backend::Sqlite);
}

#[tokio::test]
async fn ping_succeeds() {
    let db = connect().await;
    db.ping().await.unwrap();
}

#[tokio::test]
async fn migrate_runs_and_tracks_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut f = std::fs::File::create(dir.path().join("20260101000000_init.sql")).unwrap();
    writeln!(
        f,
        "CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT NOT NULL);"
    )
    .unwrap();
    let mut g = std::fs::File::create(dir.path().join("20260102000000_add_seen.sql")).unwrap();
    writeln!(g, "ALTER TABLE notes ADD COLUMN seen BOOLEAN DEFAULT 0;").unwrap();
    drop((f, g));

    let db = connect().await;
    db.migrate(dir.path()).await.unwrap();

    let stmt = Statement::from_string(
        db.orm().get_database_backend(),
        "SELECT version FROM _sqlx_migrations ORDER BY version".to_string(),
    );
    let rows = db.orm().query_all(stmt).await.unwrap();
    assert_eq!(rows.len(), 2);
}

#[tokio::test]
async fn raw_sqlx_query_via_pool() {
    let db = connect().await;
    let pool = db.sqlite_pool().expect("sqlite pool");
    let (one,): (i64,) = sqlx::query_as("SELECT 1").fetch_one(pool).await.unwrap();
    assert_eq!(one, 1);
}

#[tokio::test]
async fn transaction_commits_and_rolls_back() {
    let db = connect().await;
    let backend = db.orm().get_database_backend();
    db.orm()
        .execute(Statement::from_string(
            backend,
            "CREATE TABLE t (n INTEGER)".to_string(),
        ))
        .await
        .unwrap();

    // commit
    db.transaction::<_, (), sea_orm::DbErr>(|tx| {
        Box::pin(async move {
            tx.execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "INSERT INTO t (n) VALUES (1)".to_string(),
            ))
            .await
            .map(|_| ())
        })
    })
    .await
    .unwrap();

    // rollback
    let _ = db
        .transaction::<_, (), sea_orm::DbErr>(|tx| {
            Box::pin(async move {
                tx.execute(Statement::from_string(
                    sea_orm::DatabaseBackend::Sqlite,
                    "INSERT INTO t (n) VALUES (2)".to_string(),
                ))
                .await?;
                Err(sea_orm::DbErr::Custom("nope".to_string()))
            })
        })
        .await;

    let row = db
        .orm()
        .query_one(Statement::from_string(
            backend,
            "SELECT COUNT(*) AS c FROM t".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    let count: i64 = row.try_get("", "c").unwrap();
    assert_eq!(count, 1, "only the committed insert should remain");
}
```

- [ ] **Step 2: Run**

Run: `cargo test -p altair-db --test sqlite`
Expected: 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-db/tests/sqlite.rs
git commit -m "test(db): add SQLite integration covering connect/ping/migrate/raw/transaction"
```

### Task 5.2: Postgres integration via testcontainers

**Files:**
- Create: `crates/altair-db/tests/postgres.rs`

- [ ] **Step 1: Write `tests/postgres.rs`**

```rust
//! Postgres integration via testcontainers.
//!
//! Skipped on non-Linux because Docker availability is unreliable on
//! macOS/Windows CI runners.

#![cfg(all(feature = "postgres", target_os = "linux"))]

use altair_db::prelude::*;
use sea_orm::{ConnectionTrait, Statement};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

async fn start() -> (Db, testcontainers::ContainerAsync<Postgres>) {
    let container = Postgres::default().start().await.expect("start postgres");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("postgres port");
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let db = Db::connect(Config::from_url(url)).await.expect("connect");
    (db, container)
}

#[tokio::test]
async fn connects_and_reports_postgres_backend() {
    let (db, _c) = start().await;
    assert_eq!(db.backend(), Backend::Postgres);
    db.ping().await.unwrap();
}

#[tokio::test]
async fn materialized_view_round_trip_via_raw_sqlx() {
    let (db, _c) = start().await;
    let pool = db.pg_pool().expect("pg pool");

    sqlx::query("CREATE TABLE orders (id SERIAL PRIMARY KEY, amount NUMERIC NOT NULL)")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO orders (amount) VALUES (10), (20), (30)")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE MATERIALIZED VIEW order_totals AS \
         SELECT COUNT(*) AS n, SUM(amount) AS total FROM orders",
    )
    .execute(pool)
    .await
    .unwrap();

    let (n, total): (i64, sqlx::types::BigDecimal) =
        sqlx::query_as("SELECT n, total FROM order_totals")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(n, 3);
    assert_eq!(total.to_string(), "60");
}

#[tokio::test]
async fn orm_crud_round_trip() {
    let (db, _c) = start().await;
    let backend = db.orm().get_database_backend();
    db.orm()
        .execute(Statement::from_string(
            backend,
            "CREATE TABLE widgets (id SERIAL PRIMARY KEY, name TEXT NOT NULL)".to_string(),
        ))
        .await
        .unwrap();
    db.orm()
        .execute(Statement::from_string(
            backend,
            "INSERT INTO widgets (name) VALUES ('a'), ('b')".to_string(),
        ))
        .await
        .unwrap();
    let row = db
        .orm()
        .query_one(Statement::from_string(
            backend,
            "SELECT COUNT(*) AS c FROM widgets".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    let count: i64 = row.try_get("", "c").unwrap();
    assert_eq!(count, 2);
}
```

> **Note:** `sqlx::types::BigDecimal` requires `sqlx` feature `bigdecimal`. If the workspace `sqlx` does not enable it, replace the materialised-view assertion with a string-cast: `SELECT n, total::text AS total FROM order_totals` and decode `total` as `String`. The test still proves the round-trip.

- [ ] **Step 2: Run (Linux only)**

On Linux with Docker available:

Run: `cargo test -p altair-db --test postgres -- --test-threads=1`
Expected: 3 tests pass. (Other platforms: file is excluded by `#![cfg(target_os = "linux")]`.)

- [ ] **Step 3: Commit**

```bash
git add crates/altair-db/tests/postgres.rs
git commit -m "test(db): add Postgres integration via testcontainers covering raw sqlx + materialised view"
```

### Task 5.3: MySQL integration via testcontainers

**Files:**
- Create: `crates/altair-db/tests/mysql.rs`

- [ ] **Step 1: Write `tests/mysql.rs`**

```rust
//! MySQL integration via testcontainers.
//!
//! Skipped on non-Linux for the same Docker-availability reason as
//! Postgres tests.

#![cfg(all(feature = "mysql", target_os = "linux"))]

use altair_db::prelude::*;
use sea_orm::{ConnectionTrait, Statement};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mysql::Mysql;

async fn start() -> (Db, testcontainers::ContainerAsync<Mysql>) {
    let container = Mysql::default().start().await.expect("start mysql");
    let port = container.get_host_port_ipv4(3306).await.expect("mysql port");
    let url = format!("mysql://root@127.0.0.1:{port}/test");
    let db = Db::connect(Config::from_url(url)).await.expect("connect");
    (db, container)
}

#[tokio::test]
async fn connects_and_reports_mysql_backend() {
    let (db, _c) = start().await;
    assert_eq!(db.backend(), Backend::MySql);
    db.ping().await.unwrap();
}

#[tokio::test]
async fn raw_sqlx_query_via_pool() {
    let (db, _c) = start().await;
    let pool = db.mysql_pool().expect("mysql pool");
    let (one,): (i64,) = sqlx::query_as("SELECT 1").fetch_one(pool).await.unwrap();
    assert_eq!(one, 1);
}

#[tokio::test]
async fn orm_round_trip() {
    let (db, _c) = start().await;
    let backend = db.orm().get_database_backend();
    db.orm()
        .execute(Statement::from_string(
            backend,
            "CREATE TABLE widgets (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(64) NOT NULL)"
                .to_string(),
        ))
        .await
        .unwrap();
    db.orm()
        .execute(Statement::from_string(
            backend,
            "INSERT INTO widgets (name) VALUES ('a'), ('b'), ('c')".to_string(),
        ))
        .await
        .unwrap();
    let row = db
        .orm()
        .query_one(Statement::from_string(
            backend,
            "SELECT COUNT(*) AS c FROM widgets".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    let count: i64 = row.try_get("", "c").unwrap();
    assert_eq!(count, 3);
}
```

- [ ] **Step 2: Run (Linux only)**

Run: `cargo test -p altair-db --test mysql -- --test-threads=1`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-db/tests/mysql.rs
git commit -m "test(db): add MySQL integration via testcontainers"
```

---

## Phase 6: Examples

### Task 6.1: `basic_orm.rs` + `raw_sqlx.rs` + `migrations.rs`

**Files:**
- Create: `crates/altair-db/examples/basic_orm.rs`
- Create: `crates/altair-db/examples/raw_sqlx.rs`
- Create: `crates/altair-db/examples/migrations.rs`

- [ ] **Step 1: Write `examples/basic_orm.rs`**

```rust
//! SeaORM CRUD round-trip against SQLite.
//!
//! Run with: `cargo run --example basic_orm -p altair-db`

use altair_db::prelude::*;
use sea_orm::{ConnectionTrait, Statement};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = Db::connect(Config::from_url("sqlite::memory:")).await?;

    // Bootstrap a tiny schema with raw SQL so the example is self-contained.
    let backend = db.orm().get_database_backend();
    db.orm()
        .execute(Statement::from_string(
            backend,
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)".to_string(),
        ))
        .await?;

    // ORM-shaped insert + select. Using raw Statements here keeps the example
    // free of an Entity macro; see the sea-orm docs for the typed Entity flow.
    db.orm()
        .execute(Statement::from_string(
            backend,
            "INSERT INTO users (name) VALUES ('alice'), ('bob')".to_string(),
        ))
        .await?;
    let row = db
        .orm()
        .query_one(Statement::from_string(
            backend,
            "SELECT COUNT(*) AS c FROM users".to_string(),
        ))
        .await?
        .expect("count");
    let count: i64 = row.try_get("", "c")?;
    println!("users: {count}");

    Ok(())
}
```

- [ ] **Step 2: Write `examples/raw_sqlx.rs`**

```rust
//! Reach for raw sqlx via the pool for power-user queries.
//!
//! Run with: `cargo run --example raw_sqlx -p altair-db`

use altair_db::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = Db::connect(Config::from_url("sqlite::memory:")).await?;
    let pool = db.sqlite_pool().expect("sqlite pool");

    sqlx::query("CREATE TABLE events (ts INTEGER NOT NULL, kind TEXT NOT NULL)")
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO events (ts, kind) VALUES (?1, ?2), (?3, ?4)")
        .bind(1)
        .bind("login")
        .bind(2)
        .bind("logout")
        .execute(pool)
        .await?;

    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT ts, kind FROM events ORDER BY ts")
        .fetch_all(pool)
        .await?;
    for (ts, kind) in rows {
        println!("ts={ts} kind={kind}");
    }
    Ok(())
}
```

- [ ] **Step 3: Write `examples/migrations.rs`**

```rust
//! Run a set of SQL migrations from an in-process tempdir.
//!
//! Run with: `cargo run --example migrations -p altair-db`

use std::io::Write;

use altair_db::prelude::*;
use sea_orm::{ConnectionTrait, Statement};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut f = std::fs::File::create(dir.path().join("20260101000000_create_widgets.sql"))?;
    writeln!(
        f,
        "CREATE TABLE widgets (id INTEGER PRIMARY KEY, name TEXT NOT NULL);"
    )?;
    let mut g = std::fs::File::create(dir.path().join("20260102000000_add_qty.sql"))?;
    writeln!(g, "ALTER TABLE widgets ADD COLUMN qty INTEGER DEFAULT 0;")?;
    drop((f, g));

    let db = Db::connect(Config::from_url("sqlite::memory:")).await?;
    db.migrate(dir.path()).await?;

    let backend = db.orm().get_database_backend();
    let row = db
        .orm()
        .query_one(Statement::from_string(
            backend,
            "SELECT COUNT(*) AS c FROM _sqlx_migrations".to_string(),
        ))
        .await?
        .expect("count");
    let n: i64 = row.try_get("", "c")?;
    println!("applied {n} migrations");
    Ok(())
}
```

- [ ] **Step 4: Build all three**

Run: `cargo build -p altair-db --examples`
Expected: clean build.

- [ ] **Step 5: Smoke-run**

```bash
cargo run --example basic_orm -p altair-db
cargo run --example raw_sqlx -p altair-db
cargo run --example migrations -p altair-db
```

Expected: each prints output and exits 0.

- [ ] **Step 6: Commit**

```bash
git add crates/altair-db/examples/basic_orm.rs crates/altair-db/examples/raw_sqlx.rs crates/altair-db/examples/migrations.rs
git commit -m "docs(db): add basic_orm, raw_sqlx, and migrations examples"
```

### Task 6.2: `materialized_view.rs` (Postgres-only)

**Files:**
- Create: `crates/altair-db/examples/materialized_view.rs`

- [ ] **Step 1: Write the example**

```rust
//! Postgres-only example: materialised view via raw sqlx.
//!
//! Run with a Postgres URL on stdin:
//!
//!   DATABASE_URL=postgres://user:pass@host/db \
//!     cargo run --example materialized_view -p altair-db --features postgres

use altair_db::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/postgres".to_string());
    let db = Db::connect(Config::from_url(url)).await?;
    assert_eq!(db.backend(), Backend::Postgres);

    let pool = db.pg_pool().expect("pg pool");

    sqlx::query("DROP MATERIALIZED VIEW IF EXISTS order_totals")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DROP TABLE IF EXISTS orders")
        .execute(pool)
        .await
        .ok();
    sqlx::query("CREATE TABLE orders (id SERIAL PRIMARY KEY, amount NUMERIC NOT NULL)")
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO orders (amount) VALUES (10), (20), (30)")
        .execute(pool)
        .await?;
    sqlx::query(
        "CREATE MATERIALIZED VIEW order_totals AS \
         SELECT COUNT(*) AS n, SUM(amount) AS total FROM orders",
    )
    .execute(pool)
    .await?;
    sqlx::query("REFRESH MATERIALIZED VIEW order_totals")
        .execute(pool)
        .await?;

    let (n, total): (i64, String) =
        sqlx::query_as("SELECT n, total::text FROM order_totals")
            .fetch_one(pool)
            .await?;
    println!("count={n} total={total}");
    Ok(())
}
```

- [ ] **Step 2: Verify it builds**

Run: `cargo build -p altair-db --example materialized_view --features postgres`
Expected: clean build (no run — requires live Postgres).

- [ ] **Step 3: Commit**

```bash
git add crates/altair-db/examples/materialized_view.rs
git commit -m "docs(db): add materialized_view Postgres example via raw sqlx"
```

### Task 6.3: `with_config.rs` + `transaction.rs`

**Files:**
- Create: `crates/altair-db/examples/with_config.rs`
- Create: `crates/altair-db/examples/transaction.rs`

- [ ] **Step 1: Write `examples/with_config.rs`**

```rust
//! Load `altair_db::Config` from a TOML file via altair-config.
//!
//! Run with: `cargo run --example with_config -p altair-db`
//!
//! Requires a `db_example.toml` next to the binary (created at runtime here
//! for self-containment).

use std::io::Write;

use altair_db::prelude::*;

#[derive(serde::Deserialize)]
struct AppConfig {
    database: Config,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("db_example.toml");
    let mut f = std::fs::File::create(&path)?;
    writeln!(
        f,
        r#"[database]
url = "sqlite::memory:"
max_connections = 5
acquire_timeout = "5s"
sqlx_slow_query_threshold = "500ms"
"#
    )?;
    drop(f);

    let raw = std::fs::read_to_string(&path)?;
    let cfg: AppConfig = toml::from_str(&raw)?;
    let db = Db::connect(cfg.database).await?;
    db.ping().await?;
    println!("connected: backend={:?}", db.backend());
    Ok(())
}
```

> **Note:** this example uses `toml::from_str` directly to keep the snippet small and not require an altair-config setup file. The crate's `with_config.rs` heading reads as "config-from-file via serde"; an altair-config-driven variant lives in altair-config's own docs.

- [ ] **Step 2: Add `toml` dev-dep** (if not already present from Task 2.2)

Confirm `toml = { workspace = true }` is in `crates/altair-db/Cargo.toml` `[dev-dependencies]`. If not, add it.

- [ ] **Step 3: Write `examples/transaction.rs`**

```rust
//! Closure-style transaction across two writes.
//!
//! Run with: `cargo run --example transaction -p altair-db`

use altair_db::prelude::*;
use sea_orm::{ConnectionTrait, Statement};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = Db::connect(Config::from_url("sqlite::memory:")).await?;
    let backend = db.orm().get_database_backend();
    db.orm()
        .execute(Statement::from_string(
            backend,
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)".to_string(),
        ))
        .await?;
    db.orm()
        .execute(Statement::from_string(
            backend,
            "CREATE TABLE orders (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, total INTEGER NOT NULL)".to_string(),
        ))
        .await?;

    db.transaction::<_, (), sea_orm::DbErr>(|tx| {
        Box::pin(async move {
            tx.execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "INSERT INTO users (name) VALUES ('alice')".to_string(),
            ))
            .await?;
            tx.execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "INSERT INTO orders (user_id, total) VALUES (1, 99)".to_string(),
            ))
            .await?;
            Ok(())
        })
    })
    .await?;

    println!("two-write transaction committed");
    Ok(())
}
```

- [ ] **Step 4: Build + smoke-run**

```bash
cargo build -p altair-db --examples
cargo run --example with_config -p altair-db
cargo run --example transaction -p altair-db
```

Expected: clean build, both examples print and exit 0.

- [ ] **Step 5: Commit**

```bash
git add crates/altair-db/examples/with_config.rs crates/altair-db/examples/transaction.rs
git commit -m "docs(db): add with_config and transaction examples"
```

### Task 6.4: `with_otel.rs`

**Files:**
- Create: `crates/altair-db/examples/with_otel.rs`

- [ ] **Step 1: Write the example**

```rust
//! Cross-crate auto-integration with `altair-otel`.
//!
//! `altair-db` enables SeaORM's `sqlx_logging`, which emits `tracing` events
//! for every executed statement. Initialise `altair-otel` in the same process
//! and those events flow through the configured exporter.
//!
//! Run with: `cargo run --example with_otel -p altair-db`

use altair_db::prelude::*;
use altair_otel::{Config as OtelConfig, Exporter};
use sea_orm::{ConnectionTrait, Statement};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    OtelConfig::builder()
        .service_name("db-demo")
        .service_version("0.1.0")
        .exporter(Exporter::Stdout)
        .build()
        .init()?;

    let db = Db::connect(Config::from_url("sqlite::memory:")).await?;
    let backend = db.orm().get_database_backend();

    // Operation #1: an ORM-style execute.
    db.orm()
        .execute(Statement::from_string(
            backend,
            "CREATE TABLE pings (n INTEGER)".to_string(),
        ))
        .await?;

    // Operation #2: a raw sqlx query via the pool.
    let pool = db.sqlite_pool().expect("sqlite pool");
    sqlx::query("INSERT INTO pings (n) VALUES (1)")
        .execute(pool)
        .await?;

    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pings")
        .fetch_one(pool)
        .await?;
    println!("rows: {n}");

    altair_otel::shutdown();
    Ok(())
}
```

- [ ] **Step 2: Build + smoke-run**

```bash
cargo build -p altair-db --example with_otel
cargo run --example with_otel -p altair-db 2>&1 | head -40
```

Expected: stdout shows the program output plus at least two OTel spans (one for the CREATE TABLE, one for the INSERT) with `db.statement` attributes.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-db/examples/with_otel.rs
git commit -m "docs(db): add with_otel example showing cross-crate auto-integration"
```

---

## Phase 7: README + workspace docs

### Task 7.1: Full crate README

**Files:**
- Modify: `crates/altair-db/README.md`

- [ ] **Step 1: Replace the stub with the full README**

```markdown
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
```

- [ ] **Step 2: Commit**

```bash
git add crates/altair-db/README.md
git commit -m "docs(db): full crate README with quick-start, features, config, examples"
```

### Task 7.2: Workspace README + porting tracker + INSTRUCTION

**Files:**
- Modify: `README.md` (workspace root)
- Modify: `docs/porting-tracker.md`
- Modify: `INSTRUCTION.md` (if it lists crates)

- [ ] **Step 1: Update the workspace README crate table**

In root `README.md`, find the crate table and add a row for `altair-db` between `altair-server` and any subsequent rows, matching the existing column structure (name, version badge, one-line description). If there's no version yet (release happens after merge), use the same placeholder pattern as other in-flight crates.

- [ ] **Step 2: Update `docs/porting-tracker.md`**

- In the Starter Set table, change the `altair-db` row's Status from `💤 Deferred` to `✅ Done` (or add a row if missing).
- Move the "Next milestone" sentence: drop `altair-db` from the candidates list.
- Add a release-notes bullet under the existing list:

```markdown
- **`altair-db` 0.1.x** (date TBD on publish) — Sea-ORM + sqlx convenience layer. Postgres + MySQL + SQLite, smart pool defaults, sqlx-migrate, OTel-aware query tracing, closure-style transactions.
```

- Update the "Last updated" line at the top to today's date (2026-05-29).

- [ ] **Step 3: Update `INSTRUCTION.md` if it lists crates**

Search `INSTRUCTION.md` for an existing crates list (e.g. a workspace overview section). If one exists and lists each crate with a one-line description, append:

```markdown
- `altair-db` — sea-orm + sqlx wrapper with pool/migrate/tracing/transaction helpers, three backends.
```

If no such list exists, skip this step.

- [ ] **Step 4: Commit**

```bash
git add README.md docs/porting-tracker.md INSTRUCTION.md
git commit -m "docs: register altair-db in workspace README, porting tracker, and INSTRUCTION"
```

---

## Phase 8: CI gate + PR + merge

### Task 8.1: Full workspace gate

**Files:** none (verification only)

- [ ] **Step 1: Format**

Run: `cargo fmt --all`
Then: `cargo fmt --all --check`
Expected: exit 0.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: exit 0, no warnings.

- [ ] **Step 3: Build all targets**

Run: `cargo build --workspace --all-targets --all-features`
Expected: clean build.

- [ ] **Step 4: Tests (unit + sqlite integration + doc-tests)**

Run: `cargo test --workspace`
Expected: every crate's existing tests pass; altair-db reports unit + sqlite integration suites green.

The Postgres + MySQL integration suites only run on Linux runners with Docker — verify locally only if Docker is present; otherwise rely on CI.

- [ ] **Step 5: cargo-deny**

Run: `cargo deny check` (if `cargo-deny` is installed locally; otherwise rely on CI).
Expected: bans/licenses/sources/advisories all `ok`.

- [ ] **Step 6: Publish dry-run**

Run: `cargo publish --dry-run -p altair-db`
Expected: packaging + verification succeed; ends with the "aborting upload due to dry run" line.

- [ ] **Step 7: If anything fails, fix and re-run**

No commit yet; fix in place. Common failure modes:
- Missing `version = "0.1"` on a path dev-dep → cargo-deny `wildcard` (apply the pattern from sibling crates).
- `result_large_err` clippy lint on `Result<Db>` if `Error` is over 128 bytes → box the variant or add `#[allow(clippy::result_large_err)]` on the function.

### Task 8.2: Push + PR + foreground-poll + merge

**Files:** none

- [ ] **Step 1: Push branch**

```bash
git push -u origin feat/altair-db
```

- [ ] **Step 2: Open PR**

```bash
gh pr create --title "feat(db): add altair-db crate (sea-orm + sqlx)" --body "$(cat <<'EOF'
## Summary

New crate `altair-db`: a thin convenience layer over sea-orm 1.x and sqlx 0.8.

- One `Db` handle exposing both `orm()` and per-backend `pg_pool()` / `mysql_pool()` / `sqlite_pool()` from a single shared connection pool.
- Three backends (Postgres + MySQL + SQLite), each behind a cargo feature, all default on.
- Smart pool defaults; `Config` is `serde::Deserialize` for altair-config integration with humantime durations.
- `db.migrate("./migrations")` over sqlx's filesystem migrator.
- `db.ping()` for health checks; `db.transaction(|tx| async { ... })` closure helper.
- Query tracing via SeaORM's `sqlx_logging` — flows to `altair-otel` automatically.

Design spec: `docs/specs/2026-05-29-altair-db-design.md`
Implementation plan: `docs/plans/2026-05-29-altair-db-implementation.md`

## Test plan

- [x] Unit tests pass (`Config`, `Backend`, `build_options`, `Error`)
- [x] SQLite integration tests pass (connect/ping/migrate/raw/transaction)
- [ ] CI green on PR (Postgres + MySQL integration via testcontainers run on Linux)
- [x] `cargo publish --dry-run -p altair-db` succeeds
EOF
)"
```

- [ ] **Step 3: Foreground-poll CI**

```bash
until [ "$(gh pr checks --json state,bucket | python3 -c 'import json,sys; d=json.loads(sys.stdin.read()); print("done" if all(c.get("bucket") in ("pass","fail","skipping","cancel") for c in d) else "wait")')" = "done" ]; do sleep 20; done
gh pr checks
```

Expected: all five checks (clippy, rustfmt, doc, test, cargo-deny) pass. If anything fails, push fixes to the same branch and re-poll.

- [ ] **Step 4: Squash-merge + delete branch**

```bash
gh pr merge --squash --delete-branch
```

- [ ] **Step 5: Sync local main**

```bash
git checkout main
git pull
```

- [ ] **Step 6: Handle release-plz PR**

When release-plz opens a `chore: release` PR for the new crate, verify the diff bumps `altair-db` to its first published version (not just a workspace churn bump). If it's a real release PR, approve + merge. If it's empty churn, close it with the established "no consumable changes" comment.

---

## Self-review notes (for the executor)

- Every `# Database` workspace dep is one block; don't sprinkle across sections.
- Per-backend `*_pool()` methods are `#[cfg(feature = ...)]`. Always feature-gate both the method definition AND any call sites in tests/examples to keep `--no-default-features` clean.
- The `transaction` signature uses `sea_orm::TransactionError<E>`, not `crate::Error`. This is intentional — see spec §3.3 notes.
- `pg_pool()` returning `Option<&PgPool>` is deliberate even though, at runtime, only one backend is ever live. Callers do a one-time `.expect("postgres")` at startup, then hold the reference. Don't "improve" this to a panicking accessor.
- Backend integration tests (`postgres.rs`, `mysql.rs`) are gated `target_os = "linux"`. Don't widen — Docker availability on macOS/Windows runners is unreliable and would block CI on flake.
