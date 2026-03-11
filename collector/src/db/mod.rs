pub mod queries;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePool},
    Error,
};
use std::{path::Path, str::FromStr};

pub type Db = SqlitePool;

/// A row from the `agents` table joined with the agent's most-recent metric.
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentSummary {
    pub agent_id: String,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub duplicate_flag: bool,
    pub latest_metric: Option<MetricSnapshot>,
}

/// A raw row from the `metrics` table (no disk readings).
/// Used only for `sqlx::FromRow`; consumers should use [`MetricSnapshot`].
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct MetricRow {
    pub id: i64,
    pub agent_id: String,
    pub timestamp: DateTime<Utc>,
    pub cpu_percent: f64,
    pub memory_used_bytes: i64,
    pub memory_total_bytes: i64,
    pub memory_percent: f64,
    pub network_bytes_in: i64,
    pub network_bytes_out: i64,
    pub uptime_seconds: i64,
}

/// A row from the `metrics` table together with its associated disk readings.
#[derive(Debug, Serialize, Deserialize)]
pub struct MetricSnapshot {
    pub id: i64,
    pub agent_id: String,
    pub timestamp: DateTime<Utc>,
    pub cpu_percent: f64,
    pub memory_used_bytes: i64,
    pub memory_total_bytes: i64,
    pub memory_percent: f64,
    pub network_bytes_in: i64,
    pub network_bytes_out: i64,
    pub uptime_seconds: i64,
    /// Disk readings fetched via a separate query.
    pub disks: Vec<DiskReading>,
}

/// A row from the `disk_readings` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DiskReading {
    pub id: i64,
    pub metric_id: i64,
    pub mount_point: String,
    pub used_bytes: i64,
    pub total_bytes: i64,
    pub percent: f64,
}

/// A row from the `thresholds` table.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Threshold {
    pub id: i64,
    pub agent_id: Option<String>,
    pub metric_name: String,
    pub warning_value: f64,
    pub critical_value: f64,
}

/// Initialise the SQLite connection pool, creating the database file if it
/// does not yet exist, and run any pending migrations.
pub async fn init_pool(database_path: &str) -> Result<Db, Error> {
    if let Some(parent) = Path::new(database_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }
    }

    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{database_path}"))?
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePool::connect_with(opts).await?;

    sqlx::migrate!("src/db/migrations").run(&pool).await?;

    Ok(pool)
}
