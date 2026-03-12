mod config;
mod metrics;
mod sender;

use std::{path::PathBuf, process, time::Duration};

use metrics::NetworkBaseline;
use sender::Sender;

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

    let mut sender = Sender::new(
        cfg.collector_url.clone(),
        cfg.buffer_duration_secs,
        cfg.interval_secs,
    );

    // Network baseline carried across ticks to compute byte deltas.
    let mut net_baseline: Option<NetworkBaseline> = None;

    let interval = Duration::from_secs(cfg.interval_secs);
    let mut ticker = tokio::time::interval(interval);
    // Don't try to "catch up" missed ticks (e.g. after a slow collection).
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        ticker.tick().await;

        // Collect metrics; treat any unexpected error as a logged warning, not
        // a crash.  In practice `collect_metrics` is infallible, but wrapping
        // future-proofs the loop.
        let payload =
            match tokio::time::timeout(interval, metrics::collect_metrics(&agent_id, &mut net_baseline)).await {
                Ok(p) => p,
                Err(_) => {
                    tracing::warn!("metric collection timed out — skipping tick");
                    continue;
                }
            };

        // Send (with buffered retry).  Errors are logged inside `send_with_retry`;
        // we never propagate them here so the loop keeps running.
        sender.send_with_retry(payload).await;
    }
}

