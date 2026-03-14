use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Online,
    Warning,
    Critical,
    Offline,
}

impl AgentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Warning => "warning",
            Self::Critical => "critical",
            Self::Offline => "offline",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct DiskInfo {
    pub mount_point: String,
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub percent: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct NetworkInfo {
    pub bytes_in: u64,
    pub bytes_out: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MemoryInfo {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub percent: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MetricSnapshot {
    pub timestamp: String,
    pub cpu_percent: f64,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    pub network: NetworkInfo,
    pub uptime_seconds: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentSummary {
    pub agent_id: String,
    pub status: AgentStatus,
    pub last_seen_at: String,
    pub duplicate_flag: bool,
    pub tags: Vec<String>,
    pub snapshot: Option<MetricSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Threshold {
    pub id: i64,
    pub agent_id: Option<String>,
    pub metric_name: String,
    pub warning_value: f64,
    pub critical_value: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MetricUpdateEvent {
    pub event: String,
    pub agent_id: String,
    pub timestamp: String,
    pub status: AgentStatus,
    pub cpu_percent: f64,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    pub network: NetworkInfo,
    pub uptime_seconds: u64,
    pub duplicate_flag: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct AgentApiResponse {
    pub agent_id: String,
    pub status: AgentStatus,
    pub last_seen_at: String,
    #[serde(default)]
    pub duplicate_flag: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, alias = "latest_metric")]
    pub snapshot: Option<SnapshotWire>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum SnapshotWire {
    Nested(MetricSnapshot),
    Raw(RawMetricSnapshot),
}

impl SnapshotWire {
    pub fn into_snapshot(self) -> MetricSnapshot {
        match self {
            Self::Nested(s) => s,
            Self::Raw(raw) => raw.into_snapshot(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct RawMetricSnapshot {
    pub timestamp: String,
    pub cpu_percent: f64,
    pub memory_used_bytes: i64,
    pub memory_total_bytes: i64,
    pub memory_percent: f64,
    pub disks: Vec<RawDiskInfo>,
    pub network_bytes_in: i64,
    pub network_bytes_out: i64,
    pub uptime_seconds: i64,
}

impl RawMetricSnapshot {
    pub fn into_snapshot(self) -> MetricSnapshot {
        MetricSnapshot {
            timestamp: self.timestamp,
            cpu_percent: self.cpu_percent,
            memory: MemoryInfo {
                used_bytes: self.memory_used_bytes.max(0) as u64,
                total_bytes: self.memory_total_bytes.max(0) as u64,
                percent: self.memory_percent,
            },
            disks: self
                .disks
                .into_iter()
                .map(|d| DiskInfo {
                    mount_point: d.mount_point,
                    used_bytes: d.used_bytes.max(0) as u64,
                    total_bytes: d.total_bytes.max(0) as u64,
                    percent: d.percent,
                })
                .collect(),
            network: NetworkInfo {
                bytes_in: self.network_bytes_in.max(0) as u64,
                bytes_out: self.network_bytes_out.max(0) as u64,
            },
            uptime_seconds: self.uptime_seconds.max(0) as u64,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct RawDiskInfo {
    pub mount_point: String,
    pub used_bytes: i64,
    pub total_bytes: i64,
    pub percent: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum HistorySnapshotWire {
    Nested(MetricSnapshot),
    Raw(RawMetricSnapshot),
}

impl HistorySnapshotWire {
    pub fn into_snapshot(self) -> MetricSnapshot {
        match self {
            Self::Nested(s) => s,
            Self::Raw(raw) => raw.into_snapshot(),
        }
    }
}
