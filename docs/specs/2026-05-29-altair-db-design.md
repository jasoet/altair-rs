## altair-db — Design

**Date:** 2026-05-29
**Status:** Draft — awaiting review before implementation planning
**Author:** Jasoet
**Spec type:** Brainstorming output → input to writing-plans

---

## 1. Overview

`altair-db` is a thin convenience layer over [`sea-orm`](https://crates.io/crates/sea-orm) and [`sqlx`](https://crates.io/crates/sqlx). It builds and owns a single SeaORM `DatabaseConnection` (which internally owns the sqlx connection pool), then exposes both layers from one `Db` handle: `db.orm()` for ORM CRUD, `db.pg_pool()` / `db.mysql_pool()` / `db.sqlite_pool()` for raw sqlx (views, materialized views, CTEs, COPY, LISTEN/NOTIFY).

**One-line product goal:** Stop copying connection-pool + migration + tracing boilerplate into every service, and stop choosing between an ORM for CRUD ergonomics and raw SQL for power.

Built-in extras (all on by default, all configurable): smart pool defaults, OTel-aware query tracing via SeaORM's `sqlx_logging`, a `SELECT 1` health probe, a closure-based transaction helper, and a `db.migrate(path)` shortcut over `sqlx::migrate::Migrator`. The underlying `sea_orm` and `sqlx` crates are re-exported at the crate root, so consumers depend on `altair-db` alone for both layers.

## 2. Decisions Locked

| Decision | Choice |
|---|---|
| Crate name | `altair-db` (verified available on crates.io 2026-05-29) |
| Backends | Postgres + MySQL + SQLite, each behind a cargo feature, all can be enabled simultaneously |
| Backend selection | Runtime, from DSN scheme (`postgres://` / `postgresql://`, `mysql://`, `sqlite://`) |
| ORM layer | `sea-orm` 1.x (re-exported as `pub use ::sea_orm;`) |
| Raw SQL layer | `sqlx` (re-exported as `pub use ::sqlx;`); pool obtained from SeaORM, never built separately |
| TLS | `rustls` (via sqlx's `runtime-tokio-rustls` feature) |
| Migrations | `sqlx::migrate::Migrator` filesystem migrator over the shared pool |
| Tracing | sqlx's `tracing` events enabled via SeaORM `ConnectOptions::sqlx_logging`; spans flow to `altair-otel` automatically |
| Config | Typed `Config` struct with `serde::Deserialize` for altair-config; `Config::from_url(&str)` for raw DSN use |
| Pool defaults | `min_connections=1`, `max_connections=10`, `acquire_timeout=30s`, `idle_timeout=10m`, `max_lifetime=30m` |
| Health probe | `db.ping().await -> Result<()>` runs `SELECT 1` via backend-dispatched `Statement` |
| Transactions | `db.transaction(\|tx\| async { ... })` closure helper, passthrough to SeaORM's `transaction()` |
| Error type | `thiserror` enum: `Connect`, `Migration`, `Configuration`, `Sql`, `Orm` |
| Async runtime | tokio |
| Edition / MSRV | Inherit workspace (Edition 2024, Rust 1.95) |

## 3. Architecture

### 3.1 File layout

```
crates/altair-db/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs        # crate root: lints, mod decls, re-exports (sea_orm, sqlx), prelude
│   ├── error.rs      # Error enum + Result alias (thiserror)
│   ├── config.rs     # Config struct + Backend enum + Config::from_url + altair-config integration
│   ├── db.rs         # Db struct: orm()/pg_pool()/mysql_pool()/sqlite_pool()/ping()/transaction()/migrate()
│   ├── connect.rs    # pub(crate) fn that turns Config into a sea_orm::ConnectOptions
│   └── prelude.rs    # one-import bundle
├── tests/
│   ├── sqlite.rs     # always-on integration (sqlite::memory: + tempfile)
│   ├── postgres.rs   # gated by `postgres` feature + testcontainers
│   └── mysql.rs      # gated by `mysql` feature + testcontainers
└── examples/
    ├── basic_orm.rs            # SeaORM Active Record CRUD
    ├── raw_sqlx.rs             # raw query via db.pg_pool()
    ├── materialized_view.rs    # create + refresh + query a Postgres mat view via sqlx
    ├── with_config.rs          # load Config from altair-config TOML
    ├── with_otel.rs            # cross-crate auto-integration
    ├── migrations.rs           # db.migrate("./migrations") flow
    └── transaction.rs          # closure-based tx that touches multiple ORM operations
```

### 3.2 Module responsibilities

- **`error.rs`** — sole owner of the `Error` enum and `Result<T>` alias. Splits `Connect` (startup) from `Orm` (runtime query failure) so callers can react differently.

- **`config.rs`** — `Config` struct (defaults match the locked table), `Backend` enum, `Config::from_url(impl Into<String>)`, `Config::backend()` (parses scheme). `serde::Deserialize` with `#[serde(default)]` at the struct level; `humantime_serde` for the `Duration` fields so TOML can use `"30s"`.

- **`db.rs`** — `Db` struct holding a `sea_orm::DatabaseConnection`. Constructor `Db::connect(Config)` validates the URL, builds `ConnectOptions` (via `connect.rs`), calls `sea_orm::Database::connect`. Public methods: `orm()`, `pg_pool()` / `mysql_pool()` / `sqlite_pool()` (each feature-gated), `backend()`, `ping()`, `migrate(path)`, `transaction(closure)`, `close()`.

- **`connect.rs`** — `pub(crate) fn build_options(&Config) -> sea_orm::ConnectOptions`. Pure mapping function, unit-testable without a live DB. Centralises the "every knob from Config maps to the right ConnectOptions field" logic.

- **`prelude.rs`** — `pub use crate::{Config, Db, Error, Result, Backend};` plus the SeaORM traits needed to write CRUD without further imports (`EntityTrait`, `ActiveModelTrait`, `ColumnTrait`, `QueryFilter`).

### 3.3 Public API surface

```rust
// crate root re-exports
pub use config::{Backend, Config};
pub use db::Db;
pub use error::{Error, Result};

pub use ::sea_orm;
pub use ::sqlx;

pub mod prelude;
```

```rust
// config.rs
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    #[serde(with = "humantime_serde")]
    pub acquire_timeout: Duration,
    #[serde(with = "humantime_serde", default)]
    pub idle_timeout: Option<Duration>,
    #[serde(with = "humantime_serde", default)]
    pub max_lifetime: Option<Duration>,
    pub sqlx_logging: bool,
    #[serde(with = "humantime_serde")]
    pub sqlx_slow_query_threshold: Duration,
}

impl Config {
    pub fn from_url(url: impl Into<String>) -> Self;
    pub fn backend(&self) -> Result<Backend>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend { Postgres, MySql, Sqlite }
```

```rust
// db.rs
pub struct Db { conn: sea_orm::DatabaseConnection }

impl Db {
    pub async fn connect(config: Config) -> Result<Self>;

    pub fn orm(&self) -> &sea_orm::DatabaseConnection;

    #[cfg(feature = "postgres")]
    pub fn pg_pool(&self) -> Option<&sqlx::PgPool>;

    #[cfg(feature = "mysql")]
    pub fn mysql_pool(&self) -> Option<&sqlx::MySqlPool>;

    #[cfg(feature = "sqlite")]
    pub fn sqlite_pool(&self) -> Option<&sqlx::SqlitePool>;

    pub fn backend(&self) -> Backend;

    pub async fn ping(&self) -> Result<()>;          // SELECT 1
    pub async fn migrate(&self, path: impl AsRef<Path>) -> Result<()>;
    pub async fn close(self) -> Result<()>;

    pub async fn transaction<F, T, E>(
        &self,
        f: F,
    ) -> std::result::Result<T, sea_orm::TransactionError<E>>
    where
        F: for<'c> FnOnce(&'c sea_orm::DatabaseTransaction)
              -> Pin<Box<dyn Future<Output = std::result::Result<T, E>> + Send + 'c>> + Send,
        T: Send,
        E: std::error::Error + Send + Sync + 'static;
}
```

```rust
// error.rs
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to connect to database")]
    Connect(#[source] sea_orm::DbErr),

    #[error("migration failed")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("invalid configuration: {0}")]
    Configuration(String),

    #[error("sql error")]
    Sql(#[from] sqlx::Error),

    #[error("orm error")]
    Orm(#[source] sea_orm::DbErr),
}
pub type Result<T> = std::result::Result<T, Error>;
```

**Notes on the choices:**

- `pg_pool()` / `mysql_pool()` / `sqlite_pool()` return `Option` because at compile time multiple backend features may be on, but at runtime only one backend is connected. The `Option` collapses to a one-time `.expect("backend is postgres")` at startup; the reference is then held.
- `transaction()` uses SeaORM's `DatabaseTransaction`, which implements `ConnectionTrait`, so the closure can call any ORM method. The return type is `sea_orm::TransactionError<E>` (re-exported from SeaORM), preserving the closure's `E` faithfully — `TransactionError::Connection(DbErr)` for pool/begin/commit failures, `TransactionError::Transaction(E)` for closure failures. For a raw sqlx tx, callers go straight to the pool (`db.pg_pool().expect(...).begin().await?`) and manage it themselves. Mixing both layers in a single tx is intentionally not in v0.1 — SeaORM doesn't expose its inner sqlx tx.
- `migrate(path)` calls `sqlx::migrate::Migrator::new(path).await?.run(pool).await?` against the backend-appropriate pool.

## 4. Behaviour Details

### 4.1 Backend dispatch

`Config::backend()` parses the URL scheme and returns `Backend::Postgres` (`postgres://`, `postgresql://`), `Backend::MySql` (`mysql://`), or `Backend::Sqlite` (`sqlite://`, including `sqlite::memory:`). Unknown schemes return `Error::Configuration("unsupported url scheme")`.

At runtime, `Db::backend()` reflects which backend the underlying SeaORM connection actually opened (read from `DatabaseConnection::get_database_backend()` and mapped to our `Backend` enum). Per-backend `*_pool()` accessors return `Some` only if `backend()` matches.

### 4.2 Pool tuning via `ConnectOptions`

`build_options(&Config)` constructs `sea_orm::ConnectOptions::new(config.url.clone())` and sets:

```text
.max_connections(config.max_connections)
.min_connections(config.min_connections)
.acquire_timeout(config.acquire_timeout)
.idle_timeout(config.idle_timeout)               // Option<Duration>
.max_lifetime(config.max_lifetime)               // Option<Duration>
.sqlx_logging(config.sqlx_logging)
.sqlx_logging_level(LevelFilter::Debug)
.sqlx_slow_statements_logging_threshold(config.sqlx_slow_query_threshold)
```

Defaults applied via `Default for Config`:

```rust
Config {
    url: String::new(),
    max_connections: 10,
    min_connections: 1,
    acquire_timeout: Duration::from_secs(30),
    idle_timeout: Some(Duration::from_secs(600)),    // 10 minutes
    max_lifetime: Some(Duration::from_secs(1800)),   // 30 minutes
    sqlx_logging: true,
    sqlx_slow_query_threshold: Duration::from_secs(1),
}
```

`Db::connect` rejects `Config` with empty `url` via `Error::Configuration("url is required")`. No other config-time validation in v0.1 — sqlx surfaces malformed URLs at connect time and they become `Error::Connect`.

### 4.3 Tracing + OTel

SeaORM's `sqlx_logging` flag enables sqlx's internal `tracing` instrumentation: every executed statement emits a span named `query` with `db.statement`, `db.elapsed`, and `db.rows_affected` fields. Slow statements (above `sqlx_slow_query_threshold`) log at WARN level.

When the host process has initialised `altair-otel`, these spans are picked up by the global `tracing` subscriber and forwarded to the configured exporter — no extra wiring in `altair-db`. This mirrors the cross-crate pattern in altair-rest, altair-server, altair-retry, and altair-concurrent.

The `with_otel.rs` example performs one ORM CRUD operation and one raw sqlx query, then exits via `altair_otel::shutdown()`. The stdout exporter shows two `query` spans with backend, statement text, and timing.

### 4.4 Health probe

```rust
pub async fn ping(&self) -> Result<()> {
    use sea_orm::{ConnectionTrait, Statement};
    let stmt = Statement::from_string(self.conn.get_database_backend(), "SELECT 1");
    self.conn.execute(stmt).await.map_err(Error::Orm)?;
    Ok(())
}
```

`Statement::from_string` plus `get_database_backend()` lets the same code run on all three backends. Designed to plug into altair-server's `/health`:

```rust
let db = Arc::new(altair_db::Db::connect(cfg).await?);
let server = altair_server::Server::builder()
    .custom_health(move || {
        let db = db.clone();
        async move { db.ping().await.is_ok() }
    })
    .build()?;
```

(Exact `custom_health` signature will be verified against the current altair-server release during implementation planning.)

### 4.5 Migrations

```rust
pub async fn migrate(&self, path: impl AsRef<Path>) -> Result<()> {
    let migrator = sqlx::migrate::Migrator::new(path.as_ref()).await?;
    match self.backend() {
        #[cfg(feature = "postgres")]
        Backend::Postgres => migrator.run(self.pg_pool().expect("postgres pool")).await?,
        #[cfg(feature = "mysql")]
        Backend::MySql => migrator.run(self.mysql_pool().expect("mysql pool")).await?,
        #[cfg(feature = "sqlite")]
        Backend::Sqlite => migrator.run(self.sqlite_pool().expect("sqlite pool")).await?,
    }
    Ok(())
}
```

Migration files use sqlx's standard naming (`<timestamp>_<name>.sql`). The migrator creates and reads `_sqlx_migrations` to track applied versions; all three backends use the same table format.

Backend-specific SQL (e.g. `CREATE MATERIALIZED VIEW ...` for Postgres only) lives in the same directory. It errors if run against the wrong backend — that is the expected behaviour, because each binary picks one backend at deploy time. `altair-db` does not attempt cross-dialect abstraction.

### 4.6 Transactions

```rust
db.transaction(|tx| Box::pin(async move {
    use sea_orm::EntityTrait;
    let user = users::Entity::insert(new_user).exec(tx).await?;
    orders::Entity::insert(new_order).exec(tx).await?;
    Ok::<_, sea_orm::DbErr>(user)
})).await?;
```

Thin wrapper over `sea_orm::DatabaseConnection::transaction()`. The closure receives `&DatabaseTransaction`, which implements `ConnectionTrait`; all SeaORM operations work unchanged. Errors returned from the closure are wrapped in `Error::Orm` if they are `sea_orm::DbErr`, or surfaced via `anyhow`-style boxing if generic `E: std::error::Error + Send + Sync + 'static`.

For raw sqlx transactions, the recommended pattern is:

```rust
let mut tx = db.pg_pool().expect("postgres").begin().await?;
sqlx::query("REFRESH MATERIALIZED VIEW user_summary")
    .execute(&mut *tx).await?;
tx.commit().await?;
```

A unified ORM + sqlx transaction is out of scope for v0.1 (see §6).

### 4.7 altair-config integration

```toml
[database]
url = "postgres://app:secret@localhost/app_dev"
max_connections = 20
min_connections = 2
acquire_timeout = "30s"
idle_timeout = "10m"
max_lifetime = "30m"
sqlx_logging = true
sqlx_slow_query_threshold = "1s"
```

```rust
#[derive(serde::Deserialize)]
struct AppConfig {
    database: altair_db::Config,
}

let app: AppConfig = altair_config::load("config", "MYAPP")?;
let db = altair_db::Db::connect(app.database).await?;
```

Defaults at the struct level mean a TOML file with only `[database] url = "..."` works. Environment overrides flow through altair-config's existing mechanism (`MYAPP__DATABASE__MAX_CONNECTIONS=50`).

## 5. Testing

### 5.1 Unit tests (in `src/`)

- `config.rs`:
  - `Config::backend()` parses `postgres://`, `postgresql://`, `mysql://`, `sqlite://`, `sqlite::memory:`, errors on `mongodb://` and the empty string.
  - `Default` for `Config` produces the locked-table defaults.
  - `humantime_serde` round-trip: `"30s"` → `Duration::from_secs(30)` → `"30s"`.
- `connect.rs`:
  - `build_options(&Config)` sets every `ConnectOptions` field. Pure-function test using SeaORM's `ConnectOptions` getters; no live DB.
- `error.rs`:
  - `From<sea_orm::DbErr>` returns `Error::Connect` (used by `Db::connect`), and the dedicated `Error::Orm` wrapper is reachable from query paths.

### 5.2 Integration tests (`tests/`)

- `sqlite.rs` (always runs): connect to `sqlite::memory:`, run `migrate()` over a tempdir holding two `.sql` files, ORM CRUD round-trip via an inline entity, raw sqlx query via `db.sqlite_pool()`, `ping()`, transaction commit + rollback.
- `postgres.rs` (gated by `postgres` feature, Linux runners only): spin up Postgres via `testcontainers`, run the same surface as sqlite plus a materialised-view round-trip via raw sqlx.
- `mysql.rs` (gated by `mysql` feature, Linux runners only): spin up MySQL via `testcontainers`, run the same surface as sqlite. No materialised view (not standard MySQL).

### 5.3 Doc-tests

Every public function gets at least one doc-test per the workspace rule (`#![deny(missing_docs)]`). Doc-tests that need a live DB use `sqlite::memory:` so they run under `cargo test --doc` without infrastructure.

### 5.4 CI matrix

The existing `test` job runs all unit + sqlite tests on every commit. A new conditional `services` block adds Postgres and MySQL containers on Linux runners; macOS and Windows jobs skip those integration files via `cfg(target_os = "linux")` gating on the test modules.

## 6. Out of Scope for v0.1

- Read replicas / multi-pool routing.
- Custom pool implementations (`deadpool`, `bb8`) — SeaORM's internal sqlx pool is the only pool.
- Entity codegen (`sea-orm-cli generate entity`) — separate tooling concern.
- Schema introspection helpers.
- Connection retry-on-startup — apps that need it can wrap `Db::connect` with `altair-retry`.
- Mixing ORM + sqlx inside a single transaction — additive when needed.
- Cross-dialect SQL abstraction in migrations — files are raw SQL per backend.

## 7. Examples — what each one demonstrates

| File | Demonstrates |
|---|---|
| `basic_orm.rs` | `Db::connect` + a small Entity + insert/find/update/delete via SeaORM. |
| `raw_sqlx.rs` | `db.pg_pool()` + `sqlx::query_as!` with compile-time-checked SQL. |
| `materialized_view.rs` | `CREATE MATERIALIZED VIEW ... AS ...`, `REFRESH MATERIALIZED VIEW`, query via raw sqlx — Postgres-only example. |
| `with_config.rs` | Loading `Config` from `config.toml` via altair-config. |
| `with_otel.rs` | `altair_otel::Config::builder().exporter(Stdout).build().init()` + one ORM op + one raw sqlx op; observe two `query` spans. |
| `migrations.rs` | `db.migrate("./migrations")` over a small `migrations/` directory bundled in the example. |
| `transaction.rs` | Closure-style transaction that inserts into two tables, with a separate rollback case. |

## 8. Dependencies (workspace + crate)

Workspace `Cargo.toml` additions to `[workspace.dependencies]`:

```toml
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
```

Crate `Cargo.toml`:

```toml
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
thiserror = { workspace = true }
tracing = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
anyhow = { workspace = true }
pretty_assertions = { workspace = true }
tempfile = { workspace = true }
testcontainers = "0.23"
testcontainers-modules = { version = "0.11", features = ["postgres", "mysql"] }
altair-otel = { path = "../altair-otel", version = "0.1" }
altair-config = { path = "../altair-config", version = "0.1" }
```

Exact sea-orm / sqlx versions confirmed during planning (semver-range may shift one minor). Both already pull `rustls` via the runtime feature, so no explicit `rustls` dep is needed.

## 9. Implementation-time Verifications

These are settled at planning time, not design time — they don't change the architecture:

1. **`humantime-serde` in workspace?** Reuse if already declared; add to `[workspace.dependencies]` if not.
2. **`altair-server` `custom_health` exact signature.** §4.4 assumes an async-closure-returning-`bool` shape; the published crate's API is checked and the example wording adjusted to match.
3. **MySQL TLS feature in sqlx 0.8.** `runtime-tokio-rustls` covers Postgres and SQLite. If MySQL needs an extra feature in the current sqlx version, it is added to the workspace dep entry in §8 and documented in the README.
