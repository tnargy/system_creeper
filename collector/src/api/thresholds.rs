use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use super::errors::ProblemDetail;
use crate::{
    db::{self, Threshold},
    AppState,
};

const VALID_METRIC_NAMES: &[&str] = &["cpu", "memory", "disk"];

/// `GET /api/v1/thresholds`
pub async fn list_thresholds(
    State(state): State<AppState>,
) -> Result<Json<Vec<Threshold>>, ProblemDetail> {
    db::queries::get_thresholds(&state.pool)
        .await
        .map(Json)
        .map_err(|e| {
            tracing::error!(error = %e, "database error in list_thresholds");
            ProblemDetail::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "A database error occurred while listing thresholds; please retry.",
            )
        })
}

#[derive(Deserialize)]
pub struct CreateThresholdBody {
    pub agent_id: Option<String>,
    pub metric_name: String,
    pub warning_value: f64,
    pub critical_value: f64,
}

/// `POST /api/v1/thresholds`
///
/// `metric_name` must be one of `"cpu"`, `"memory"`, `"disk"`; returns 400 otherwise.
/// Returns 201 with the created row on success.
pub async fn create_threshold(
    State(state): State<AppState>,
    payload: Result<Json<CreateThresholdBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Threshold>), ProblemDetail> {
    let Json(body) = payload.map_err(|e| {
        tracing::debug!(error = %e, "rejected malformed create-threshold body");
        ProblemDetail::new(
            StatusCode::BAD_REQUEST,
            format!("Invalid request body: {e}"),
        )
    })?;

    if !VALID_METRIC_NAMES.contains(&body.metric_name.as_str()) {
        return Err(ProblemDetail::new(
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid metric_name '{}'. Must be one of: {}.",
                body.metric_name,
                VALID_METRIC_NAMES.join(", ")
            ),
        ));
    }

    let threshold = db::queries::upsert_threshold(
        &state.pool,
        body.agent_id.as_deref(),
        &body.metric_name,
        body.warning_value,
        body.critical_value,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "database error in create_threshold");
        ProblemDetail::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "A database error occurred while creating the threshold; please retry.",
        )
    })?;

    Ok((StatusCode::CREATED, Json(threshold)))
}

#[derive(Deserialize)]
pub struct UpdateThresholdBody {
    pub warning_value: f64,
    pub critical_value: f64,
}

/// `PUT /api/v1/thresholds/:id`
///
/// Returns 200 with the updated row; 404 if the id does not exist.
pub async fn update_threshold(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    payload: Result<Json<UpdateThresholdBody>, JsonRejection>,
) -> Result<Json<Threshold>, ProblemDetail> {
    let Json(body) = payload.map_err(|e| {
        tracing::debug!(error = %e, "rejected malformed update-threshold body");
        ProblemDetail::new(
            StatusCode::BAD_REQUEST,
            format!("Invalid request body: {e}"),
        )
    })?;

    let updated =
        db::queries::update_threshold(&state.pool, id, body.warning_value, body.critical_value)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "database error in update_threshold");
                ProblemDetail::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "A database error occurred while updating the threshold; please retry.",
                )
            })?;

    match updated {
        Some(t) => Ok(Json(t)),
        None => Err(ProblemDetail::new(
            StatusCode::NOT_FOUND,
            format!("Threshold with id {id} was not found."),
        )),
    }
}

/// `DELETE /api/v1/thresholds/:id`
///
/// Returns 204 on success; 404 if the id does not exist.
pub async fn delete_threshold(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ProblemDetail> {
    match db::queries::delete_threshold(&state.pool, id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(ProblemDetail::new(
            StatusCode::NOT_FOUND,
            format!("Threshold with id {id} was not found."),
        )),
        Err(e) => {
            tracing::error!(error = %e, "database error in delete_threshold");
            Err(ProblemDetail::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "A database error occurred while deleting the threshold; please retry.",
            ))
        }
    }
}
