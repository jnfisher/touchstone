//! `touchstone.toml`.

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

pub const DEFAULT_BIND: &str = "0.0.0.0:8370";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub bind: String,
    pub data_dir: PathBuf,
    pub source: SourceConfig,
}

/// Filled in when the loaders exist.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SourceConfig {}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: DEFAULT_BIND.to_owned(),
            data_dir: PathBuf::from("data"),
            source: SourceConfig::default(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing config {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_gives_defaults() {
        let c: Config = toml::from_str("").unwrap();
        assert_eq!(c, Config::default());
        assert_eq!(c.bind, "0.0.0.0:8370");
    }

    #[test]
    fn overrides_parse() {
        let c: Config = toml::from_str("bind = \"127.0.0.1:1\"\n[source]\n").unwrap();
        assert_eq!(c.bind, "127.0.0.1:1");
    }
}
