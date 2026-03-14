use gloo_net::http::Request;
use serde::Serialize;

use crate::types::{
    AgentApiResponse, AgentSummary, HistorySnapshotWire, MetricSnapshot, Threshold,
};

const BASE: &str = "/api/v1";

pub fn ws_url() -> String {
    let window = web_sys::window().expect("window unavailable in browser");
    let location = window.location();
    let protocol = location.protocol().unwrap_or_else(|_| "http:".to_string());
    let host = location
        .host()
        .unwrap_or_else(|_| "localhost:8080".to_string());
    let ws_proto = if protocol == "https:" { "wss" } else { "ws" };
    format!("{ws_proto}://{host}/ws")
}

pub async fn fetch_agents() -> Result<Vec<AgentSummary>, String> {
    let response = Request::get(&format!("{BASE}/agents"))
        .send()
        .await
        .map_err(|e| format!("GET /agents failed: {e}"))?;

    if !response.ok() {
        return Err(format!("GET /agents failed: {}", response.status()));
    }

    let body = response
        .json::<Vec<AgentApiResponse>>()
        .await
        .map_err(|e| format!("Failed to decode /agents payload: {e}"))?;

    Ok(body
        .into_iter()
        .map(|a| AgentSummary {
            agent_id: a.agent_id,
            status: a.status,
            last_seen_at: a.last_seen_at,
            duplicate_flag: a.duplicate_flag,
            tags: a.tags,
            snapshot: a.snapshot.map(|s| s.into_snapshot()),
        })
        .collect())
}

pub async fn fetch_thresholds() -> Result<Vec<Threshold>, String> {
    let response = Request::get(&format!("{BASE}/thresholds"))
        .send()
        .await
        .map_err(|e| format!("GET /thresholds failed: {e}"))?;

    if !response.ok() {
        return Err(format!("GET /thresholds failed: {}", response.status()));
    }

    response
        .json::<Vec<Threshold>>()
        .await
        .map_err(|e| format!("Failed to decode /thresholds payload: {e}"))
}

pub async fn fetch_history(agent_id: &str, range: &str) -> Result<Vec<MetricSnapshot>, String> {
    let encoded = js_sys::encode_uri_component(agent_id)
        .as_string()
        .unwrap_or_else(|| agent_id.to_owned());
    let path = format!("{BASE}/agents/{encoded}/history?range={range}");

    let response = Request::get(&path)
        .send()
        .await
        .map_err(|e| format!("GET history failed: {e}"))?;

    if !response.ok() {
        return Err(format!("GET history failed: {}", response.status()));
    }

    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed reading history response body: {e}"))?;

    let snapshots = serde_json::from_str::<Vec<HistorySnapshotWire>>(&text)
        .map_err(|e| format!("Failed to decode history payload: {e}"))?;

    Ok(snapshots
        .into_iter()
        .map(HistorySnapshotWire::into_snapshot)
        .collect())
}

#[derive(Serialize)]
struct ThresholdPayload<'a> {
    agent_id: Option<&'a str>,
    metric_name: &'a str,
    warning_value: f64,
    critical_value: f64,
}

#[derive(Serialize)]
struct ThresholdUpdatePayload {
    warning_value: f64,
    critical_value: f64,
}

pub async fn create_threshold(
    agent_id: Option<&str>,
    metric_name: &str,
    warning_value: f64,
    critical_value: f64,
) -> Result<Threshold, String> {
    let body = ThresholdPayload {
        agent_id,
        metric_name,
        warning_value,
        critical_value,
    };

    let response = Request::post(&format!("{BASE}/thresholds"))
        .header("Content-Type", "application/json")
        .json(&body)
        .map_err(|e| format!("Failed to encode create threshold payload: {e}"))?
        .send()
        .await
        .map_err(|e| format!("POST /thresholds failed: {e}"))?;

    if !response.ok() {
        return Err(format!("POST /thresholds failed: {}", response.status()));
    }

    response
        .json::<Threshold>()
        .await
        .map_err(|e| format!("Failed to decode created threshold: {e}"))
}

pub async fn update_threshold(
    id: i64,
    warning_value: f64,
    critical_value: f64,
) -> Result<Threshold, String> {
    let body = ThresholdUpdatePayload {
        warning_value,
        critical_value,
    };

    let response = Request::put(&format!("{BASE}/thresholds/{id}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .map_err(|e| format!("Failed to encode update threshold payload: {e}"))?
        .send()
        .await
        .map_err(|e| format!("PUT /thresholds/{id} failed: {e}"))?;

    if !response.ok() {
        return Err(format!(
            "PUT /thresholds/{id} failed: {}",
            response.status()
        ));
    }

    response
        .json::<Threshold>()
        .await
        .map_err(|e| format!("Failed to decode updated threshold: {e}"))
}

pub async fn delete_threshold(id: i64) -> Result<(), String> {
    let response = Request::delete(&format!("{BASE}/thresholds/{id}"))
        .send()
        .await
        .map_err(|e| format!("DELETE /thresholds/{id} failed: {e}"))?;

    if !response.ok() {
        return Err(format!(
            "DELETE /thresholds/{id} failed: {}",
            response.status()
        ));
    }

    Ok(())
}
