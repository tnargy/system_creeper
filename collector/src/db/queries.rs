use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use shared::{DiskInfo, MetricPayload};

use crate::db::{AgentSummary, DiskReading, MetricRow, MetricSnapshot, Threshold};

// ---------------------------------------------------------------------------
// Agent operations
// ---------------------------------------------------------------------------

/// Insert a new agent row, or update `last_seen_at` if the agent already exists.
/// `first_seen_at` is preserved on subsequent calls.
pub async fn upsert_agent(
    pool: &SqlitePool,
    agent_id: &str,
    timestamp: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO agents (agent_id, first_seen_at, last_seen_at, duplicate_flag)
        VALUES (?, ?, ?, 0)
        ON CONFLICT(agent_id) DO UPDATE SET last_seen_at = excluded.last_seen_at
        "#,
    )
    .bind(agent_id)
    .bind(timestamp)
    .bind(timestamp)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Metric insert
// ---------------------------------------------------------------------------

/// Insert a metric payload row and return the new row's `id`.
pub async fn insert_metric(
    pool: &SqlitePool,
    payload: &MetricPayload,
) -> Result<i64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO metrics (
            agent_id, timestamp,
            cpu_percent,
            memory_used_bytes, memory_total_bytes, memory_percent,
            network_bytes_in, network_bytes_out,
            uptime_seconds
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&payload.agent_id)
    .bind(payload.timestamp)
    .bind(payload.cpu_percent)
    .bind(payload.memory.used_bytes as i64)
    .bind(payload.memory.total_bytes as i64)
    .bind(payload.memory.percent)
    .bind(payload.network.bytes_in as i64)
    .bind(payload.network.bytes_out as i64)
    .bind(payload.uptime_seconds as i64)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

