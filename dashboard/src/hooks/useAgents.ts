import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect } from 'react'
import { fetchAgents, fetchThresholds } from '../api/client'
import type { AgentSummary, MetricUpdateEvent, Threshold } from '../types'

// ── Agents list ───────────────────────────────────────────────────────────────

export function useAgents(lastEvent: MetricUpdateEvent | null) {
  const qc = useQueryClient()

  const agentsQuery = useQuery<AgentSummary[]>({
    queryKey: ['agents'],
    queryFn: fetchAgents,
    refetchOnWindowFocus: false,
  })

  // Apply live WebSocket updates directly into the query cache
  useEffect(() => {
    if (!lastEvent) return
    qc.setQueryData<AgentSummary[]>(['agents'], (prev) => {
      if (!prev) return prev
      const exists = prev.some((a) => a.agent_id === lastEvent.agent_id)
      const updated: AgentSummary = {
        agent_id: lastEvent.agent_id,
        status: lastEvent.status,
        last_seen_at: lastEvent.timestamp,
        duplicate_flag: lastEvent.duplicate_flag,
        snapshot: {
          timestamp: lastEvent.timestamp,
          cpu_percent: lastEvent.cpu_percent,
          memory: lastEvent.memory,
          disks: lastEvent.disks,
          network: lastEvent.network,
          uptime_seconds: lastEvent.uptime_seconds,
        },
      }
      if (exists) {
        return prev.map((a) => (a.agent_id === lastEvent.agent_id ? updated : a))
      }
      return [...prev, updated]
    })
  }, [lastEvent, qc])

  return agentsQuery
}

// ── Thresholds ─────────────────────────────────────────────────────────────────

export function useThresholds() {
  return useQuery<Threshold[]>({
    queryKey: ['thresholds'],
    queryFn: fetchThresholds,
    refetchOnWindowFocus: false,
  })
}

// ── Resolved thresholds for a specific agent + metric ─────────────────────────

export function resolveThreshold(
  thresholds: Threshold[],
  agentId: string,
  metric: 'cpu' | 'memory' | 'disk',
): { warning: number; critical: number; id: number | null } {
  const agentSpecific = thresholds.find(
    (t) => t.agent_id === agentId && t.metric_name === metric,
  )
  if (agentSpecific) {
    return { warning: agentSpecific.warning_value, critical: agentSpecific.critical_value, id: agentSpecific.id }
  }
  const global = thresholds.find((t) => t.agent_id === null && t.metric_name === metric)
  if (global) {
    return { warning: global.warning_value, critical: global.critical_value, id: global.id }
  }
  return { warning: 0, critical: 0, id: null }
}
