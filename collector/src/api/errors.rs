use axum::{
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// RFC 7807 / RFC 9457 — Problem Details for HTTP APIs.
///
/// All error responses from the collector API use this JSON body with
/// `Content-Type: application/problem+json` as the media type.  Structured
/// error bodies let clients distinguish error cases programmatically and
/// display meaningful messages to end users.
///
/// Reference: <https://www.rfc-editor.org/rfc/rfc7807>
#[derive(Debug, Serialize)]
pub struct ProblemDetail {
    /// A URI reference that identifies the problem type.  Uses `"about:blank"`
    /// when no specific documentation URI is defined, per RFC 7807 §4.2.
    #[serde(rename = "type")]
    pub problem_type: String,
    /// Short, human-readable summary of the problem type (mirrors the HTTP
    /// reason phrase and should not change between occurrences of the same
    /// problem type).
    pub title: String,
    /// The HTTP status code for this occurrence of the problem.
    pub status: u16,
    /// Human-readable explanation specific to this occurrence of the problem.
    pub detail: String,
}

impl ProblemDetail {
    /// Construct a [`ProblemDetail`] from a [`StatusCode`] and a detail string.
    ///
    /// `title` is derived from the canonical HTTP reason phrase and
    /// `problem_type` defaults to `"about:blank"`.
    pub fn new(status: StatusCode, detail: impl Into<String>) -> Self {
        Self {
            problem_type: "about:blank".to_string(),
            title: status
                .canonical_reason()
                .unwrap_or("Unknown Error")
                .to_string(),
            status: status.as_u16(),
            detail: detail.into(),
        }
    }
}

impl IntoResponse for ProblemDetail {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut resp = (status, Json(&self)).into_response();
        // RFC 7807 §3: error responses MUST use "application/problem+json".
        resp.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn new_sets_correct_status_and_title() {
        let pd = ProblemDetail::new(StatusCode::NOT_FOUND, "Agent 'x' was not found.");
        assert_eq!(pd.status, 404);
        assert_eq!(pd.title, "Not Found");
        assert_eq!(pd.problem_type, "about:blank");
        assert_eq!(pd.detail, "Agent 'x' was not found.");
    }

    #[test]
    fn new_sets_correct_status_for_service_unavailable() {
        let pd = ProblemDetail::new(StatusCode::SERVICE_UNAVAILABLE, "Database error.");
        assert_eq!(pd.status, 503);
        assert_eq!(pd.title, "Service Unavailable");
    }

    #[test]
    fn new_sets_correct_status_for_bad_request() {
        let pd = ProblemDetail::new(StatusCode::BAD_REQUEST, "Invalid body.");
        assert_eq!(pd.status, 400);
        assert_eq!(pd.title, "Bad Request");
    }

    #[test]
    fn serializes_with_renamed_type_field() {
        let pd = ProblemDetail::new(StatusCode::NOT_FOUND, "detail text");
        let json = serde_json::to_value(&pd).unwrap();
        // "type" (not "problem_type") must appear in the serialised output.
        assert!(json.get("type").is_some());
        assert!(json.get("problem_type").is_none());
        assert_eq!(json["type"], "about:blank");
        assert_eq!(json["status"], 404);
        assert_eq!(json["title"], "Not Found");
        assert_eq!(json["detail"], "detail text");
    }
}
