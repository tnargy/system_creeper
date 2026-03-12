import { useEffect, useRef, useState, useCallback } from 'react'
import { getWsUrl } from '../api/client'
import type { MetricUpdateEvent } from '../types'

export type WsStatus = 'connecting' | 'connected' | 'disconnected'

interface UseWebSocketReturn {
  status: WsStatus
  lastEvent: MetricUpdateEvent | null
}

const INITIAL_DELAY = 1_000
const MAX_DELAY = 30_000

export function useWebSocket(): UseWebSocketReturn {
  const [status, setStatus] = useState<WsStatus>('connecting')
  const [lastEvent, setLastEvent] = useState<MetricUpdateEvent | null>(null)
  const wsRef = useRef<WebSocket | null>(null)
  const delayRef = useRef(INITIAL_DELAY)
  const mountedRef = useRef(true)

  const connect = useCallback(() => {
    if (!mountedRef.current) return

    setStatus('connecting')
    const ws = new WebSocket(getWsUrl())
    wsRef.current = ws

    ws.onopen = () => {
      if (!mountedRef.current) { ws.close(); return }
      setStatus('connected')
      delayRef.current = INITIAL_DELAY
    }

    ws.onmessage = (ev: MessageEvent<string>) => {
      if (!mountedRef.current) return
      try {
        const data = JSON.parse(ev.data) as MetricUpdateEvent
        if (data.event === 'metric_update') setLastEvent(data)
      } catch {
        // ignore malformed frames
      }
    }

    ws.onclose = () => {
      if (!mountedRef.current) return
      setStatus('disconnected')
      const delay = delayRef.current
      delayRef.current = Math.min(delay * 2, MAX_DELAY)
      setTimeout(connect, delay)
    }

    ws.onerror = () => ws.close()
  }, [])

  useEffect(() => {
    mountedRef.current = true
    connect()
    return () => {
      mountedRef.current = false
      wsRef.current?.close()
    }
  }, [connect])

  return { status, lastEvent }
}
