import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { ArrowLeft } from 'lucide-react'
import type { AgentSummary, MetricSnapshot, Threshold } from '../types'
import { fetchHistory } from '../api/client'
import { StatusBadge } from './StatusBadge'
import { CpuPanel, MemoryPanel, DiskPanel, NetworkPanel } from './MetricPanel'
import { formatLastSeen, formatUptime } from '../utils/format'

type TimeRange = '1h' | '6h' | '24h' | '7d'
const TIME_RANGES: TimeRange[] = ['1h', '6h', '24h', '7d']

interface Props {
  agent: AgentSummary
  thresholds: Threshold[]
  onBack: () => void
}

export function AgentDetail({ agent, thresholds, onBack }: Props) {
  const [range, setRange] = useState<TimeRange>('1h')

  const historyQuery = useQuery<MetricSnapshot[]>({
    queryKey: ['history', agent.agent_id, range],
    queryFn: () => fetchHistory(agent.agent_id, range),
    refetchInterval: 30_000,
    refetchOnWindowFocus: false,
  })

  const history = historyQuery.data ?? []
  const snap = agent.snapshot

  return (
    <div className="p-6 flex flex-col gap-4">
      {/* Header row */}
      <div className="flex flex-wrap items-center gap-3">
        <button
          onClick={onBack}
          className="inline-flex items-center gap-1 text-sm text-gray-500 hover:text-gray-800 transition-colors"
        >
          <ArrowLeft className="w-4 h-4" />
          All Agents
        </button>
        <div className="flex items-center gap-2">
          <span className="text-base font-semibold text-gray-900">{agent.agent_id}</span>
          <StatusBadge status={agent.status} />
          {agent.snapshot && (
            <span className="text-xs text-gray-400">
              Last seen: {formatLastSeen(agent.last_seen_at)}
            </span>
          )}
        </div>
        {/* Time range pills */}
        <div className="ml-auto flex items-center gap-1">
          <span className="text-xs text-gray-400 mr-1">Time range:</span>
          {TIME_RANGES.map((r) => (
            <button
              key={r}
              onClick={() => setRange(r)}
              className={[
                'px-3 py-1 rounded-full text-xs font-medium border transition-colors',
                range === r
                  ? 'bg-gray-800 text-white border-gray-800'
                  : 'bg-white text-gray-600 border-gray-300 hover:border-gray-500',
              ].join(' ')}
            >
              {r}
            </button>
          ))}
        </div>
      </div>

      {/* Loading state */}
      {historyQuery.isLoading && (
        <div className="text-center py-8 text-gray-400 text-sm">Loading history…</div>
      )}

      {/* No snapshot */}
      {!snap && (
        <div className="text-center py-8 text-gray-400 text-sm">No data available for this agent.</div>
      )}

      {/* Panels */}
      {snap && (
        <>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <CpuPanel
              current={snap.cpu_percent}
              history={history}
              thresholds={thresholds}
              agentId={agent.agent_id}
            />
            <MemoryPanel
              snapshot={snap}
              history={history}
              thresholds={thresholds}
              agentId={agent.agent_id}
            />
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <DiskPanel
              snapshot={snap}
              history={history}
              thresholds={thresholds}
              agentId={agent.agent_id}
            />
            <NetworkPanel
              snapshot={snap}
              history={history}
            />
          </div>
          {/* Uptime footer */}
          <div className="text-xs text-gray-400">
            Uptime: {formatUptime(snap.uptime_seconds)}
          </div>
        </>
      )}
    </div>
  )
}
