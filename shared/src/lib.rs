use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Payload sent by an agent to the collector for each metric collection interval.
///
/// `agent_id` uniquely identifies the reporting machine; it is configured in the agent's
/// TOML file and falls back to the machine hostname when absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPayload {
    pub agent_id: String,
    pub timestamp: DateTime<Utc>,
    pub cpu_percent: f64,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    pub network: NetworkInfo,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub mount_point: String,
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub bytes_in: u64,
    pub bytes_out: u64,
}
