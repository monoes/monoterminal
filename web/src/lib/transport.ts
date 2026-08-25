/**
 * Shared transport abstraction for a workspace's connection — lets
 * WorkspaceSession use either WebSocketClient (direct) or WebRtcClient
 * (P2P) interchangeably based on the computer's configured mode. Every
 * workspace's root session is now a Phase 4 (Splits/Tabs) layout, even one
 * with a single pane, so every workspace needs the pane-aware surface
 * (splitPane/closePane/focusPane, pane-tagged sendInput/resize).
 */

import { WebSocketClient, ConnectionState } from './websocket-client';
import { WebRtcClient } from './webrtc-client';
import type { MessageHandler, SplitDirection } from './protocol';
import type { ComputerConfig } from '../state/WorkspaceContext';

export interface TerminalTransport {
  connect(): void;
  disconnect(): void;
  attach(sessionId: string, rows: number, cols: number, sessionName?: string): void;
  sendInput(data: string | Uint8Array, paneId?: string): void;
  resize(rows: number, cols: number, paneId?: string): void;
  splitPane(paneId: string, direction: SplitDirection, newSessionShell?: string): void;
  closePane(paneId: string): void;
  focusPane(paneId: string): void;
  setHandlers(handlers: MessageHandler): void;
  onStateChange(listener: (state: ConnectionState) => void): () => void;
  getState(): ConnectionState;
}

export function createTransport(computer: ComputerConfig): TerminalTransport {
  if (computer.mode === 'p2p') {
    return new WebRtcClient({
      relayUrl: computer.relayUrl || '',
      peerId: computer.peerId || '',
    });
  }
  return new WebSocketClient({
    url: computer.wsUrl,
    autoReconnect: true,
    reconnectInterval: 3000,
    maxReconnectAttempts: 5,
  });
}
