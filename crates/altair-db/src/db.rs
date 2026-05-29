//! [`Db`]: the single handle that owns a `SeaORM` connection and exposes both
//! the ORM layer and the raw sqlx pool.

use sea_orm::{ConnectionTrait, Database, DatabaseConnection};

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
    /// Validates the URL is non-empty, builds [`sea_orm::ConnectOptions`], then calls
    /// [`sea_orm::Database::connect`]. The returned `Db` owns the pool.
    pub async fn connect(config: Config) -> Result<Self> {
        if config.url.trim().is_empty() {
            return Err(Error::Configuration("url is required".to_string()));
        }
        let opts = build_options(&config);
        let conn = Database::connect(opts).await.map_err(Error::Connect)?;
        Ok(Self { conn })
    }

    /// The `SeaORM` connection — use for ORM-style CRUD.
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

    /// Raw sqlx Postgres pool, if this `Db` is connected to Postgres.
    ///
    /// Returns `None` when connected to a different backend.
    #[cfg(feature = "postgres")]
    #[must_use]
    pub fn pg_pool(&self) -> Option<&sqlx::PgPool> {
        match &self.conn {
            DatabaseConnection::SqlxPostgresPoolConnection(_) => {
                Some(self.conn.get_postgres_connection_pool())
            }
            _ => None,
        }
    }

    /// Raw sqlx `MySQL` pool, if this `Db` is connected to `MySQL`.
    ///
    /// Returns `None` when connected to a different backend.
    #[cfg(feature = "mysql")]
    #[must_use]
    pub fn mysql_pool(&self) -> Option<&sqlx::MySqlPool> {
        match &self.conn {
            DatabaseConnection::SqlxMySqlPoolConnection(_) => {
                Some(self.conn.get_mysql_connection_pool())
            }
            _ => None,
        }
    }

    /// Raw sqlx `SQLite` pool, if this `Db` is connected to `SQLite`.
    ///
    /// Returns `None` when connected to a different backend.
    #[cfg(feature = "sqlite")]
    #[must_use]
    pub fn sqlite_pool(&self) -> Option<&sqlx::SqlitePool> {
        match &self.conn {
            DatabaseConnection::SqlxSqlitePoolConnection(_) => {
                Some(self.conn.get_sqlite_connection_pool())
            }
            _ => None,
        }
    }

    /// Run `SELECT 1` to verify the pool is alive.
    pub async fn ping(&self) -> Result<()> {
        use sea_orm::Statement;
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

    /// Run a closure inside a `SeaORM` transaction.
    ///
    /// The closure receives `&DatabaseTransaction`, which implements
    /// [`sea_orm::ConnectionTrait`], so all `SeaORM` operations work. Returning `Ok`
    /// commits; returning `Err` rolls back.
    ///
    /// The error type is [`sea_orm::TransactionError<E>`], re-exported by
    /// `SeaORM` and preserving the closure's `E` faithfully.
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
        use sea_orm::TransactionTrait;
        self.conn.transaction(f).await
    }
}

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

    #[tokio::test]
    async fn sqlite_pool_present_postgres_absent() {
        let db = Db::connect(Config::from_url("sqlite::memory:")).await.unwrap();
        assert!(db.sqlite_pool().is_some());
        #[cfg(feature = "postgres")]
        assert!(db.pg_pool().is_none());
        #[cfg(feature = "mysql")]
        assert!(db.mysql_pool().is_none());
    }

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

    #[tokio::test]
    async fn migrate_applies_files() {
        use std::io::Write;
        use sea_orm::{ConnectionTrait, Statement};

        let dir = tempfile::tempdir().unwrap();
        let mut f = std::fs::File::create(dir.path().join("20260101000000_init.sql")).unwrap();
        writeln!(f, "CREATE TABLE widgets (id INTEGER PRIMARY KEY, name TEXT NOT NULL);").unwrap();
        drop(f);

        let db = Db::connect(Config::from_url("sqlite::memory:")).await.unwrap();
        db.migrate(dir.path()).await.unwrap();

        let stmt = Statement::from_string(
            db.orm().get_database_backend(),
            "INSERT INTO widgets (name) VALUES ('test')".to_string(),
        );
        db.orm().execute(stmt).await.unwrap();
    }

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
}
