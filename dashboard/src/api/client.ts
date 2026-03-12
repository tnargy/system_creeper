import type { AgentSummary, MetricSnapshot, Threshold } from '../types'

const BASE = '/api/v1'

async function get<T>(path: string): Promise<T> {
  const res = await fetch(`${BASE}${path}`)
  if (!res.ok) throw new Error(`GET ${path} failed: ${res.status}`)
  return res.json() as Promise<T>
}

async function post<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  if (!res.ok) throw new Error(`POST ${path} failed: ${res.status}`)
  return res.json() as Promise<T>
}

async function put<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  if (!res.ok) throw new Error(`PUT ${path} failed: ${res.status}`)
  return res.json() as Promise<T>
}

async function del(path: string): Promise<void> {
  const res = await fetch(`${BASE}${path}`, { method: 'DELETE' })
  if (!res.ok) throw new Error(`DELETE ${path} failed: ${res.status}`)
}

// ── Agents ────────────────────────────────────────────────────────────────────

export const fetchAgents = (): Promise<AgentSummary[]> =>
  get('/agents')

export const fetchSnapshot = (agentId: string): Promise<MetricSnapshot> =>
  get(`/agents/${encodeURIComponent(agentId)}/snapshot`)

export const fetchHistory = (
  agentId: string,
  range: '1h' | '6h' | '24h' | '7d',
): Promise<MetricSnapshot[]> =>
  get(`/agents/${encodeURIComponent(agentId)}/history?range=${range}`)

// ── Thresholds ────────────────────────────────────────────────────────────────

export const fetchThresholds = (): Promise<Threshold[]> =>
  get('/thresholds')

export interface ThresholdPayload {
  agent_id?: string | null
  metric_name: 'cpu' | 'memory' | 'disk'
  warning_value: number
  critical_value: number
}

export const createThreshold = (payload: ThresholdPayload): Promise<Threshold> =>
  post('/thresholds', payload)

export const updateThreshold = (
  id: number,
  warning_value: number,
  critical_value: number,
): Promise<Threshold> =>
  put(`/thresholds/${id}`, { warning_value, critical_value })

export const deleteThreshold = (id: number): Promise<void> =>
  del(`/thresholds/${id}`)

// ── WebSocket URL ─────────────────────────────────────────────────────────────

export function getWsUrl(): string {
  const proto = window.location.protocol === 'https:' ? 'wss' : 'ws'
  return `${proto}://${window.location.host}/ws`
}
