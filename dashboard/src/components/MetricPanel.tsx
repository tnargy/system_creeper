import { XCircle, TriangleAlert } from 'lucide-react'
import type { MetricSnapshot, Threshold } from '../types'
import { MetricChart, NetworkChart } from './MetricChart'
import { ThresholdInput } from './ThresholdInput'
import { resolveThreshold } from '../hooks/useAgents'
import { formatBytes } from '../utils/format'

interface PanelProps {
  title: string
  children: React.ReactNode
}

function Panel({ title, children }: PanelProps) {
  return (
    <div className="bg-white rounded-lg border border-gray-200 shadow-sm p-4 flex flex-col gap-3">
      <h3 className="text-xs font-semibold uppercase tracking-wider text-gray-500">{title}</h3>
      {children}
    </div>
  )
}

// ── CPU Panel ─────────────────────────────────────────────────────────────────

interface CpuPanelProps {
  current: number
  history: MetricSnapshot[]
  thresholds: Threshold[]
  agentId: string
}

export function CpuPanel({ current, history, thresholds, agentId }: CpuPanelProps) {
  const t = resolveThreshold(thresholds, agentId, 'cpu')
  const alertStatus =
    t.critical > 0 && current >= t.critical ? 'critical'
    : t.warning > 0 && current >= t.warning ? 'warning'
    : null

  return (
    <Panel title="CPU Usage">
      <div className="flex items-baseline justify-between">
        <div className="flex items-center gap-2">
          <span className="text-2xl font-bold text-gray-900">{current.toFixed(1)}%</span>
          {alertStatus === 'critical' && <XCircle className="w-4 h-4 text-red-500" />}
          {alertStatus === 'warning'  && <TriangleAlert className="w-4 h-4 text-amber-500" />}
        </div>
        <div className="flex flex-col items-end gap-0.5">
          <ThresholdInput label="warn"  value={t.warning}  thresholdId={t.id} agentId={agentId} metricName="cpu" isWarning={true}  pairedValue={t.critical} />
          <ThresholdInput label="crit"  value={t.critical} thresholdId={t.id} agentId={agentId} metricName="cpu" isWarning={false} pairedValue={t.warning}  />
        </div>
      </div>
      <MetricChart snapshots={history} metric="cpu" warningThreshold={t.warning} criticalThreshold={t.critical} />
    </Panel>
  )
}

// ── Memory Panel ──────────────────────────────────────────────────────────────

interface MemoryPanelProps {
  snapshot: MetricSnapshot
  history: MetricSnapshot[]
  thresholds: Threshold[]
  agentId: string
}

export function MemoryPanel({ snapshot, history, thresholds, agentId }: MemoryPanelProps) {
  const t = resolveThreshold(thresholds, agentId, 'memory')
  const current = snapshot.memory.percent
  const alertStatus =
    t.critical > 0 && current >= t.critical ? 'critical'
    : t.warning > 0 && current >= t.warning ? 'warning'
    : null

  return (
    <Panel title="Memory Usage">
      <div className="flex items-baseline justify-between">
        <div className="flex items-center gap-2">
          <span className="text-2xl font-bold text-gray-900">{current.toFixed(1)}%</span>
          {alertStatus === 'critical' && <XCircle className="w-4 h-4 text-red-500" />}
          {alertStatus === 'warning'  && <TriangleAlert className="w-4 h-4 text-amber-500" />}
          <span className="text-xs text-gray-400">
            {formatBytes(snapshot.memory.used_bytes)} / {formatBytes(snapshot.memory.total_bytes)}
          </span>
        </div>
        <div className="flex flex-col items-end gap-0.5">
          <ThresholdInput label="warn" value={t.warning}  thresholdId={t.id} agentId={agentId} metricName="memory" isWarning={true}  pairedValue={t.critical} />
          <ThresholdInput label="crit" value={t.critical} thresholdId={t.id} agentId={agentId} metricName="memory" isWarning={false} pairedValue={t.warning}  />
        </div>
      </div>
      <MetricChart snapshots={history} metric="memory" warningThreshold={t.warning} criticalThreshold={t.critical} />
    </Panel>
  )
}

// ── Disk Panel ────────────────────────────────────────────────────────────────

interface DiskPanelProps {
  snapshot: MetricSnapshot
  history: MetricSnapshot[]
  thresholds: Threshold[]
  agentId: string
}

function DiskBar({ label, percent, status }: { label: string; percent: number; status: 'warning' | 'critical' | null }) {
  const barCls =
    status === 'critical' ? 'bg-red-500'
    : status === 'warning' ? 'bg-amber-500'
    : 'bg-blue-500'

  return (
    <div className="flex flex-col gap-0.5">
      <div className="flex justify-between text-xs">
        <span className="text-gray-600 font-mono truncate max-w-32">{label}</span>
        <span className={status ? (status === 'critical' ? 'text-red-600 font-semibold' : 'text-amber-600 font-semibold') : 'text-gray-700'}>
          {percent.toFixed(1)}%
          {status === 'warning'  && <TriangleAlert className="inline w-3 h-3 ml-0.5 text-amber-500" />}
          {status === 'critical' && <XCircle       className="inline w-3 h-3 ml-0.5 text-red-500"   />}
        </span>
      </div>
      <div className="h-1.5 bg-gray-100 rounded-full overflow-hidden">
        <div className={`h-full rounded-full transition-all ${barCls}`} style={{ width: `${Math.min(percent, 100)}%` }} />
      </div>
    </div>
  )
}

export function DiskPanel({ snapshot, history, thresholds, agentId }: DiskPanelProps) {
  const t = resolveThreshold(thresholds, agentId, 'disk')

  return (
    <Panel title="Disk Usage">
      <div className="flex flex-col gap-2">
        {snapshot.disks.map((disk) => {
          const s =
            t.critical > 0 && disk.percent >= t.critical ? 'critical' as const
            : t.warning > 0 && disk.percent >= t.warning ? 'warning' as const
            : null
          return <DiskBar key={disk.mount_point} label={disk.mount_point} percent={disk.percent} status={s} />
        })}
        <div className="flex justify-end gap-1 pt-1">
          <ThresholdInput label="warn" value={t.warning}  thresholdId={t.id} agentId={agentId} metricName="disk" isWarning={true}  pairedValue={t.critical} />
          <ThresholdInput label="crit" value={t.critical} thresholdId={t.id} agentId={agentId} metricName="disk" isWarning={false} pairedValue={t.warning}  />
        </div>
      </div>
      {history.length > 0 && (
        <MetricChart
          snapshots={history}
          metric="cpu"  // proxy: shows primary disk via first disk percent isn't directly in MetricSnapshot
          yMax={100}
        />
      )}
    </Panel>
  )
}

// ── Network Panel ─────────────────────────────────────────────────────────────

interface NetworkPanelProps {
  snapshot: MetricSnapshot
  history: MetricSnapshot[]
}

export function NetworkPanel({ snapshot, history }: NetworkPanelProps) {
  return (
    <Panel title="Network Throughput">
      <div className="flex gap-4 text-sm">
        <div>
          <span className="text-gray-500 text-xs">In: </span>
          <span className="font-semibold text-blue-600">{formatBytes(snapshot.network.bytes_in)}/s</span>
        </div>
        <div>
          <span className="text-gray-500 text-xs">Out: </span>
          <span className="font-semibold text-purple-600">{formatBytes(snapshot.network.bytes_out)}/s</span>
        </div>
      </div>
      <NetworkChart snapshots={history} />
    </Panel>
  )
}
