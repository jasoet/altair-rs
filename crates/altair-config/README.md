# altair-config

Type-safe TOML config loading with env-var overrides and validation. Wraps [`figment`](https://crates.io/crates/figment) and [`validator`](https://crates.io/crates/validator) under a unified surface.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Add to your project

```bash
cargo add altair-config
```

You do **not** need to add `figment`, `validator`, `serde`, or `toml` separately — `altair-config` re-exports the types and derives you need.

## Quick start — load + validate from a TOML string

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

Validation runs automatically before `from_toml_str` returns — a `port = 0` or `host = ""` fails with `Error::Validation`.

## Load from a file path

```rust,no_run
use altair_config::prelude::*;

# #[derive(Debug, Deserialize, Validate)] struct AppConfig { port: u16 }
# fn main() -> altair_config::Result<()> {
let cfg: AppConfig = from_file("config/app.toml", "APP")?;
# Ok(()) }
```

## Layered loading — base + local overrides + env

```rust,no_run
use altair_config::prelude::*;
# #[derive(Debug, Deserialize, Validate)] struct AppConfig { port: u16 }

# fn main() -> altair_config::Result<()> {
let cfg: AppConfig = Loader::new()
    .toml_file("config/base.toml")               // required — fails if missing
    .toml_file_optional("config/local.toml")     // overrides base if present
    .toml_file_optional("config/secret.toml")    // overrides local if present
    .env_prefix("APP")                            // overrides everything if set
    .build()?;
# Ok(()) }
```

Layers merge in insertion order — **later sources override earlier ones**.

## Env-var overrides

With `env_prefix("APP")`:

| Env var | Sets |
|---|---|
| `APP_PORT=9090` | `cfg.port = 9090` |
| `APP_DATABASE_HOST=db.prod.example.com` | `cfg.database.host = "db.prod.example.com"` |
| `APP_DATABASE_PORT=5433` | `cfg.database.port = 5433` |

Nested keys are joined by `_`. Values are coerced into the target Rust type — bad values produce `Error::Parse`.

## Validation — typed errors with field details

```rust,no_run
use altair_config::prelude::*;

#[derive(Debug, Deserialize, Validate)]
struct ServiceConfig {
    #[validate(length(min = 3, max = 32))]
    name: String,

    #[validate(range(min = 1, max = 1024))]
    workers: u32,

    #[validate(url)]
    callback_url: String,
}

# fn main() {
let bad = r#"
name = "x"
workers = 0
callback_url = "not-a-url"
"#;
match from_toml_str::<ServiceConfig>(bad, "SVC_NONE") {
    Ok(_) => unreachable!(),
    Err(Error::Validation(errs)) => {
        // ValidationErrors → field-name -> Vec<ValidationError>
        for (field, _) in errs.field_errors() {
            eprintln!("invalid field: {field}");
        }
    }
    Err(e) => eprintln!("other error: {e}"),
}
# }
```

## Load from any `Read` source

Useful for embedded config, network responses, or test fixtures:

```rust,no_run
use altair_config::prelude::*;
use std::io::Cursor;

# #[derive(Debug, Deserialize, Validate)] struct Cfg { port: u16 }
# fn main() -> altair_config::Result<()> {
let raw: Vec<u8> = b"port = 8080".to_vec();
let cfg: Cfg = from_reader(Cursor::new(raw), "CFG_NONE")?;
# Ok(()) }
```

## Error reference

| Variant | When |
|---|---|
| `Error::Io` | Couldn't read the TOML file from disk |
| `Error::Parse` | TOML is malformed or a key has the wrong type |
| `Error::Validation` | One or more `#[validate(...)]` rules failed |

## License

[MIT](../../LICENSE)
