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
  /** The `linked_computers` row id on the account server — only set for
   * account-linked (p2p) computers. Used to sync this computer's
   * workspaces across devices; a computer without one is local-only. */
  serverId?: number;
}

export interface WorkspaceConfig {
  id: string;
  computerId: string;
  name: string;
  /** The server-side `workspaces` row id, once synced — see
   * `mergeServerWorkspaces`. Absent for workspaces under a computer with no
   * `serverId`, or not yet round-tripped through the server. */
  serverId?: number;
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
   * whatever the user is currently looking at. `serverId` is the
   * `linked_computers` row id, stored so this computer's workspaces can be
   * synced (see `mergeServerWorkspaces`). */
  addLinkedComputer: (name: string, peerId: string, relayUrl: string, serverId: number) => void;
  /** Additive-only merge of the account server's workspace list for one
   * computer into local state, keyed by name (matching the daemon's own
   * `"<computer>/<workspace>"` session-name identity) — adds workspaces
   * this browser hasn't seen yet, and backfills `serverId` onto local
   * workspaces that already match by name. Never removes or renames a
   * local workspace. */
  mergeServerWorkspaces: (computerId: string, serverWorkspaces: { id: number; name: string }[]) => void;
  /** Records the server-side workspace id for a workspace created locally,
   * once the best-effort `createWorkspace` push resolves — by exact local
   * `id`, not by name, so it's correct even if the workspace gets renamed
   * locally before that push resolves (a real race: `handleAddWorkspace`
   * immediately opens rename-on-create). Without this, a rename that beats
   * the create-push would never reach the server, and the next
   * `mergeServerWorkspaces` would re-add the old pre-rename name as a
   * spurious duplicate. */
  setWorkspaceServerId: (id: string, serverId: number) => void;
  /** True only for the very first render on a device that's never used the
   * app before (no persisted state at all yet) — the seeded "This Machine"
   * default at that point is a meaningless localhost placeholder, not a
   * real prior session worth protecting. Used to decide whether
   * account-discovered computers should also become the active one. */
  isFirstRun: boolean;
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
        wsUrl: import.meta.env.VITE_WS_URL || 'wss://localhost:54321',
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
  const [isFirstRun] = useState(() => localStorage.getItem(STORAGE_KEY) === null);
  const [state, setState] = useState<PersistedState>(loadState);

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  }, [state]);

  const value = useMemo<WorkspaceContextValue>(
    () => ({
      ...state,
      isFirstRun,

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
        const workspaceId = makeId();
        setState((s) => ({
          ...s,
          computers: [...s.computers, computer],
          workspaces: [...s.workspaces, { id: workspaceId, computerId: id, name: 'Default' }],
          activeComputerId: id,
          activeWorkspaceId: workspaceId,
        }));
        return id;
      },

      addLinkedComputer: (name, peerId, relayUrl, serverId) => {
        setState((s) => {
          const idx = s.computers.findIndex((c) => c.peerId === peerId);
          if (idx !== -1) {
            if (s.computers[idx].serverId === serverId) return s;
            const computers = s.computers.slice();
            computers[idx] = { ...computers[idx], serverId };
            return { ...s, computers };
          }
          const computer: ComputerConfig = {
            id: makeId(),
            name,
            mode: 'p2p',
            wsUrl: '',
            peerId,
            relayUrl,
            serverId,
          };
          return { ...s, computers: [...s.computers, computer] };
        });
      },

      mergeServerWorkspaces: (computerId, serverWorkspaces) => {
        setState((s) => {
          const byName = new Map(serverWorkspaces.map((w) => [w.name, w]));
          let backfilled = false;
          const workspaces = s.workspaces.map((w) => {
            if (w.computerId !== computerId) return w;
            const match = byName.get(w.name);
            if (match && w.serverId !== match.id) {
              backfilled = true;
              return { ...w, serverId: match.id };
            }
            return w;
          });

          const localNames = new Set(
            s.workspaces.filter((w) => w.computerId === computerId).map((w) => w.name)
          );
          const toAdd = serverWorkspaces.filter((w) => !localNames.has(w.name));

          if (!backfilled && toAdd.length === 0) return s;

          return {
            ...s,
            workspaces: [
              ...workspaces,
              ...toAdd.map((w) => ({ id: makeId(), computerId, name: w.name, serverId: w.id })),
            ],
          };
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

      setWorkspaceServerId: (id, serverId) => {
        setState((s) => ({
          ...s,
          workspaces: s.workspaces.map((w) => (w.id === id ? { ...w, serverId } : w)),
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

      setActiveComputerId: (id) =>
        setState((s) => {
          const existing = s.workspaces.find((w) => w.computerId === id);
          if (existing) {
            return { ...s, activeComputerId: id, activeWorkspaceId: existing.id };
          }
          const workspaceId = makeId();
          return {
            ...s,
            activeComputerId: id,
            activeWorkspaceId: workspaceId,
            workspaces: [...s.workspaces, { id: workspaceId, computerId: id, name: 'Default' }],
          };
        }),
      setActiveWorkspaceId: (id) => setState((s) => ({ ...s, activeWorkspaceId: id })),
    }),
    [state, isFirstRun]
  );

  return <WorkspaceContext.Provider value={value}>{children}</WorkspaceContext.Provider>;
}

export function useWorkspace(): WorkspaceContextValue {
  const ctx = useContext(WorkspaceContext);
  if (!ctx) throw new Error('useWorkspace must be used within a WorkspaceProvider');
  return ctx;
}
