use axum::{
    extract::{State, WebSocketUpgrade},
    response::Response,
};
use axum::extract::ws::{Message, WebSocket};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::broadcast;

use crate::{db, AppState};

// ---------------------------------------------------------------------------
// WS message schema
// ---------------------------------------------------------------------------

/// The JSON message pushed to every dashboard client after each metric persist.
/// Field names and structure must match the schema in plan.md exactly.
#[derive(Serialize)]
struct MetricUpdateEvent {
    event: &'static str,
    agent_id: String,
    timestamp: DateTime<Utc>,
    status: String,
    cpu_percent: f64,
    memory: MemoryMsg,
    disks: Vec<DiskMsg>,
    network: NetworkMsg,
    uptime_seconds: u64,
    duplicate_flag: bool,
}

#[derive(Serialize)]
struct MemoryMsg {
    used_bytes: u64,
    total_bytes: u64,
    percent: f64,
}

#[derive(Serialize)]
struct DiskMsg {
    mount_point: String,
    used_bytes: u64,
    total_bytes: u64,
    percent: f64,
}

#[derive(Serialize)]
struct NetworkMsg {
    bytes_in: u64,
    bytes_out: u64,
}

// ---------------------------------------------------------------------------
// Status computation
// ---------------------------------------------------------------------------

/// Compute the agent status string from raw metric values.
///
/// Since this is called immediately after a successful ingest, the agent is
/// by definition reachable — we only need to check threshold breaches.
/// Returns one of `"online"`, `"warning"`, or `"critical"`.
pub(crate) fn compute_status_for_broadcast(
    cpu_percent: f64,
    memory_percent: f64,
    max_disk_percent: f64,
    thresholds: &[db::Threshold],
    agent_id: &str,
) -> &'static str {
    let applicable = thresholds
        .iter()
        .filter(|t| t.agent_id.is_none() || t.agent_id.as_deref() == Some(agent_id));

    let mut status = "online";
    for t in applicable {
        let value = match t.metric_name.as_str() {
            "cpu" => cpu_percent,
            "memory" => memory_percent,
            "disk" => max_disk_percent,
            _ => continue,
        };
        if t.critical_value > 0.0 && value >= t.critical_value {
            return "critical";
        }
        if t.warning_value > 0.0 && value >= t.warning_value {
            status = "warning";
        }
    }
    status
}

// ---------------------------------------------------------------------------
// Broadcast helper — called from the ingest handler after a successful persist
// ---------------------------------------------------------------------------

/// Build and broadcast a `metric_update` JSON message to all connected clients.
///
/// This is spawned as a background task so it never delays the HTTP 200 response
/// sent back to the agent.  If there are no connected clients the function
/// returns immediately without touching the database.
pub(crate) async fn broadcast_metric_update(
    pool: db::Db,
    tx: broadcast::Sender<String>,
    payload: shared::MetricPayload,
) {
    // Skip the DB round-trips when nobody is listening.
    if tx.receiver_count() == 0 {
        return;
    }

    let (thresholds, duplicate_flag) = match tokio::try_join!(
        db::queries::get_thresholds(&pool),
        db::queries::get_agent_duplicate_flag(&pool, &payload.agent_id),
    ) {
        Ok(pair) => pair,
        Err(e) => {
            tracing::warn!(error = %e, "failed to fetch data for WS broadcast; skipping");
            return;
        }
    };

    let max_disk = payload
        .disks
        .iter()
        .map(|d| d.percent)
        .fold(f64::NEG_INFINITY, f64::max);

    let status = compute_status_for_broadcast(
        payload.cpu_percent,
        payload.memory.percent,
        max_disk,
        &thresholds,
        &payload.agent_id,
    );

    let event = MetricUpdateEvent {
        event: "metric_update",
        agent_id: payload.agent_id,
        timestamp: payload.timestamp,
        status: status.to_string(),
        cpu_percent: payload.cpu_percent,
        memory: MemoryMsg {
            used_bytes: payload.memory.used_bytes,
            total_bytes: payload.memory.total_bytes,
            percent: payload.memory.percent,
        },
        disks: payload
            .disks
            .into_iter()
            .map(|d| DiskMsg {
                mount_point: d.mount_point,
                used_bytes: d.used_bytes,
                total_bytes: d.total_bytes,
                percent: d.percent,
            })
            .collect(),
        network: NetworkMsg {
            bytes_in: payload.network.bytes_in,
            bytes_out: payload.network.bytes_out,
        },
        uptime_seconds: payload.uptime_seconds,
        duplicate_flag,
    };

    match serde_json::to_string(&event) {
        Ok(json) => {
            // send() returns Err only when all receivers have been dropped —
            // that is a normal race condition, not an error worth logging.
            let _ = tx.send(json);
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to serialise WS broadcast message");
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP → WebSocket upgrade handler
// ---------------------------------------------------------------------------

/// `GET /ws` — upgrades the HTTP connection to a WebSocket.
///
/// After the upgrade the handler subscribes to the broadcast channel and
/// forwards every JSON message to the connected client.  When the client
/// disconnects (the send returns an error), the handler exits silently: the
/// broadcast `Receiver` is dropped and no other connection is affected.
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state.tx.subscribe()))
}

async fn handle_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    loop {
        match rx.recv().await {
            Ok(json) => {
                if socket.send(Message::Text(json.into())).await.is_err() {
                    // Client has gone away — exit cleanly.
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                // The client fell behind; messages were dropped by the channel.
                // Log and continue — do not disconnect.
                tracing::warn!(skipped = n, "WebSocket client lagged; messages were dropped");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}