/// Bulk-insert disk readings for a previously-inserted metric row.
pub async fn insert_disk_readings(
    pool: &SqlitePool,
    metric_id: i64,
    disks: &[DiskInfo],
) -> Result<(), sqlx::Error> {
    if disks.is_empty() {
        return Ok(());
    }
    // Use QueryBuilder to emit a single multi-row INSERT rather than N round-trips.
    let mut qb = sqlx::QueryBuilder::new(
        "INSERT INTO disk_readings (metric_id, mount_point, used_bytes, total_bytes, percent) ",
    );
    qb.push_values(disks, |mut b, disk| {
        b.push_bind(metric_id)
            .push_bind(&disk.mount_point)
            .push_bind(disk.used_bytes as i64)
            .push_bind(disk.total_bytes as i64)
            .push_bind(disk.percent);
    });
    qb.build().execute(pool).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Agent summary / snapshot / history
// ---------------------------------------------------------------------------

/// Intermediate flat row returned by the agents-with-latest-metric join.
#[derive(sqlx::FromRow)]
struct AgentMetricRow {
    agent_id: String,
    first_seen_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    duplicate_flag: i64,
    metric_id: Option<i64>,
    metric_timestamp: Option<DateTime<Utc>>,
    cpu_percent: Option<f64>,
    memory_used_bytes: Option<i64>,
    memory_total_bytes: Option<i64>,
    memory_percent: Option<f64>,
    network_bytes_in: Option<i64>,
    network_bytes_out: Option<i64>,
    uptime_seconds: Option<i64>,
}

/// Return all agents together with their most-recent metric snapshot (if any).
pub async fn get_agents_summary(pool: &SqlitePool) -> Result<Vec<AgentSummary>, sqlx::Error> {
    let rows = sqlx::query_as::<_, AgentMetricRow>(
        r#"
        SELECT
            a.agent_id,
            a.first_seen_at,
            a.last_seen_at,
            a.duplicate_flag,
            m.id            AS metric_id,
            m.timestamp     AS metric_timestamp,
            m.cpu_percent,
            m.memory_used_bytes,
            m.memory_total_bytes,
            m.memory_percent,
            m.network_bytes_in,
            m.network_bytes_out,
            m.uptime_seconds
        FROM agents a
        LEFT JOIN metrics m
            ON  m.agent_id = a.agent_id
            AND m.id = (
                SELECT id FROM metrics
                WHERE  agent_id = a.agent_id
                ORDER BY timestamp DESC
                LIMIT 1
            )
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut summaries = Vec::with_capacity(rows.len());
    for row in rows {
        let latest_metric = match row.metric_id {
            None => None,
            Some(id) => {
                // Safety: the LEFT JOIN guarantees that if metric_id IS NOT NULL
                // then all other metric columns are also NOT NULL.
                let ts = row.metric_timestamp.ok_or_else(|| {
                    sqlx::Error::Protocol(
                        "metric_timestamp was NULL when metric_id is NOT NULL".into(),
                    )
                })?;
                let disks = disks_for_metric(pool, id).await?;
                Some(MetricSnapshot {
                    id,
                    agent_id: row.agent_id.clone(),
                    timestamp: ts,
                    cpu_percent: row.cpu_percent.unwrap_or_default(),
                    memory_used_bytes: row.memory_used_bytes.unwrap_or_default(),
                    memory_total_bytes: row.memory_total_bytes.unwrap_or_default(),
                    memory_percent: row.memory_percent.unwrap_or_default(),
                    network_bytes_in: row.network_bytes_in.unwrap_or_default(),
                    network_bytes_out: row.network_bytes_out.unwrap_or_default(),
                    uptime_seconds: row.uptime_seconds.unwrap_or_default(),
                    disks,
                })
            }
        };
        summaries.push(AgentSummary {
            agent_id: row.agent_id,
            first_seen_at: row.first_seen_at,
            last_seen_at: row.last_seen_at,
            duplicate_flag: row.duplicate_flag != 0,
            latest_metric,
        });
    }
    Ok(summaries)
}

/// Return the most-recent metric snapshot for a single agent, or `None` if the
/// agent is unknown or has no metrics yet.
pub async fn get_snapshot(
    pool: &SqlitePool,
    agent_id: &str,
) -> Result<Option<MetricSnapshot>, sqlx::Error> {
    let row_opt = sqlx::query_as::<_, MetricRow>(
        r#"
        SELECT id, agent_id, timestamp,
               cpu_percent,
               memory_used_bytes, memory_total_bytes, memory_percent,
               network_bytes_in, network_bytes_out,
               uptime_seconds
        FROM metrics
        WHERE agent_id = ?
        ORDER BY timestamp DESC
        LIMIT 1
        "#,
    )
    .bind(agent_id)
    .fetch_optional(pool)
    .await?;

    let row = match row_opt {
        None => return Ok(None),
        Some(r) => r,
    };
    let disks = disks_for_metric(pool, row.id).await?;
    Ok(Some(MetricSnapshot {
        id: row.id,
        agent_id: row.agent_id,
        timestamp: row.timestamp,
        cpu_percent: row.cpu_percent,
        memory_used_bytes: row.memory_used_bytes,
        memory_total_bytes: row.memory_total_bytes,
        memory_percent: row.memory_percent,
        network_bytes_in: row.network_bytes_in,
        network_bytes_out: row.network_bytes_out,
        uptime_seconds: row.uptime_seconds,
        disks,
    }))
}

/// Return all metric snapshots for `agent_id` with `timestamp >= since`, ordered
/// oldest-first.
pub async fn get_history(
    pool: &SqlitePool,
    agent_id: &str,
    since: DateTime<Utc>,
) -> Result<Vec<MetricSnapshot>, sqlx::Error> {
    let rows = sqlx::query_as::<_, MetricRow>(
        r#"
        SELECT id, agent_id, timestamp,
               cpu_percent,
               memory_used_bytes, memory_total_bytes, memory_percent,
               network_bytes_in, network_bytes_out,
               uptime_seconds
        FROM metrics
        WHERE agent_id = ? AND timestamp >= ?
        ORDER BY timestamp ASC
        "#,
    )
    .bind(agent_id)
    .bind(since)
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok(vec![]);
    }

    // Fetch disk readings for all returned metrics in a single query.
    let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
    let disks = disks_for_metrics(pool, &ids).await?;

    let snapshots = rows
        .into_iter()
        .map(|row| {
            let row_disks: Vec<DiskReading> = disks
                .iter()
                .filter(|d| d.metric_id == row.id)
                .cloned()
                .collect();
            MetricSnapshot {
                id: row.id,
                agent_id: row.agent_id,
                timestamp: row.timestamp,
                cpu_percent: row.cpu_percent,
                memory_used_bytes: row.memory_used_bytes,
                memory_total_bytes: row.memory_total_bytes,
                memory_percent: row.memory_percent,
                network_bytes_in: row.network_bytes_in,
                network_bytes_out: row.network_bytes_out,
                uptime_seconds: row.uptime_seconds,
                disks: row_disks,
            }
        })
        .collect();
    Ok(snapshots)
}

// ---------------------------------------------------------------------------
// Threshold operations
// ---------------------------------------------------------------------------

/// Return all threshold rows.
pub async fn get_thresholds(pool: &SqlitePool) -> Result<Vec<Threshold>, sqlx::Error> {
    sqlx::query_as::<_, Threshold>("SELECT id, agent_id, metric_name, warning_value, critical_value FROM thresholds")
        .fetch_all(pool)
        .await
}

/// Insert a new threshold row and return the created row.
pub async fn upsert_threshold(
    pool: &SqlitePool,
    agent_id: Option<&str>,
    metric_name: &str,
    warning_value: f64,
    critical_value: f64,
) -> Result<Threshold, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO thresholds (agent_id, metric_name, warning_value, critical_value)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(agent_id)
    .bind(metric_name)
    .bind(warning_value)
    .bind(critical_value)
    .execute(pool)
    .await?;

    let id = result.last_insert_rowid();
    sqlx::query_as::<_, Threshold>(
        "SELECT id, agent_id, metric_name, warning_value, critical_value FROM thresholds WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await
}

/// Update `warning_value` and `critical_value` for the given threshold id.
/// Returns the updated row, or `None` if no row with that id exists.
pub async fn update_threshold(
    pool: &SqlitePool,
    id: i64,
    warning_value: f64,
    critical_value: f64,
) -> Result<Option<Threshold>, sqlx::Error> {
    let rows_affected = sqlx::query(
        "UPDATE thresholds SET warning_value = ?, critical_value = ? WHERE id = ?",
    )
    .bind(warning_value)
    .bind(critical_value)
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        return Ok(None);
    }
    sqlx::query_as::<_, Threshold>(
        "SELECT id, agent_id, metric_name, warning_value, critical_value FROM thresholds WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Delete the threshold with the given id.  Returns `true` if a row was deleted.
pub async fn delete_threshold(pool: &SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
    let rows_affected = sqlx::query("DELETE FROM thresholds WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(rows_affected > 0)
}

/// Return the `duplicate_flag` for the given agent, or `false` if the agent
/// is not found.  Used when building the WebSocket broadcast message.
pub async fn get_agent_duplicate_flag(
    pool: &SqlitePool,
    agent_id: &str,
) -> Result<bool, sqlx::Error> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT duplicate_flag FROM agents WHERE agent_id = ?")
            .bind(agent_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(flag,)| flag != 0).unwrap_or(false))
}

// ---------------------------------------------------------------------------
// Retention
// ---------------------------------------------------------------------------

/// Delete all metric rows (and their cascading disk readings) older than `cutoff`.
/// Returns the number of rows deleted.
pub async fn delete_old_metrics(
    pool: &SqlitePool,
    cutoff: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    let rows_affected = sqlx::query("DELETE FROM metrics WHERE timestamp < ?")
        .bind(cutoff)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(rows_affected)
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

async fn disks_for_metric(
    pool: &SqlitePool,
    metric_id: i64,
) -> Result<Vec<DiskReading>, sqlx::Error> {
    sqlx::query_as::<_, DiskReading>(
        "SELECT id, metric_id, mount_point, used_bytes, total_bytes, percent FROM disk_readings WHERE metric_id = ?",
    )
    .bind(metric_id)
    .fetch_all(pool)
    .await
}

async fn disks_for_metrics(
    pool: &SqlitePool,
    metric_ids: &[i64],
) -> Result<Vec<DiskReading>, sqlx::Error> {
    if metric_ids.is_empty() {
        return Ok(vec![]);
    }
    // Build a parameterised IN clause.  Each placeholder is the literal '?'
    // character, never data supplied by users, so there is no SQL injection
    // risk here.  All actual values are supplied through the bind calls below.
    let placeholders = metric_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT id, metric_id, mount_point, used_bytes, total_bytes, percent \
         FROM disk_readings WHERE metric_id IN ({placeholders})"
    );
    let mut q = sqlx::query_as::<_, DiskReading>(&sql);
    for id in metric_ids {
        q = q.bind(*id);
    }
    q.fetch_all(pool).await
}
