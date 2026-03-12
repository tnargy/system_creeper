import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  ReferenceLine,
} from 'recharts'
import type { MetricSnapshot } from '../types'
import { formatBytes } from '../utils/format'

export type ChartMetric = 'cpu' | 'memory' | 'network_in' | 'network_out'

interface DataPoint {
  time: number   // unix ms
  value: number
  value2?: number  // used for network dual-line
}

interface Props {
  snapshots: MetricSnapshot[]
  metric: ChartMetric
  warningThreshold?: number
  criticalThreshold?: number
  /** Y axis domain max. Defaults to 100 for percent metrics */
  yMax?: number
  /** Formatter for Y axis ticks and tooltip */
  yFormat?: (v: number) => string
}

function toDataPoints(snapshots: MetricSnapshot[], metric: ChartMetric): DataPoint[] {
  return snapshots.map((s) => {
    const t = new Date(s.timestamp).getTime()
    if (metric === 'cpu')        return { time: t, value: s.cpu_percent }
    if (metric === 'memory')     return { time: t, value: s.memory.percent }
    if (metric === 'network_in') return { time: t, value: s.network.bytes_in }
    // network_out
    return { time: t, value: s.network.bytes_out }
  })
}

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

const pctFmt = (v: number) => `${v.toFixed(0)}%`

export function MetricChart({ snapshots, metric, warningThreshold, criticalThreshold, yMax = 100, yFormat }: Props) {
  const data = toDataPoints(snapshots, metric)

  const isNetwork = metric === 'network_in' || metric === 'network_out'
  const tickFmt = yFormat ?? (isNetwork ? (v: number) => formatBytes(v) : pctFmt)

  return (
    <ResponsiveContainer width="100%" height={160}>
      <LineChart data={data} margin={{ top: 4, right: 4, bottom: 0, left: 4 }}>
        <CartesianGrid strokeDasharray="3 3" stroke="#f0f0f0" />
        <XAxis
          dataKey="time"
          type="number"
          domain={['dataMin', 'dataMax']}
          scale="time"
          tickFormatter={formatTime}
          tick={{ fontSize: 10, fill: '#9ca3af' }}
          tickLine={false}
          axisLine={false}
          minTickGap={40}
        />
        <YAxis
          domain={isNetwork ? [0, 'auto'] : [0, yMax]}
          tickFormatter={tickFmt}
          tick={{ fontSize: 10, fill: '#9ca3af' }}
          tickLine={false}
          axisLine={false}
          width={46}
        />
        <Tooltip
          labelFormatter={(v) => formatTime(v as number)}
          formatter={(v) => [tickFmt(v as number)]}
          contentStyle={{ fontSize: 12, border: '1px solid #e5e7eb', borderRadius: 6 }}
        />
        {warningThreshold != null && warningThreshold > 0 && (
          <ReferenceLine y={warningThreshold} stroke="#f59e0b" strokeDasharray="4 3" strokeWidth={1.5} />
        )}
        {criticalThreshold != null && criticalThreshold > 0 && (
          <ReferenceLine y={criticalThreshold} stroke="#ef4444" strokeDasharray="4 3" strokeWidth={1.5} />
        )}
        <Line
          type="monotone"
          dataKey="value"
          stroke="#3b82f6"
          strokeWidth={2}
          dot={false}
          isAnimationActive={false}
        />
      </LineChart>
    </ResponsiveContainer>
  )
}

// ── Dual-line network chart ───────────────────────────────────────────────────

interface NetworkChartProps {
  snapshots: MetricSnapshot[]
}

export function NetworkChart({ snapshots }: NetworkChartProps) {
  const data = snapshots.map((s) => ({
    time: new Date(s.timestamp).getTime(),
    bytes_in: s.network.bytes_in,
    bytes_out: s.network.bytes_out,
  }))

  return (
    <ResponsiveContainer width="100%" height={160}>
      <LineChart data={data} margin={{ top: 4, right: 4, bottom: 0, left: 4 }}>
        <CartesianGrid strokeDasharray="3 3" stroke="#f0f0f0" />
        <XAxis
          dataKey="time"
          type="number"
          domain={['dataMin', 'dataMax']}
          scale="time"
          tickFormatter={(v) => new Date(v as number).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
          tick={{ fontSize: 10, fill: '#9ca3af' }}
          tickLine={false}
          axisLine={false}
          minTickGap={40}
        />
        <YAxis
          domain={[0, 'auto']}
          tickFormatter={(v) => formatBytes(v as number)}
          tick={{ fontSize: 10, fill: '#9ca3af' }}
          tickLine={false}
          axisLine={false}
          width={56}
        />
        <Tooltip
          labelFormatter={(v) => new Date(v as number).toLocaleTimeString()}
          formatter={(v, name) => [formatBytes(v as number), name === 'bytes_in' ? 'In' : 'Out']}
          contentStyle={{ fontSize: 12, border: '1px solid #e5e7eb', borderRadius: 6 }}
        />
        <Line type="monotone" dataKey="bytes_in"  name="bytes_in"  stroke="#3b82f6" strokeWidth={2} dot={false} isAnimationActive={false} />
        <Line type="monotone" dataKey="bytes_out" name="bytes_out" stroke="#8b5cf6" strokeWidth={2} dot={false} isAnimationActive={false} />
      </LineChart>
    </ResponsiveContainer>
  )
}
