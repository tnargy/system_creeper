pub mod agents;
pub mod ingest;
pub mod thresholds;
pub mod ws;

use axum::{
    routing::{get, post, put},
    Router,
};
use tower_http::cors::CorsLayer;

use crate::AppState;

/// Build the full application [`Router`].
///
/// All routes are mounted under `/api/v1` and a permissive CORS layer
/// (allows any origin / method / header) is applied to every response,
/// as required for development use.
pub fn router(state: AppState) -> Router {
    Router::new()
        // Ingest
        .route("/api/v1/metrics", post(ingest::ingest_metrics))
        // Agents
        .route("/api/v1/agents", get(agents::list_agents))
        .route("/api/v1/agents/:agent_id/snapshot", get(agents::get_snapshot))
        .route("/api/v1/agents/:agent_id/history", get(agents::get_history))
        // Thresholds
        .route("/api/v1/thresholds", get(thresholds::list_thresholds).post(thresholds::create_threshold))
        .route("/api/v1/thresholds/:id", put(thresholds::update_threshold).delete(thresholds::delete_threshold))
        // WebSocket
        .route("/ws", get(ws::ws_handler))
        .with_state(state)
        .layer(CorsLayer::permissive())
}
