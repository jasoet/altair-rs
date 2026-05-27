# altair-config

Type-safe TOML config loading with env-var overrides and validation. Wraps [`figment`](https://crates.io/crates/figment) and [`validator`](https://crates.io/crates/validator) under a unified surface.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Add to your project

```bash
cargo add altair-config
```

You do **not** need to add `figment`, `validator`, `serde`, or `toml` separately — `altair-config` re-exports the types and derives you need.

## Quick start

```rust,no_run
use altair_config::prelude::*;

#[derive(Debug, Deserialize, Validate)]
struct AppConfig {
    #[validate(range(min = 1, max = 65535))]
    port: u16,
    database: DbConfig,
}

#[derive(Debug, Deserialize, Validate)]
struct DbConfig {
    #[validate(length(min = 1))]
    host: String,
    port: u16,
}

# fn main() -> altair_config::Result<()> {
let toml = r#"
port = 8080

[database]
host = "localhost"
port = 5432
"#;
let cfg: AppConfig = from_toml_str(toml, "APP")?;
# Ok(()) }
```

## Layered loading

```rust,no_run
use altair_config::prelude::*;
# #[derive(Debug, Deserialize, Validate)] struct AppConfig { port: u16 }
# fn main() -> altair_config::Result<()> {
let cfg: AppConfig = Loader::new()
    .toml_file("config/base.toml")
    .toml_file_optional("config/local.toml")
    .env_prefix("APP")
    .build()?;
# Ok(()) }
```

## Env overrides

With `env_prefix("APP")`, `APP_PORT=9090` sets `cfg.port`, and `APP_DATABASE_HOST=db.prod` sets `cfg.database.host`. Nested keys are joined by `_`.

## License

[MIT](../../LICENSE)
