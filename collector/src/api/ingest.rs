use crate::db::tags_to_str;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use shared::auth::{verify_auth_header, AuthError};
use shared::MetricPayload;

use super::errors::ProblemDetail;
use crate::AppState;

/// `POST /api/v1/metrics`
///
/// Accepts a [`MetricPayload`] JSON body, persists the agent record, metric row,
/// and disk readings inside a single transaction, then returns 200 OK.
///
/// Authentication is performed first: when `hmac_secret` is non-empty the
/// request must carry a valid `Authorization: HMAC …` header whose timestamp
/// falls within 300 s of the collector's clock.  Missing or invalid credentials
/// yield 401 Unauthorized.
///
/// Error mapping (RFC 7807 `ProblemDetail` body on all errors):
/// - Missing / invalid HMAC signature   → 401 Unauthorized
/// - Malformed / missing-field body     → 400 Bad Request
/// - Database failure                   → 503 Service Unavailable
pub async fn ingest_metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, ProblemDetail> {
    // ── Auth ──────────────────────────────────────────────────────────────────
    let auth_header_value = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if let Err(e) = verify_auth_header(&state.hmac_secret, auth_header_value, &body) {
        let (status, detail) = match e {
            AuthError::Missing => (
                StatusCode::UNAUTHORIZED,
                "Authorization header is missing.".to_string(),
            ),
            AuthError::Malformed => (
                StatusCode::UNAUTHORIZED,
                "Authorization header is malformed.".to_string(),
            ),
            AuthError::InvalidSignature => (
                StatusCode::UNAUTHORIZED,
                "HMAC signature is invalid.".to_string(),
            ),
            AuthError::TimestampExpired => (
                StatusCode::UNAUTHORIZED,
                "Request timestamp is outside the allowed 300 s window.".to_string(),
            ),
        };
        tracing::debug!(error = %e, "rejected request due to auth failure");
        return Err(ProblemDetail::new(status, detail));
    }

    // ── Parse JSON ────────────────────────────────────────────────────────────
    let payload: MetricPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!(error = %e, "rejected malformed ingest payload");
            return Err(ProblemDetail::new(
                StatusCode::BAD_REQUEST,
                format!("Invalid request body: {e}"),
            ));
        }
    };

    // ── Persist ───────────────────────────────────────────────────────────────
    match persist(&state, &payload).await {
        Ok(()) => {
            // Fire-and-forget: build and push the WS event without blocking
            // the HTTP response back to the agent.
            tokio::spawn(super::ws::broadcast_metric_update(
                state.pool.clone(),
                state.tx.clone(),
                payload,
            ));
            Ok(StatusCode::OK)
        }
        Err(e) => {
            tracing::error!(error = %e, "database error during metric ingest");
            Err(ProblemDetail::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "A database error occurred while persisting metrics; please retry.",
            ))
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
        INSERT INTO agents (agent_id, first_seen_at, last_seen_at, duplicate_flag, tags)
        VALUES (?, ?, ?, 0, ?)
        ON CONFLICT(agent_id) DO UPDATE SET
            last_seen_at = excluded.last_seen_at,
            tags = excluded.tags
        "#,
    )
    .bind(&payload.agent_id)
    .bind(payload.timestamp)
    .bind(payload.timestamp)
    .bind(tags_to_str(&payload.tags))
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
