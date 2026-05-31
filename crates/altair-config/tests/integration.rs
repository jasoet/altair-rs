//! End-to-end behaviour tests: write a real TOML file on disk, layer an
//! env-var override on top, deserialize through `Loader`, verify
//! validation runs.
//!
//! Env-var mutation goes through `figment::Jail` so it's both safe (no
//! `unsafe { set_var }`) and serialised against other tests.

#![allow(clippy::result_large_err)] // figment::Error is intentionally rich.

use altair_config::{Deserialize, Loader, Validate};
use figment::Jail;
use pretty_assertions::assert_eq;
use std::fs;
use tempfile::TempDir;

#[derive(Debug, Deserialize, Validate)]
struct Database {
    #[validate(length(min = 1))]
    host: String,
    #[validate(range(min = 1, max = 65535))]
    port: u16,
}

#[derive(Debug, Deserialize, Validate)]
struct AppConfig {
    #[validate(length(min = 1))]
    name: String,
    #[validate(nested)]
    database: Database,
}

fn write_toml(dir: &TempDir, contents: &str) -> std::path::PathBuf {
    let path = dir.path().join("app.toml");
    fs::write(&path, contents).expect("write toml fixture");
    path
}

#[test]
fn loads_toml_file_only() {
    let dir = TempDir::new().unwrap();
    let path = write_toml(
        &dir,
        r#"
name = "altair"

[database]
host = "db.prod"
port = 5432
"#,
    );

    let cfg: AppConfig = Loader::new()
        .toml_file(&path)
        .build()
        .expect("loader builds from toml");
    assert_eq!(cfg.name, "altair");
    assert_eq!(cfg.database.host, "db.prod");
    assert_eq!(cfg.database.port, 5432);
}

#[test]
fn env_var_overrides_toml_value() {
    Jail::expect_with(|jail| {
        jail.create_file(
            "app.toml",
            r#"
name = "altair"

[database]
host = "db.staging"
port = 5432
"#,
        )?;
        let toml_path = jail.directory().join("app.toml");
        jail.set_env("APPCFG_DATABASE_HOST", "db.prod");

        let cfg: AppConfig = Loader::new()
            .toml_file(toml_path)
            .env_prefix("APPCFG")
            .build()
            .expect("loader builds with env override");

        assert_eq!(cfg.database.host, "db.prod");
        assert_eq!(cfg.database.port, 5432); // unchanged
        Ok(())
    });
}

#[test]
fn optional_missing_file_is_skipped() {
    let dir = TempDir::new().unwrap();
    let required = write_toml(
        &dir,
        r#"
name = "altair"

[database]
host = "localhost"
port = 5432
"#,
    );
    let optional_missing = dir.path().join("overrides.toml");

    let cfg: AppConfig = Loader::new()
        .toml_file(&required)
        .toml_file_optional(&optional_missing)
        .build()
        .expect("missing optional file should not error");
    assert_eq!(cfg.name, "altair");
}

#[test]
fn validation_failure_surfaces_error() {
    let dir = TempDir::new().unwrap();
    let path = write_toml(
        &dir,
        r#"
name = ""

[database]
host = "db.prod"
port = 5432
"#,
    );

    let res: altair_config::Result<AppConfig> = Loader::new().toml_file(&path).build();
    assert!(res.is_err(), "empty name must fail validation");
}

#[test]
fn missing_required_file_surfaces_error() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("nope.toml");

    let res: altair_config::Result<AppConfig> = Loader::new().toml_file(&missing).build();
    assert!(res.is_err(), "missing required file must error");
}
