//! One-liner convenience loaders.

use crate::error::Result;
use crate::loader::Loader;
use serde::de::DeserializeOwned;
use std::io::Read;
use std::path::Path;
use validator::Validate;

/// Load and validate config from a TOML string with env-var overrides.
pub fn from_toml_str<T>(toml: &str, env_prefix: &str) -> Result<T>
where
    T: DeserializeOwned + Validate,
{
    use figment::Figment;
    use figment::providers::{Env, Format, Toml};

    let figment = Figment::new()
        .merge(Toml::string(toml))
        .merge(Env::prefixed(&format!("{env_prefix}_")).split('_'));
    let value: T = figment.extract().map_err(crate::error::Error::from)?;
    value.validate()?;
    Ok(value)
}

/// Load and validate config from a TOML file with env-var overrides.
pub fn from_file<T>(path: impl AsRef<Path>, env_prefix: &str) -> Result<T>
where
    T: DeserializeOwned + Validate,
{
    Loader::new()
        .toml_file(path.as_ref().to_path_buf())
        .env_prefix(env_prefix)
        .build()
}

/// Load and validate config from any `Read` source.
pub fn from_reader<T>(mut reader: impl Read, env_prefix: &str) -> Result<T>
where
    T: DeserializeOwned + Validate,
{
    let mut s = String::new();
    reader.read_to_string(&mut s)?;
    from_toml_str(&s, env_prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde::Deserialize;
    use validator::Validate;

    #[derive(Debug, Deserialize, Validate)]
    struct Cfg {
        #[validate(range(min = 1))]
        port: u16,
    }

    #[test]
    fn from_toml_str_loads() {
        let cfg: Cfg = from_toml_str("port = 7777", "FYS_NONE_XYZ").unwrap();
        assert_eq!(cfg.port, 7777);
    }

    #[test]
    fn from_reader_loads() {
        let bytes = b"port = 4242";
        let cfg: Cfg = from_reader(&bytes[..], "FYR_NONE_XYZ").unwrap();
        assert_eq!(cfg.port, 4242);
    }
}
