mod api;
mod config;
mod db;
mod retention;

use std::{net::SocketAddr, path::PathBuf, process, sync::Arc};

use arc_swap::ArcSwap;
use tokio::sync::broadcast;
use tracing_subscriber::{fmt, prelude::*, reload, EnvFilter};

use config::ReloadableConfig;

/// Shared application state threaded through all route handlers.
///
/// The reloadable fields (`offline_threshold_secs`, `retention_days`,
/// `log_level`) live inside an `ArcSwap` so that the signal handler can swap
/// in a fresh value atomically while in-flight handlers keep reading the
/// previous snapshot safely.
#[derive(Clone)]
pub struct AppState {
    pub pool: db::Db,
    /// Hot-reloadable config fields.  Always access via `.reloadable.load()`.
    pub reloadable: Arc<ArcSwap<ReloadableConfig>>,
    /// Sender half of the broadcast channel used to push metric update events
    /// to all connected WebSocket clients.  The receiver half is subscribed to
    /// inside each WebSocket handler task.
    pub tx: broadcast::Sender<String>,
    /// Shared HMAC-SHA256 secret used to verify incoming metric payloads.
    /// Empty string means authentication is disabled (dev / backward-compatible mode).
    /// NOT reloadable — changing the secret mid-run would invalidate in-flight requests.
    pub hmac_secret: String,
}

#[tokio::main]
async fn main() {
    // ── Config ────────────────────────────────────────────────────────────────
    let (cfg, config_path) = match std::env::args().nth(1) {
        Some(path) => {
            let p = PathBuf::from(&path);
            match config::load(&p) {
                Ok(c) => (c, Some(p)),
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
            let default_cfg = config::Config {
                listen_addr: "0.0.0.0:8080".to_string(),
                database_path: "./data/metrics.db".to_string(),
                offline_threshold_secs: 120,
                retention_days: 30,
                log_level: "info".to_string(),
                dashboard_dir: "./dashboard/dist".to_string(),
                hmac_secret: String::new(),
            };
            (default_cfg, None)
        }
    };

    // ── Logging (with hot-reload support) ─────────────────────────────────────
    //
    // We build the subscriber in two layers so we can hand a `reload::Handle`
    // to the signal task:
    //
    //   reload::Layer<EnvFilter, _>   ← swapped out on SIGHUP
    //   fmt::Layer                    ← format, always the same
    //
    // The `EnvFilter` is chosen the same way as before: RUST_LOG env-var wins,
    // then the config file's `log_level` field.
    let initial_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cfg.log_level));

    let (filter_layer, reload_handle) = reload::Layer::new(initial_filter);

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt::Layer::default())
        .init();

    tracing::info!(listen_addr = %cfg.listen_addr, "collector starting");

    // ── Database ──────────────────────────────────────────────────────────────
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

    // ── Broadcast channel ─────────────────────────────────────────────────────
    // Capacity of 256: if a slow client falls this far behind it is acceptable
    // to drop messages rather than build up unbounded memory.
    let (tx, _initial_rx) = broadcast::channel::<String>(256);

    // ── Reloadable config ─────────────────────────────────────────────────────
    let reloadable = Arc::new(ArcSwap::from_pointee(ReloadableConfig::from(&cfg)));

    // ── Retention task ────────────────────────────────────────────────────────
    // Pass the ArcSwap so the task re-reads `retention_days` on each iteration.
    retention::spawn(pool.clone(), Arc::clone(&reloadable));

    // ── Signal task ───────────────────────────────────────────────────────────
    // Owns the OS signal stream, the config path, the ArcSwap, and the
    // tracing reload handle.  Spawned before we start serving so it is ready
    // from the first request.
    if let Some(path) = config_path.clone() {
        spawn_signal_task(path, Arc::clone(&reloadable), reload_handle);
    } else {
        // No config file path available — hot-reload would have nowhere to
        // read from.  Spawn a no-op task so the type-system stays consistent.
        tracing::warn!("no config file path available; hot-reload is disabled");
        spawn_noop_signal_task();
    }

    // ── Router / AppState ─────────────────────────────────────────────────────
    let state = AppState {
        pool,
        reloadable,
        tx,
        hmac_secret: cfg.hmac_secret,
    };
    let app = api::router(state, &cfg.dashboard_dir);

    // ── Bind ──────────────────────────────────────────────────────────────────
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

