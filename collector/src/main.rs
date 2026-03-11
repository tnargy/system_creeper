mod api;
mod config;
mod db;
mod retention;

use std::{net::SocketAddr, path::PathBuf, process};
use tokio::sync::broadcast;

/// Shared application state threaded through all route handlers.
#[derive(Clone)]
pub struct AppState {
    pub pool: db::Db,
    pub offline_threshold_secs: u64,
    /// Sender half of the broadcast channel used to push metric update events
    /// to all connected WebSocket clients.  The receiver half is subscribed to
    /// inside each WebSocket handler task.
    pub tx: broadcast::Sender<String>,
}

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

    let pool = match db::init_pool(&cfg.database_path).await {
        Ok(p) => {
            tracing::info!(path = %cfg.database_path, "database ready");
            p
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to initialise database");
            process::exit(1);
        }
    };

    // Capacity of 256: if a slow client falls this far behind it is acceptable
    // to drop messages rather than build up unbounded memory.
    let (tx, _initial_rx) = broadcast::channel::<String>(256);

    // Spawn the retention background task.  It runs once immediately at
    // startup, then every 24 hours, deleting metrics older than retention_days.
    retention::spawn(pool.clone(), cfg.retention_days);

    let state = AppState { pool, offline_threshold_secs: cfg.offline_threshold_secs, tx };
    let app = api::router(state);

    let addr: SocketAddr = match cfg.listen_addr.parse() {
        Ok(a) => a,
        Err(e) => {
            tracing::error!(error = %e, addr = %cfg.listen_addr, "invalid listen address");
            process::exit(1);
        }
    };

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(error = %e, %addr, "failed to bind");
            process::exit(1);
        }
    };

    tracing::info!(%addr, "listening");

    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!(error = %e, "server error");
        process::exit(1);
    }
}

