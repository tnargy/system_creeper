use chrono::Utc;
use tokio::time::{self, Duration};

use crate::db;

/// Spawn a background task that deletes metrics older than `retention_days`.
///
/// The task runs immediately on startup, then once every 24 hours thereafter.
/// A DB error is logged but does not crash the process — the next run is
/// still scheduled normally.
pub fn spawn(pool: db::Db, retention_days: u32) {
    tokio::spawn(async move {
        loop {
            run_once(&pool, retention_days).await;
            time::sleep(Duration::from_secs(24 * 60 * 60)).await;
        }
    });
}

async fn run_once(pool: &db::Db, retention_days: u32) {
    let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
    tracing::info!(
        cutoff = %cutoff,
        retention_days,
        "retention: running purge"
    );
    match db::queries::delete_old_metrics(pool, cutoff).await {
        Ok(deleted) => {
            tracing::info!(deleted, "retention: purge complete");
        }
        Err(e) => {
            tracing::error!(error = %e, "retention: purge failed, will retry in 24h");
        }
    }
}
