use axum::{
    extract::{rejection::JsonRejection, State},
    http::StatusCode,
    Json,
};
use shared::MetricPayload;

use crate::AppState;

/// `POST /api/v1/metrics`
///
/// Accepts a [`MetricPayload`] JSON body, persists the agent record, metric row,
/// and disk readings inside a single transaction, then returns 200 OK.
///
/// Error mapping:
/// - Malformed / missing-field body → 400 Bad Request
/// - Database failure → 503 Service Unavailable
pub async fn ingest_metrics(
    State(state): State<AppState>,
    payload: Result<Json<MetricPayload>, JsonRejection>,
) -> StatusCode {
    let Json(payload) = match payload {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!(error = %e, "rejected malformed ingest payload");
            return StatusCode::BAD_REQUEST;
        }
    };

    match persist(&state, &payload).await {
        Ok(()) => {
            // Fire-and-forget: build and push the WS event without blocking
            // the HTTP response back to the agent.
            tokio::spawn(super::ws::broadcast_metric_update(
                state.pool.clone(),
                state.tx.clone(),
                payload,
            ));
            StatusCode::OK
        }
        Err(e) => {
            tracing::error!(error = %e, "database error during metric ingest");
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}

/// Persists the full [`MetricPayload`] (agent upsert + metric row + disk readings)
/// inside a single SQLite transaction.
async fn persist(state: &AppState, payload: &MetricPayload) -> Result<(), sqlx::Error> {
    let mut tx = state.pool.begin().await?;

    // Upsert the agent — sets first_seen_at only on first appearance.
    sqlx::query(
        r#"
        INSERT INTO agents (agent_id, first_seen_at, last_seen_at, duplicate_flag)
        VALUES (?, ?, ?, 0)
        ON CONFLICT(agent_id) DO UPDATE SET last_seen_at = excluded.last_seen_at
        "#,
    )
    .bind(&payload.agent_id)
    .bind(payload.timestamp)
    .bind(payload.timestamp)
    .execute(&mut *tx)
    .await?;

    // Insert the metric row.
    let result = sqlx::query(
        r#"
        INSERT INTO metrics (
            agent_id, timestamp,
            cpu_percent,
            memory_used_bytes, memory_total_bytes, memory_percent,
            network_bytes_in,  network_bytes_out,
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
    .execute(&mut *tx)
    .await?;

    let metric_id = result.last_insert_rowid();

    // Bulk-insert disk readings (skipped when there are none).
    if !payload.disks.is_empty() {
        let mut qb = sqlx::QueryBuilder::new(
            "INSERT INTO disk_readings (metric_id, mount_point, used_bytes, total_bytes, percent) ",
        );
        qb.push_values(&payload.disks, |mut b, disk| {
            b.push_bind(metric_id)
                .push_bind(&disk.mount_point)
                .push_bind(disk.used_bytes as i64)
                .push_bind(disk.total_bytes as i64)
                .push_bind(disk.percent);
        });
        qb.build().execute(&mut *tx).await?;
    }

    tx.commit().await?;
    Ok(())
}
