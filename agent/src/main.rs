mod config;

use std::{path::PathBuf, process};

#[tokio::main]
async fn main() {
    let path = match std::env::args().nth(1) {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("usage: agent <config-file>");
            process::exit(1);
        }
    };

    let cfg = match config::load(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    // Initialise structured logging.  The level comes from config; RUST_LOG
    // overrides it for development convenience.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&cfg.log_level));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let agent_id = cfg.effective_agent_id();
    tracing::info!(
        agent_id = %agent_id,
        collector_url = %cfg.collector_url,
        interval_secs = cfg.interval_secs,
        buffer_duration_secs = cfg.buffer_duration_secs,
        "agent starting",
    );
}

