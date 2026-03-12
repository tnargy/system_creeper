import { TriangleAlert, Circle, XCircle, Minus } from 'lucide-react'
import type { AgentSummary, AgentStatus } from '../types'
import { formatUptime, formatLastSeen } from '../utils/format'

interface Props {
  agent: AgentSummary
  connected: boolean
  onClick: () => void
}

const borderColor: Record<AgentStatus, string> = {
  online:   'border-l-green-500',
  warning:  'border-l-amber-500',
  critical: 'border-l-red-500',
  offline:  'border-l-gray-400',
}

const StatusIcon = ({ status }: { status: AgentStatus }) => {
  if (status === 'online')   return <Circle className="w-3 h-3 text-green-500 fill-current flex-shrink-0" />
  if (status === 'warning')  return <TriangleAlert className="w-3 h-3 text-amber-500 flex-shrink-0" />
  if (status === 'critical') return <XCircle className="w-3 h-3 text-red-500 flex-shrink-0" />
  return <Minus className="w-3 h-3 text-gray-400 flex-shrink-0" />
}

function MetricRow({
  label,
  value,
  status,
}: {
  label: string
  value: string
  status?: 'warning' | 'critical' | null
}) {
  const valCls =
    status === 'critical'
      ? 'text-red-600 font-semibold'
      : status === 'warning'
        ? 'text-amber-600 font-semibold'
        : 'text-gray-800'

  return (
    <div className="flex justify-between items-baseline">
      <span className="text-xs text-gray-500 uppercase tracking-wide">{label}</span>
      <span className={`text-sm font-medium ${valCls}`}>
        {value}
        {status === 'warning' && (
          <TriangleAlert className="inline w-3 h-3 ml-1 text-amber-500" />
        )}
        {status === 'critical' && (
          <XCircle className="inline w-3 h-3 ml-1 text-red-500" />
        )}
      </span>
    </div>
  )
}

export function AgentCard({ agent, connected, onClick }: Props) {
  const isOffline = agent.status === 'offline'
  const snap = agent.snapshot
  const clickable = connected && !isOffline

  // Simple inline threshold check — cards use status derived from the collector
  // The per-metric icon is driven by the status field from the WS event
  const cpuStatus =
    agent.status === 'critical'
      ? ('critical' as const)
      : agent.status === 'warning'
        ? ('warning' as const)
        : null

  return (
    <button
      onClick={clickable ? onClick : undefined}
      disabled={!clickable}
      className={[
        'text-left w-full bg-white rounded-lg shadow-sm border border-gray-200',
        'border-l-4 p-4 flex flex-col gap-2',
        borderColor[agent.status],
        clickable ? 'hover:shadow-md hover:border-gray-300 cursor-pointer transition-shadow duration-200' : 'cursor-default',
      ].join(' ')}
    >
      {/* Header row */}
      <div className="flex items-center gap-1.5 min-w-0">
        <StatusIcon status={agent.status} />
        <span
          className={[
            'text-sm font-semibold truncate',
            agent.duplicate_flag ? 'text-red-500' : 'text-gray-900',
          ].join(' ')}
          title={agent.duplicate_flag ? 'Duplicate agent ID detected' : undefined}
        >
          {agent.duplicate_flag && (
            <TriangleAlert className="inline w-3 h-3 mr-0.5 text-red-500" />
          )}
          {agent.agent_id}
        </span>
      </div>

      {/* Metric rows */}
      {isOffline || !snap || !connected ? (
        <div className="flex-1 flex flex-col items-center justify-center py-3 gap-1">
          <span className="text-lg font-bold text-gray-300 tracking-widest uppercase">
            {isOffline ? 'Offline' : '—'}
          </span>
          {agent.last_seen_at && (
            <span className="text-xs text-gray-400">
              Last: {formatLastSeen(agent.last_seen_at)}
            </span>
          )}
        </div>
      ) : (
        <>
          <div className="flex flex-col gap-1">
            <MetricRow
              label="CPU"
              value={`${snap.cpu_percent.toFixed(1)}%`}
              status={cpuStatus}
            />
            <MetricRow
              label="MEM"
              value={`${snap.memory.percent.toFixed(1)}%`}
            />
            {snap.disks.length > 0 && (
              <MetricRow
                label="DISK"
                value={`${snap.disks[0].percent.toFixed(1)}%`}
              />
            )}
          </div>
          {/* Footer */}
          <div className="flex justify-between text-xs text-gray-400 pt-1 border-t border-gray-100">
            <span>Up: {formatUptime(snap.uptime_seconds)}</span>
            <span>{formatLastSeen(agent.last_seen_at)}</span>
          </div>
        </>
      )}
    </button>
  )
}
