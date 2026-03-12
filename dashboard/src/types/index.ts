// ── Status ────────────────────────────────────────────────────────────────────

export type AgentStatus = 'online' | 'warning' | 'critical' | 'offline'

// ── Metric sub-types ──────────────────────────────────────────────────────────

export interface DiskInfo {
  mount_point: string
  used_bytes: number
  total_bytes: number
  percent: number
}

export interface NetworkInfo {
  bytes_in: number
  bytes_out: number
}

export interface MemoryInfo {
  used_bytes: number
  total_bytes: number
  percent: number
}

// ── Metric snapshot (a single point-in-time reading) ──────────────────────────

export interface MetricSnapshot {
  timestamp: string
  cpu_percent: number
  memory: MemoryInfo
  disks: DiskInfo[]
  network: NetworkInfo
  uptime_seconds: number
}

// ── Agent summary (returned by GET /api/v1/agents) ───────────────────────────

export interface AgentSummary {
  agent_id: string
  status: AgentStatus
  last_seen_at: string
  duplicate_flag: boolean
  snapshot: MetricSnapshot | null
}

// ── Threshold configuration ───────────────────────────────────────────────────

export interface Threshold {
  id: number
  agent_id: string | null
  metric_name: 'cpu' | 'memory' | 'disk'
  warning_value: number
  critical_value: number
}

// ── WebSocket push event ──────────────────────────────────────────────────────

export interface MetricUpdateEvent {
  event: 'metric_update'
  agent_id: string
  timestamp: string
  status: AgentStatus
  cpu_percent: number
  memory: MemoryInfo
  disks: DiskInfo[]
  network: NetworkInfo
  uptime_seconds: number
  duplicate_flag: boolean
}
