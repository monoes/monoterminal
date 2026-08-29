import { createContext, useContext, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';

/**
 * 'direct': connect straight to `wsUrl` (same network, VPN, or tunnel).
 * 'p2p': connect via a WebRTC DataChannel negotiated through a signaling
 * relay — reaches the daemon from outside the local network without port
 * forwarding (see docs/decisions/011-p2p-networking-architecture.md).
 */
export type ComputerMode = 'direct' | 'p2p';

export interface ComputerConfig {
  id: string;
  name: string;
  mode: ComputerMode;
  /** Used when mode === 'direct' */
  wsUrl: string;
  /** Used when mode === 'p2p': the daemon's peer_id (Ed25519 pubkey fingerprint) */
  peerId?: string;
  /** Used when mode === 'p2p': the signaling relay URL */
  relayUrl?: string;
}

export interface WorkspaceConfig {
  id: string;
  computerId: string;
  name: string;
}

interface PersistedState {
  computers: ComputerConfig[];
  workspaces: WorkspaceConfig[];
  activeComputerId: string | null;
  activeWorkspaceId: string | null;
  /** Client-side display names for panes, keyed by workspace then pane id
   * (e.g. "pane-0") — panes are server-owned and their id is never
   * renamed, this is purely a local label layered on top. */
  paneNames: Record<string, Record<string, string>>;
}

/** Connection details for a new computer — direct URL or P2P peer/relay. */
export type NewComputerConnection =
  | { mode: 'direct'; wsUrl: string }
  | { mode: 'p2p'; peerId: string; relayUrl: string };

interface WorkspaceContextValue extends PersistedState {
  addComputer: (name: string, connection: NewComputerConnection) => string;
  /** Silently adds a P2P computer discovered via the account's linked
   * computers list — unlike `addComputer`, does not change
   * `activeComputerId`, so background discovery never steals focus from
   * whatever the user is currently looking at. */
  addLinkedComputer: (name: string, peerId: string, relayUrl: string) => void;
  removeComputer: (id: string) => void;
  renameComputer: (id: string, name: string) => void;
  addWorkspace: (computerId: string, name?: string) => string;
  removeWorkspace: (id: string) => void;
  renameWorkspace: (id: string, name: string) => void;
  renamePane: (workspaceId: string, paneId: string, name: string) => void;
  setActiveComputerId: (id: string) => void;
  setActiveWorkspaceId: (id: string) => void;
}

const STORAGE_KEY = 'monoterminal.workspaces.v1';

function makeId(): string {
  return Math.random().toString(36).slice(2) + Date.now().toString(36);
}

function defaultState(): PersistedState {
  const computerId = makeId();
  const workspaceId = makeId();
  return {
    computers: [
      {
        id: computerId,
        name: 'This Machine',
        mode: 'direct',
        wsUrl: import.meta.env.VITE_WS_URL || 'wss://localhost:5000',
      },
    ],
    workspaces: [{ id: workspaceId, computerId, name: 'Default' }],
    activeComputerId: computerId,
    activeWorkspaceId: workspaceId,
    paneNames: {},
  };
}

function loadState(): PersistedState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return defaultState();
    const parsed = JSON.parse(raw) as PersistedState;
    if (!parsed.computers?.length) return defaultState();
    // Migrate computers persisted before `mode` existed (they were always
    // direct-URL connections).
    parsed.computers = parsed.computers.map((c) => ({ mode: 'direct' as const, ...c }));
    if (!parsed.paneNames) parsed.paneNames = {};
    return parsed;
  } catch {
    return defaultState();
  }
}

const WorkspaceContext = createContext<WorkspaceContextValue | null>(null);

export function WorkspaceProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<PersistedState>(loadState);

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  }, [state]);

  const value = useMemo<WorkspaceContextValue>(
    () => ({
      ...state,

      addComputer: (name, connection) => {
        const id = makeId();
        const computer: ComputerConfig =
          connection.mode === 'direct'
            ? { id, name, mode: 'direct', wsUrl: connection.wsUrl }
            : {
                id,
                name,
                mode: 'p2p',
                wsUrl: '',
                peerId: connection.peerId,
                relayUrl: connection.relayUrl,
              };
        setState((s) => ({
          ...s,
          computers: [...s.computers, computer],
          activeComputerId: id,
        }));
        return id;
      },

      addLinkedComputer: (name, peerId, relayUrl) => {
        setState((s) => {
          if (s.computers.some((c) => c.peerId === peerId)) return s;
          const computer: ComputerConfig = {
            id: makeId(),
            name,
            mode: 'p2p',
            wsUrl: '',
            peerId,
            relayUrl,
          };
          return { ...s, computers: [...s.computers, computer] };
        });
      },

      removeComputer: (id) => {
        setState((s) => {
          const removedWorkspaceIds = s.workspaces.filter((w) => w.computerId === id).map((w) => w.id);
          const computers = s.computers.filter((c) => c.id !== id);
          const workspaces = s.workspaces.filter((w) => w.computerId !== id);
          const paneNames = { ...s.paneNames };
          for (const wid of removedWorkspaceIds) delete paneNames[wid];
          const activeComputerId = s.activeComputerId === id ? (computers[0]?.id ?? null) : s.activeComputerId;
          return { ...s, computers, workspaces, paneNames, activeComputerId };
        });
      },

      renameComputer: (id, name) => {
        setState((s) => ({
          ...s,
          computers: s.computers.map((c) => (c.id === id ? { ...c, name } : c)),
        }));
      },

      addWorkspace: (computerId, name) => {
        const id = makeId();
        setState((s) => {
          const count = s.workspaces.filter((w) => w.computerId === computerId).length;
          return {
            ...s,
            workspaces: [
              ...s.workspaces,
              { id, computerId, name: name || `Workspace ${count + 1}` },
            ],
            activeWorkspaceId: id,
          };
        });
        return id;
      },

      removeWorkspace: (id) => {
        setState((s) => {
          const workspaces = s.workspaces.filter((w) => w.id !== id);
          const paneNames = { ...s.paneNames };
          delete paneNames[id];
          const activeWorkspaceId = s.activeWorkspaceId === id ? (workspaces[0]?.id ?? null) : s.activeWorkspaceId;
          return { ...s, workspaces, paneNames, activeWorkspaceId };
        });
      },

      renameWorkspace: (id, name) => {
        setState((s) => ({
          ...s,
          workspaces: s.workspaces.map((w) => (w.id === id ? { ...w, name } : w)),
        }));
      },

      renamePane: (workspaceId, paneId, name) => {
        setState((s) => ({
          ...s,
          paneNames: {
            ...s.paneNames,
            [workspaceId]: { ...s.paneNames[workspaceId], [paneId]: name },
          },
        }));
      },

      setActiveComputerId: (id) => setState((s) => ({ ...s, activeComputerId: id })),
      setActiveWorkspaceId: (id) => setState((s) => ({ ...s, activeWorkspaceId: id })),
    }),
    [state]
  );

  return <WorkspaceContext.Provider value={value}>{children}</WorkspaceContext.Provider>;
}

export function useWorkspace(): WorkspaceContextValue {
  const ctx = useContext(WorkspaceContext);
  if (!ctx) throw new Error('useWorkspace must be used within a WorkspaceProvider');
  return ctx;
}
