use serde::Deserialize;
use std::{fs, path::Path};

/// The subset of [`Config`] fields that can be changed at runtime without a restart.
///
/// Wrapped in an `Arc<ArcSwap<ReloadableConfig>>` inside `AppState` so that a
/// SIGHUP / Ctrl+Break signal handler can swap in a fresh value while all
/// in-flight handlers keep reading the old one safely.
#[derive(Debug, Clone)]
pub struct ReloadableConfig {
    pub offline_threshold_secs: u64,
    pub retention_days: u32,
    pub log_level: String,
}

impl From<&Config> for ReloadableConfig {
    fn from(c: &Config) -> Self {
        ReloadableConfig {
            offline_threshold_secs: c.offline_threshold_secs,
            retention_days: c.retention_days,
            log_level: c.log_level.clone(),
        }
    }
}

fn default_offline_threshold_secs() -> u64 {
    120
}

fn default_retention_days() -> u32 {
    30
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_dashboard_dir() -> String {
    "./dashboard/dist".to_string()
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
    #[serde(default = "default_dashboard_dir")]
    pub dashboard_dir: String,
    /// Shared HMAC-SHA256 secret used to verify incoming metric payloads.
    ///
    /// When empty (the default) authentication is disabled entirely, preserving
    /// backward compatibility with agents that have not yet configured a secret.
    #[serde(default)]
    pub hmac_secret: String,
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
