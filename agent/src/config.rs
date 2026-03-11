use serde::Deserialize;
use std::{fs, path::Path};

fn default_interval_secs() -> u64 {
    30
}

fn default_buffer_duration_secs() -> u64 {
    300
}

fn default_log_level() -> String {
    "info".to_string()
}

/// Agent configuration loaded from an external TOML file.
///
/// `collector_url` is required and has no default; all other fields fall back
/// to sensible defaults when absent from the file.
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Unique identifier for this agent.  Defaults to the machine hostname
    /// when left empty or omitted.
    #[serde(default)]
    pub agent_id: String,
    /// Full URL of the collector's metrics ingest endpoint.
    pub collector_url: String,
    /// How often (in seconds) the agent collects and ships metrics.
    #[serde(default = "default_interval_secs")]
    pub interval_secs: u64,
    /// How long (in seconds) the agent buffers payloads when the collector is
    /// unreachable before dropping the oldest entries.
    #[serde(default = "default_buffer_duration_secs")]
    pub buffer_duration_secs: u64,
    /// Minimum log level: `error`, `warn`, `info`, `debug`, or `trace`.
    /// Overridable at runtime via the `RUST_LOG` environment variable.
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl Config {
    /// Returns the effective agent identifier.
    ///
    /// Uses the configured `agent_id` when it is non-empty; otherwise falls
    /// back to the machine's hostname.
    pub fn effective_agent_id(&self) -> String {
        let id = self.agent_id.trim();
        if id.is_empty() {
            fallback_hostname()
        } else {
            id.to_string()
        }
    }
}

/// Load a [`Config`] from the TOML file at `path`.
///
/// Returns an error string if the file cannot be read or if the TOML is
/// malformed / missing required fields (`collector_url` has no default and
/// must be present in the config file).
pub fn load(path: &Path) -> Result<Config, String> {
    let contents = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read config file '{}': {}", path.display(), e))?;
    toml::from_str(&contents)
        .map_err(|e| format!("Malformed config file '{}': {}", path.display(), e))
}

/// Attempt to resolve the local machine hostname via the `hostname` command.
///
/// Falls back to `"unknown-host"` if the command is unavailable or returns
/// non-UTF-8 output.
pub(crate) fn fallback_hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown-host".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_toml(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn load_minimal_config() {
        let f = write_toml(
            r#"collector_url = "http://localhost:8080/api/v1/metrics""#,
        );
        let cfg = load(f.path()).unwrap();
        assert_eq!(cfg.collector_url, "http://localhost:8080/api/v1/metrics");
        assert_eq!(cfg.interval_secs, 30);
        assert_eq!(cfg.buffer_duration_secs, 300);
        assert_eq!(cfg.log_level, "info");
    }

    #[test]
    fn load_full_config() {
        let f = write_toml(
            r#"
agent_id             = "server-01"
collector_url        = "http://collector:9000/api/v1/metrics"
interval_secs        = 60
buffer_duration_secs = 600
log_level            = "debug"
"#,
        );
        let cfg = load(f.path()).unwrap();
        assert_eq!(cfg.agent_id, "server-01");
        assert_eq!(cfg.interval_secs, 60);
        assert_eq!(cfg.buffer_duration_secs, 600);
        assert_eq!(cfg.log_level, "debug");
    }

    #[test]
    fn effective_agent_id_uses_configured_value() {
        let cfg = Config {
            agent_id: "my-server".to_string(),
            collector_url: "http://c/m".to_string(),
            interval_secs: 30,
            buffer_duration_secs: 300,
            log_level: "info".to_string(),
        };
        assert_eq!(cfg.effective_agent_id(), "my-server");
    }

    #[test]
    fn effective_agent_id_falls_back_to_hostname_when_empty() {
        let cfg = Config {
            agent_id: "".to_string(),
            collector_url: "http://c/m".to_string(),
            interval_secs: 30,
            buffer_duration_secs: 300,
            log_level: "info".to_string(),
        };
        let id = cfg.effective_agent_id();
        // Hostname should be non-empty (or at least the known fallback).
        assert!(!id.is_empty());
    }

    #[test]
    fn load_missing_file_returns_error() {
        let result = load(Path::new("/nonexistent/path/agent.toml"));
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("Cannot read config file"));
    }

    #[test]
    fn load_malformed_toml_returns_error() {
        let f = write_toml("this is not valid toml ===");
        let result = load(f.path());
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("Malformed config file"));
    }

    #[test]
    fn fallback_hostname_returns_non_empty_string() {
        // In any normal environment the hostname command is available and
        // returns a non-empty string.  This test exercises the function
        // directly rather than only via effective_agent_id.
        let h = fallback_hostname();
        assert!(!h.is_empty());
        assert_ne!(h, "");
    }

    #[test]
    fn load_missing_required_field_returns_error() {
        let f = write_toml(r#"agent_id = "srv""#);
        let result = load(f.path());
        assert!(result.is_err());
    }
}
