//! Run with: `cargo run --example basic -p altair-config`

use altair_config::prelude::*;

#[derive(Debug, Deserialize, Validate)]
struct AppConfig {
    #[validate(range(min = 1, max = 65535))]
    port: u16,

    #[validate(length(min = 1))]
    name: String,
}

fn main() -> anyhow::Result<()> {
    let toml = r#"
port = 8080
name = "my-service"
"#;
    let cfg: AppConfig = from_toml_str(toml, "APP_UNUSED")?;
    println!("{cfg:#?}");
    Ok(())
}
