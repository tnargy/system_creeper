import { useState } from 'react'
import { useWebSocket } from './hooks/useWebSocket'
import { useAgents, useThresholds } from './hooks/useAgents'
import { Header } from './components/Header'
import { DisconnectedBanner } from './components/DisconnectedBanner'
import { AgentGrid } from './components/AgentGrid'
import { AgentDetail } from './components/AgentDetail'
import type { AgentSummary } from './types'

export default function App() {
  const { status, lastEvent } = useWebSocket()
  const agentsQuery = useAgents(lastEvent)
  const thresholdsQuery = useThresholds()

  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null)

  const agents: AgentSummary[] = agentsQuery.data ?? []
  const thresholds = thresholdsQuery.data ?? []
  const connected = status === 'connected'
  const disconnected = status === 'disconnected'

  const selectedAgent = selectedAgentId
    ? agents.find((a) => a.agent_id === selectedAgentId) ?? null
    : null

  return (
    <div className="min-h-screen bg-gray-50 flex flex-col">
      <Header status={status} />
      {disconnected && <DisconnectedBanner />}

      <main className="flex-1 overflow-auto">
        {agentsQuery.isLoading && (
          <div className="flex items-center justify-center h-64 text-gray-400 text-sm">
            Loading agents…
          </div>
        )}
        {agentsQuery.isError && (
          <div className="flex items-center justify-center h-64 text-red-500 text-sm">
            Failed to load agents. Is the collector running?
          </div>
        )}
        {!agentsQuery.isLoading && !agentsQuery.isError && (
          <>
            {selectedAgent ? (
              <AgentDetail
                agent={selectedAgent}
                thresholds={thresholds}
                onBack={() => setSelectedAgentId(null)}
              />
            ) : (
              <AgentGrid
                agents={agents}
                connected={connected}
                onSelectAgent={setSelectedAgentId}
              />
            )}
          </>
        )}
      </main>
    </div>
  )
}
