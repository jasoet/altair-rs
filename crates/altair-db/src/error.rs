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
        let raw = sqlx::migrate::MigrateError::Source(
            Box::<dyn std::error::Error + Send + Sync>::from("boom".to_string()),
        );
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