// ---------------------------------------------------------------------------
// Signal task
// ---------------------------------------------------------------------------

/// Type alias for the tracing reload handle we carry into the signal task.
///
/// The inner type is `EnvFilter` wrapped in the `reload` layer.  We use a
/// type alias to keep the `spawn_signal_task` signature readable.
type ReloadHandle = reload::Handle<EnvFilter, tracing_subscriber::Registry>;

/// Spawn a dedicated Tokio task that waits for the platform reload signal
/// (`SIGHUP` on Unix, `Ctrl+Break` on Windows) and hot-applies every
/// reloadable config field.
fn spawn_signal_task(
    config_path: PathBuf,
    reloadable: Arc<ArcSwap<ReloadableConfig>>,
    reload_handle: ReloadHandle,
) {
    tokio::spawn(async move {
        // Build the platform-specific signal stream once, outside the loop.
        #[cfg(unix)]
        let mut signal_stream = {
            use tokio::signal::unix::{signal, SignalKind};
            signal(SignalKind::hangup()).expect("failed to register SIGHUP handler")
        };

        #[cfg(windows)]
        let mut signal_stream = {
            use tokio::signal::windows::ctrl_break;
            ctrl_break().expect("failed to register Ctrl+Break handler")
        };

        loop {
            // Block until we receive the reload signal.
            signal_stream.recv().await;

            tracing::info!(path = %config_path.display(), "received reload signal; re-reading config");

            // Attempt to re-read and parse the config file.
            let new_cfg = match config::load(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(error = %e, "config reload failed — keeping current config");
                    continue;
                }
            };

            // Build the candidate reloadable snapshot.
            let new_reloadable = ReloadableConfig::from(&new_cfg);

            // Diff against the current values and collect a description of
            // what actually changed for the info log line.
            let old = reloadable.load();
            let mut changed: Vec<&'static str> = Vec::new();

            if old.offline_threshold_secs != new_reloadable.offline_threshold_secs {
                changed.push("offline_threshold_secs");
            }
            if old.retention_days != new_reloadable.retention_days {
                changed.push("retention_days");
            }
            if old.log_level != new_reloadable.log_level {
                changed.push("log_level");
            }

            // Apply log_level change first so that subsequent log lines
            // already reflect the new level.
            if old.log_level != new_reloadable.log_level {
                let new_filter = EnvFilter::new(&new_reloadable.log_level);
                match reload_handle.modify(|f| *f = new_filter) {
                    Ok(()) => {}
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to reload log filter");
                    }
                }
            }

            // Atomically swap in the new reloadable config.
            reloadable.store(Arc::new(new_reloadable));

            if changed.is_empty() {
                tracing::info!("config reloaded — no reloadable fields changed");
            } else {
                tracing::info!(
                    changed = %changed.join(", "),
                    offline_threshold_secs = new_cfg.offline_threshold_secs,
                    retention_days = new_cfg.retention_days,
                    log_level = %new_cfg.log_level,
                    "config reloaded successfully"
                );
            }
        }
    });
}

/// Spawn a task that still listens for the reload signal but does nothing when
/// it fires, because no config path was provided at startup.
fn spawn_noop_signal_task() {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut s = match signal(SignalKind::hangup()) {
                Ok(s) => s,
                Err(_) => return,
            };
            loop {
                s.recv().await;
                tracing::warn!("received SIGHUP but hot-reload is disabled (no config file path)");
            }
        }

        #[cfg(windows)]
        {
            use tokio::signal::windows::ctrl_break;
            let mut cb = match ctrl_break() {
                Ok(cb) => cb,
                Err(_) => return,
            };
            loop {
                cb.recv().await;
                tracing::warn!(
                    "received Ctrl+Break but hot-reload is disabled (no config file path)"
                );
            }
        }

        // On platforms that are neither unix nor windows, this task exits
        // immediately and silently — there is no signal to handle.
        #[cfg(not(any(unix, windows)))]
        {}
    });
}
