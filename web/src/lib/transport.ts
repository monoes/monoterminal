/**
 * Shared transport abstraction for a workspace's connection — lets
 * WorkspaceSession use either WebSocketClient (direct) or WebRtcClient
 * (P2P) interchangeably based on the computer's configured mode. Every
 * workspace's root session is now a Phase 4 (Splits/Tabs) layout, even one
 * with a single pane, so every workspace needs the pane-aware surface
 * (splitPane/closePane/focusPane, pane-tagged sendInput/resize).
 */

import { ConnectionState } from './websocket-client';
import { HybridTransport } from './hybrid-transport';
import type { MessageHandler, SplitDirection } from './protocol';
import type { ComputerConfig } from '../state/WorkspaceContext';
import { resolveRoutes } from '../state/WorkspaceContext';
import { getLocalDaemon } from './local-daemon';

export interface TerminalTransport {
  connect(): void;
  disconnect(): void;
  attach(
    sessionId: string,
    rows: number,
    cols: number,
    sessionName?: string,
    previousSessionName?: string
  ): void;
  sendInput(data: string | Uint8Array, paneId?: string): void;
  resize(rows: number, cols: number, paneId?: string): void;
  splitPane(paneId: string, direction: SplitDirection, newSessionShell?: string): void;
  closePane(paneId: string): void;
  focusPane(paneId: string): void;
  setHandlers(handlers: MessageHandler): void;
  onStateChange(listener: (state: ConnectionState) => void): () => void;
  getState(): ConnectionState;
}

/** Takes a getter, not a `ComputerConfig` directly: the transport is built
 * once inside a `useState` initializer (see WorkspaceSession.tsx) and lives
 * for the whole session, but `mergeComputers`/`adoptPeerId` can update a
 * computer's `peerId`/`wsUrl` later (e.g. discovering the same daemon is
 * also reachable locally, well after a pane was already opened) by
 * producing a new `ComputerConfig` object — closing over the object itself
 * would freeze the routes this transport ever considers to whatever they
 * were at mount, silently defeating "prefer local over P2P" for any
 * already-open session. The getter is re-invoked on every `connect()`
 * attempt (same as `getLocalDaemon()` already was), so an identity update
 * is picked up on the next reconnect with no remount needed. */
export function createTransport(getComputer: () => ComputerConfig): TerminalTransport {
  return new HybridTransport(() => resolveRoutes(getComputer(), getLocalDaemon()));
}
