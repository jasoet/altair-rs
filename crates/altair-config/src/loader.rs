//! Multi-source layered config loader.

use crate::error::Result;
use figment::Figment;
use figment::providers::{Env, Format, Toml};
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use validator::Validate;

/// Builder for layered config loads.
///
/// Layers are merged in insertion order — later sources override earlier ones.
#[derive(Debug, Default)]
pub struct Loader {
    files: Vec<(PathBuf, bool)>, // (path, optional?)
    env_prefix: Option<String>,
}

impl Loader {
    /// Create a new empty loader.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a required TOML file. Loading fails if the file is missing.
    #[must_use]
    pub fn toml_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.files.push((path.into(), false));
        self
    }

    /// Add an optional TOML file. Missing files are silently skipped.
    #[must_use]
    pub fn toml_file_optional(mut self, path: impl Into<PathBuf>) -> Self {
        self.files.push((path.into(), true));
        self
    }

    /// Apply environment variable overrides with the given prefix.
    ///
    /// `APP_DATABASE_HOST=db.prod` sets `database.host` to `db.prod`.
    #[must_use]
    pub fn env_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.env_prefix = Some(prefix.into());
        self
    }

    /// Build, deserialize, and validate.
    pub fn build<T>(self) -> Result<T>
    where
        T: DeserializeOwned + Validate,
    {
        let mut figment = Figment::new();
        for (path, optional) in &self.files {
            if *optional && !Path::new(path).exists() {
                continue;
            }
            figment = figment.merge(Toml::file(path));
        }
        if let Some(prefix) = self.env_prefix {
            figment = figment.merge(Env::prefixed(&format!("{prefix}_")).split('_'));
        }
        let value: T = figment.extract()?;
        value.validate()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use pretty_assertions::assert_eq;
    use serde::Deserialize;
    use tempfile::NamedTempFile;
    use validator::Validate;

    #[derive(Debug, Deserialize, Validate)]
    struct Cfg {
        #[validate(range(min = 1))]
        port: u16,
        host: String,
    }

    #[test]
    fn loader_reads_toml_file() {
        let mut f = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut f, b"port = 9000\nhost = \"localhost\"\n").unwrap();
        let cfg: Cfg = Loader::new().toml_file(f.path()).build().unwrap();
        assert_eq!(cfg.port, 9000);
        assert_eq!(cfg.host, "localhost");
    }

    #[test]
    fn loader_missing_required_file_errors() {
        let r: Result<Cfg> = Loader::new().toml_file("/nonexistent/x.toml").build();
        assert!(r.is_err());
    }

    #[test]
    fn loader_missing_optional_file_is_ok_with_base() {
        let mut base = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut base, b"port = 9000\nhost = \"localhost\"\n").unwrap();
        let cfg: Cfg = Loader::new()
            .toml_file(base.path())
            .toml_file_optional("/does/not/exist.toml")
            .build()
            .unwrap();
        assert_eq!(cfg.port, 9000);
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn loader_env_override() {
        figment::Jail::expect_with(|jail| {
            jail.create_file("base.toml", "port = 9000\nhost = \"localhost\"\n")?;
            jail.set_env("TEST_ALT_PORT", "1234");
            let cfg: Cfg = Loader::new()
                .toml_file("base.toml")
                .env_prefix("TEST_ALT")
                .build()
                .map_err(|e| figment::Error::from(e.to_string()))?;
            assert_eq!(cfg.port, 1234);
            Ok(())
        });
    }

    #[test]
    fn validation_error_propagates() {
        let mut f = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut f, b"port = 0\nhost = \"localhost\"\n").unwrap();
        let r: Result<Cfg> = Loader::new().toml_file(f.path()).build();
        assert!(matches!(r, Err(Error::Validation(_))));
    }
}
