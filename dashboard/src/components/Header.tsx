import { WifiOff, Circle } from 'lucide-react'
import type { WsStatus } from '../hooks/useWebSocket'

interface Props {
  status: WsStatus
}

export function Header({ status }: Props) {
  return (
    <header className="flex items-center justify-between px-6 py-3 bg-white border-b border-gray-200 shadow-sm">
      <span className="text-xl font-semibold text-gray-900">System Creeper</span>
      <ConnectionBadge status={status} />
    </header>
  )
}

function ConnectionBadge({ status }: Props) {
  if (status === 'connected') {
    return (
      <span className="inline-flex items-center gap-1.5 text-sm text-green-700 font-medium">
        <Circle className="w-2.5 h-2.5 fill-current" />
        Connected
      </span>
    )
  }
  if (status === 'connecting') {
    return (
      <span className="inline-flex items-center gap-1.5 text-sm text-amber-600 font-medium">
        <span className="w-2.5 h-2.5 rounded-full border-2 border-amber-500 border-t-transparent animate-spin" />
        Connecting…
      </span>
    )
  }
  return (
    <span className="inline-flex items-center gap-1.5 text-sm text-red-700 font-medium bg-red-50 px-2 py-0.5 rounded">
      <WifiOff className="w-3.5 h-3.5" />
      DISCONNECTED — reconnecting…
    </span>
  )
}
