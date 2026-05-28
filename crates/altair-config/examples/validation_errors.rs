//! Validation failures surface as `Error::Validation(ValidationErrors)` —
//! the inner errors give you field-level detail you can render in a UI or
//! log line.
//!
//! Run with: `cargo run --example validation_errors -p altair-config`

use altair_config::prelude::*;

#[derive(Debug, Deserialize, Validate)]
struct ServiceConfig {
    #[validate(length(min = 3, max = 32))]
    name: String,

    #[validate(range(min = 1, max = 1024))]
    workers: u32,

    #[validate(url)]
    callback_url: String,

    #[validate(email)]
    admin_email: String,
}

fn main() {
    let toml_bad = r#"
name = "x"
workers = 0
callback_url = "not-a-url"
admin_email = "admin"
"#;

    match from_toml_str::<ServiceConfig>(toml_bad, "SVC_NONE_XYZ") {
        Ok(_) => println!("(unexpected) parsed fine"),
        Err(Error::Validation(errs)) => {
            println!("validation failures:");
            for (field, field_errs) in errs.field_errors() {
                for fe in field_errs {
                    println!("  {field}: code={} message={:?}", fe.code, fe.message);
                }
            }
        }
        Err(other) => println!("non-validation error: {other}"),
    }
}
