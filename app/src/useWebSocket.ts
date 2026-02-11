import { useEffect, useRef, useState, useCallback } from 'react';
import type { NetworkStats, PulseBlock } from './api';

/** WebSocket event types matching the Rust WsEvent enum */
export type WsEvent =
  | { type: 'new_block'; block: PulseBlock }
  | { type: 'stats'; stats: NetworkStats }
  | { type: 'heartbeat_count'; count: number };

type WsStatus = 'connecting' | 'connected' | 'disconnected';

interface UseWebSocketOptions {
  /** HTTP(S) node URL — automatically converted to ws(s):// */
  nodeUrl: string;
  /** Called on each event */
  onEvent?: (event: WsEvent) => void;
  /** Auto-reconnect (default: true) */
  reconnect?: boolean;
  /** Reconnect delay in ms (default: 3000) */
  reconnectDelay?: number;
}

export function useWebSocket({
  nodeUrl,
  onEvent,
  reconnect = true,
  reconnectDelay = 3000,
}: UseWebSocketOptions) {
  const [status, setStatus] = useState<WsStatus>('disconnected');
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;

  const connect = useCallback(() => {
    // Convert http(s) to ws(s)
    const wsUrl = nodeUrl
      .replace(/^https:\/\//, 'wss://')
      .replace(/^http:\/\//, 'ws://')
      .replace(/\/$/, '') + '/ws';

    setStatus('connecting');
    const ws = new WebSocket(wsUrl);
    wsRef.current = ws;

    ws.onopen = () => {
      setStatus('connected');
      console.log('[Pulse WS] Connected to', wsUrl);
    };

    ws.onmessage = (evt) => {
      try {
        const event = JSON.parse(evt.data) as WsEvent;
        onEventRef.current?.(event);
      } catch (e) {
        console.warn('[Pulse WS] Failed to parse message:', evt.data);
      }
    };

    ws.onclose = () => {
      setStatus('disconnected');
      console.log('[Pulse WS] Disconnected');
      wsRef.current = null;

      if (reconnect) {
        reconnectTimer.current = setTimeout(connect, reconnectDelay);
      }
    };

    ws.onerror = (err) => {
      console.warn('[Pulse WS] Error:', err);
      ws.close();
    };
  }, [nodeUrl, reconnect, reconnectDelay]);

  useEffect(() => {
    connect();
    return () => {
      if (reconnectTimer.current) clearTimeout(reconnectTimer.current);
      wsRef.current?.close();
    };
  }, [connect]);

  return { status };
}
