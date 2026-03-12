import { WifiOff } from 'lucide-react'

export function DisconnectedBanner() {
  return (
    <div className="flex items-center gap-3 px-6 py-3 bg-amber-50 border-b border-amber-200 text-amber-800 text-sm">
      <WifiOff className="w-4 h-4 flex-shrink-0" />
      <span>
        <strong>Data unavailable.</strong> Connection to collector lost. Metric values are hidden
        until connection is restored.
      </span>
    </div>
  )
}
