use serde::Deserialize;
use std::{fs, path::Path};

fn default_offline_threshold_secs() -> u64 {
    120
}

fn default_retention_days() -> u32 {
    30
}

fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub listen_addr: String,
    pub database_path: String,
    #[serde(default = "default_offline_threshold_secs")]
    pub offline_threshold_secs: u64,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

/// Load a [`Config`] from the TOML file at `path`.
///
/// Returns an error string if the file cannot be read or if the TOML is
/// malformed / missing required fields (`listen_addr` and `database_path`
/// have no defaults and must be present in the config file).
pub fn load(path: &Path) -> Result<Config, String> {
    let contents = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read config file '{}': {}", path.display(), e))?;
    toml::from_str(&contents)
        .map_err(|e| format!("Malformed config file '{}': {}", path.display(), e))
}
