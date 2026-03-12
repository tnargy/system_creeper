import { useState, useMemo } from 'react'
import { Search } from 'lucide-react'
import type { AgentSummary, AgentStatus } from '../types'
import { AgentCard } from './AgentCard'

type FilterOption = 'all' | AgentStatus

interface Props {
  agents: AgentSummary[]
  connected: boolean
  onSelectAgent: (agentId: string) => void
}

export function AgentGrid({ agents, connected, onSelectAgent }: Props) {
  const [filter, setFilter] = useState<FilterOption>('all')
  const [search, setSearch] = useState('')

  const filtered = useMemo(() => {
    return agents.filter((a) => {
      if (filter !== 'all' && a.status !== filter) return false
      if (search && !a.agent_id.toLowerCase().includes(search.toLowerCase())) return false
      return true
    })
  }, [agents, filter, search])

  const counts = useMemo(() => {
    const c = { online: 0, warning: 0, critical: 0, offline: 0 }
    for (const a of agents) c[a.status]++
    return c
  }, [agents])

  const filterOptions: Array<{ value: FilterOption; label: string; dot?: string }> = [
    { value: 'all',      label: `All (${agents.length})` },
    { value: 'online',   label: `Online (${counts.online})`,   dot: 'bg-green-500' },
    { value: 'warning',  label: `Warning (${counts.warning})`, dot: 'bg-amber-500' },
    { value: 'critical', label: `Critical (${counts.critical})`, dot: 'bg-red-500' },
    { value: 'offline',  label: `Offline (${counts.offline})`, dot: 'bg-gray-400' },
  ]

  return (
    <div className="p-6 flex flex-col gap-4">
      {/* Toolbar */}
      <div className="flex flex-wrap items-center gap-3">
        <span className="text-sm font-semibold text-gray-700 uppercase tracking-wider">
          Agents ({agents.length})
        </span>
        <div className="flex items-center gap-1 flex-wrap ml-2">
          {filterOptions.map((opt) => (
            <button
              key={opt.value}
              onClick={() => setFilter(opt.value)}
              className={[
                'inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-medium border transition-colors',
                filter === opt.value
                  ? 'bg-gray-800 text-white border-gray-800'
                  : 'bg-white text-gray-600 border-gray-300 hover:border-gray-400',
              ].join(' ')}
            >
              {opt.dot && (
                <span className={`w-1.5 h-1.5 rounded-full ${opt.dot}`} />
              )}
              {opt.label}
            </button>
          ))}
        </div>
        <div className="relative flex-1 min-w-48 max-w-xs ml-auto">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-400" />
          <input
            type="text"
            placeholder="Search agents…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="w-full pl-8 pr-3 py-1.5 text-sm border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>
      </div>

      {/* Grid */}
      {filtered.length === 0 ? (
        <div className="text-center py-16 text-gray-400 text-sm">
          {agents.length === 0 ? 'No agents reporting yet.' : 'No agents match your filter.'}
        </div>
      ) : (
        <div className="grid gap-4 grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
          {filtered.map((agent) => (
            <AgentCard
              key={agent.agent_id}
              agent={agent}
              connected={connected}
              onClick={() => onSelectAgent(agent.agent_id)}
            />
          ))}
        </div>
      )}
    </div>
  )
}
