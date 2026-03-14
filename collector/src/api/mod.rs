pub mod agents;
pub mod errors;
pub mod health;
pub mod ingest;
pub mod thresholds;
pub mod ws;

use axum::{
    http::header,
    response::IntoResponse,
    routing::{get, post, put},
    Router,
};
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
};

use crate::AppState;

/// Serves the embedded [`openapi.yaml`](../../openapi.yaml) specification.
///
/// The spec is embedded at compile time via [`include_str!`] so that the
/// binary is fully self-contained and does not depend on the file being
/// present at runtime.
async fn openapi_spec() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/yaml; charset=utf-8")],
        include_str!("../../openapi.yaml"),
    )
}

/// Build the full application [`Router`].
///
/// All routes are mounted under `/api/v1` and a permissive CORS layer
/// (allows any origin / method / header) is applied to every response,
/// as required for development use.
///
/// The dashboard SPA is served from `dashboard_dir` at `/`.  Any path not
/// matched by an API route falls back to `index.html` so that client-side
/// routing works correctly.
pub fn router(state: AppState, dashboard_dir: impl AsRef<std::path::Path>) -> Router {
    let dashboard_dir = dashboard_dir.as_ref();
    let fallback = ServeFile::new(dashboard_dir.join("index.html"));
    let serve_dir = ServeDir::new(dashboard_dir).not_found_service(fallback);

    Router::new()
        // Operations
        .route("/health", get(health::health_check))
        .route("/openapi.yaml", get(openapi_spec))
        // Ingest
        .route("/api/v1/metrics", post(ingest::ingest_metrics))
        // Agents
        .route("/api/v1/agents", get(agents::list_agents))
        .route(
            "/api/v1/agents/{agent_id}/snapshot",
            get(agents::get_snapshot),
        )
        .route(
            "/api/v1/agents/{agent_id}/history",
            get(agents::get_history),
        )
        // Thresholds
        .route(
            "/api/v1/thresholds",
            get(thresholds::list_thresholds).post(thresholds::create_threshold),
        )
        .route(
            "/api/v1/thresholds/{id}",
            put(thresholds::update_threshold).delete(thresholds::delete_threshold),
        )
        // WebSocket
        .route("/ws", get(ws::ws_handler))
        .with_state(state)
        .layer(CorsLayer::permissive())
        // Static files — any request not matched above falls through to the
        // dashboard SPA; unknown paths within the SPA are served index.html.
        .fallback_service(serve_dir)
}
