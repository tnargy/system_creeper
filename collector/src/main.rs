mod config;

use std::{path::PathBuf, process};

#[tokio::main]
async fn main() {
    let cfg = match std::env::args().nth(1) {
        Some(path) => {
            let p = PathBuf::from(&path);
            match config::load(&p) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        None => {
            // No config file provided — use built-in defaults.
            // `listen_addr` and `database_path` have no defaults, so they must
            // be present in a real deployment.  For the skeleton we fall back
            // to sensible development values so the binary can be exercised
            // without a config file during early development.
            config::Config {
                listen_addr: "0.0.0.0:8080".to_string(),
                database_path: "./data/metrics.db".to_string(),
                offline_threshold_secs: 120,
                retention_days: 30,
                log_level: "info".to_string(),
            }
        }
    };

    // Initialize structured logging.  The level is taken from the config; the
    // RUST_LOG env-var can still override it for development convenience.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            tracing_subscriber::EnvFilter::new(&cfg.log_level)
        });
    tracing_subscriber::fmt().with_env_filter(filter).init();

    tracing::info!(listen_addr = %cfg.listen_addr, "collector starting");
}

