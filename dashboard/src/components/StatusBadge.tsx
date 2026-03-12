import { Circle, TriangleAlert, XCircle, Minus } from 'lucide-react'
import type { AgentStatus } from '../types'

interface Props {
  status: AgentStatus
  className?: string
}

const config: Record<AgentStatus, { label: string; icon: React.ReactNode; cls: string }> = {
  online:   { label: 'Online',   icon: <Circle   className="w-3 h-3 fill-current" />, cls: 'bg-green-100  text-green-700'  },
  warning:  { label: 'Warning',  icon: <TriangleAlert className="w-3 h-3" />,          cls: 'bg-amber-100  text-amber-700'  },
  critical: { label: 'Critical', icon: <XCircle   className="w-3 h-3" />,              cls: 'bg-red-100    text-red-700'    },
  offline:  { label: 'Offline',  icon: <Minus     className="w-3 h-3" />,              cls: 'bg-gray-100   text-gray-600'   },
}

export function StatusBadge({ status, className = '' }: Props) {
  const { label, icon, cls } = config[status]
  return (
    <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${cls} ${className}`}>
      {icon}
      {label}
    </span>
  )
}
