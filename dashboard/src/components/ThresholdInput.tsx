import { useState, useRef, useCallback } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { updateThreshold, createThreshold, type ThresholdPayload } from '../api/client'

type SaveState = 'idle' | 'saving' | 'success' | 'error'

interface Props {
  label: string
  value: number
  thresholdId: number | null
  agentId: string
  metricName: 'cpu' | 'memory' | 'disk'
  isWarning: boolean   // true = editing warning_value, false = critical_value
  pairedValue: number  // the other threshold value (for UPSERT)
}

export function ThresholdInput({
  label,
  value,
  thresholdId,
  agentId,
  metricName,
  isWarning,
  pairedValue,
}: Props) {
  const [local, setLocal] = useState(String(value))
  const [saveState, setSaveState] = useState<SaveState>('idle')
  const confirmedRef = useRef(String(value))
  const qc = useQueryClient()

  // Update local value when external value changes (e.g., WS update)
  if (saveState === 'idle' && String(value) !== confirmedRef.current) {
    confirmedRef.current = String(value)
    setLocal(String(value))
  }

  const save = useCallback(async () => {
    const parsed = parseFloat(local)
    if (isNaN(parsed) || parsed < 0) {
      setLocal(confirmedRef.current)
      return
    }
    if (parsed === parseFloat(confirmedRef.current)) return

    setSaveState('saving')
    const warning = isWarning ? parsed : pairedValue
    const critical = isWarning ? pairedValue : parsed

    try {
      if (thresholdId !== null) {
        await updateThreshold(thresholdId, warning, critical)
      } else {
        const payload: ThresholdPayload = {
          agent_id: agentId || null,
          metric_name: metricName,
          warning_value: warning,
          critical_value: critical,
        }
        await createThreshold(payload)
      }
      confirmedRef.current = String(parsed)
      setSaveState('success')
      await qc.invalidateQueries({ queryKey: ['thresholds'] })
      setTimeout(() => setSaveState('idle'), 600)
    } catch {
      setSaveState('error')
      setLocal(confirmedRef.current)
      setTimeout(() => setSaveState('idle'), 1500)
    }
  }, [local, isWarning, pairedValue, thresholdId, agentId, metricName, qc])

  const ringCls =
    saveState === 'success'
      ? 'ring-2 ring-green-400'
      : saveState === 'error'
        ? 'ring-2 ring-red-400'
        : saveState === 'saving'
          ? 'opacity-50'
          : ''

  return (
    <div className="flex items-center gap-1.5">
      <span className="text-xs text-gray-500 w-8">{label}:</span>
      <input
        type="number"
        min={0}
        max={100}
        value={local}
        onChange={(e) => setLocal(e.target.value)}
        onBlur={save}
        onKeyDown={(e) => { if (e.key === 'Enter') { e.currentTarget.blur() } }}
        disabled={saveState === 'saving'}
        className={[
          'w-16 px-1.5 py-0.5 text-sm font-mono border border-gray-300 rounded',
          'focus:outline-none focus:ring-2 focus:ring-blue-400',
          'transition-all duration-200',
          ringCls,
        ].join(' ')}
      />
      <span className="text-xs text-gray-400">%</span>
    </div>
  )
}
