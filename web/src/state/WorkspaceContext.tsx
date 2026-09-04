import { createContext, useContext, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { getLocalDaemon } from '../lib/local-daemon';
import type { LocalDaemon } from '../lib/local-daemon';

/**
 * 'direct': connect straight to `wsUrl` (same network, VPN, or tunnel).
 * 'p2p': connect via a WebRTC DataChannel negotiated through a signaling
 * relay — reaches the daemon from outside the local network without port
 * forwarding (see docs/decisions/011-p2p-networking-architecture.md).
 *
 * @deprecated no longer used to decide how to connect — `resolveRoutes()`
 * does that from whichever of `wsUrl`/`peerId`+`relayUrl` are present, and a
 * computer can have both (a unified identity reachable either way). Kept on
 * `ComputerConfig` only so a rollback to an older build still reads
 * persisted state correctly.
 */
export type ComputerMode = 'direct' | 'p2p';

export interface ComputerConfig {
  id: string;
  name: string;
  /** @deprecated see the type's own doc comment. */
  mode?: ComputerMode;
  /** A directly-reachable URL for this daemon (hand-typed, a VPN/tunnel
   * address, or the seeded localhost default) — absent if this computer is
   * only known via P2P. May coexist with `peerId` once identity is unified
   * (see `mergeComputers`/`adoptPeerId`). */
  wsUrl?: string;
  /** This daemon's peer_id (Ed25519 pubkey fingerprint) — the canonical,
   * account-synced identity once present. May coexist with `wsUrl`. */
  peerId?: string;
  /** The signaling relay URL for P2P — present whenever `peerId` is. */
  relayUrl?: string;
  /** The `linked_computers` row id on the account server — only set for
   * account-linked computers. Used to sync this computer's name and
   * workspaces across devices; a computer without one is local-only. */
  serverId?: number;
}

/** A concrete way to reach a computer's daemon, in preference order — see
 * `resolveRoutes`. */
export type Route =
  | { kind: 'ws'; url: string }
  | { kind: 'p2p'; peerId: string; relayUrl: string };

/** Every way this computer can currently be reached, in preference order:
 * (1) a local daemon the browser has already probed and confirmed shares
 * this computer's peer_id — the fast path, skipping the relay entirely;
 * (2) a configured direct URL; (3) P2P via the relay. A computer with only
 * a `wsUrl` (no `peerId`, e.g. a logged-out user, or one never linked to an
 * account) always yields exactly the one route it did before this
 * identity-unification feature existed — the new behavior only appears
 * once a computer has both a `peerId` and a matching local probe. */
export function resolveRoutes(computer: ComputerConfig, local: LocalDaemon | null): Route[] {
  const routes: Route[] = [];
  if (local && computer.peerId && local.peerId === computer.peerId) {
    routes.push({ kind: 'ws', url: local.url });
  }
  if (computer.wsUrl && !routes.some((r) => r.kind === 'ws' && r.url === computer.wsUrl)) {
    routes.push({ kind: 'ws', url: computer.wsUrl });
  }
  if (computer.peerId && computer.relayUrl) {
    routes.push({ kind: 'p2p', peerId: computer.peerId, relayUrl: computer.relayUrl });
  }
  return routes;
}

/** Normalizes a `wss://`/`ws://` URL's host to compare `localhost`,
 * `127.0.0.1`, and `[::1]` as equivalent — used only to decide whether a
 * directly-configured URL is provably "this same machine" (see
 * `isSameHostPort`), never to compare two arbitrary URLs generally. */
function normalizeLocalHost(host: string): string {
  if (host === '127.0.0.1' || host === '[::1]' || host === '::1') return 'localhost';
  return host;
}

/** True only when both URLs resolve to the same host+port after localhost
 * normalization — deliberately strict. A LAN IP (e.g. `192.168.1.5`) is
 * never treated as equivalent to `localhost`, since the browser has no way
 * to prove that's the same machine (out of scope — localhost-only
 * detection, per design). */
export function isSameHostPort(a: string, b: string): boolean {
  try {
    // URL requires a supported scheme; ws(s) parses fine as a WHATWG URL.
    const ua = new URL(a);
    const ub = new URL(b);
    return normalizeLocalHost(ua.hostname) === normalizeLocalHost(ub.hostname) && ua.port === ub.port;
  } catch {
    return false;
  }
}

/** True if `url`'s host is localhost/127.0.0.1/::1, regardless of port —
 * used to prefer a non-localhost `wsUrl` (a user's hand-typed tunnel/LAN
 * address, real information worth keeping) over a localhost one when
 * merging two computer records. */
function isLocalhostUrl(url: string): boolean {
  try {
    return normalizeLocalHost(new URL(url).hostname) === 'localhost';
  } catch {
    return false;
  }
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
  /** Schema version of this persisted blob — absent/undefined means
   * pre-identity-unification (`ComputerConfig.wsUrl` always a string,
   * `mode` always the connection discriminator). See `loadState`'s
   * migration. */
  version?: number;
}

const CURRENT_STATE_VERSION = 2;

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
  /** Sets `peerId` on a computer that doesn't have one yet — the primitive
   * used once a locally-probed daemon or an already-connected direct
   * computer reveals its identity. If another computer already carries
   * that `peerId`, this delegates to `mergeComputers` instead of creating
   * a duplicate identity. */
  adoptPeerId: (computerId: string, peerId: string) => void;
  /** Folds `absorbedId` into `survivorId` — used once the local-daemon
   * probe or an account sync proves two sidebar entries are actually the
   * same physical daemon (e.g. a hand-typed `wss://localhost:54321` entry
   * and its P2P-linked counterpart). Workspaces are reparented (their `id`
   * preserved, never deleted and recreated) so the daemon-session rename
   * path (`AttachRequest.previous_session_name`) fires correctly instead
   * of orphaning a live session; same-named duplicates are then merged
   * into one. Refuses (logs only) if both computers already have a
   * `serverId` and they differ — the account server considers those
   * genuinely different computers and the client must not overrule it. */
  mergeComputers: (survivorId: string, absorbedId: string) => void;
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
    version: CURRENT_STATE_VERSION,
  };
}

function loadState(): PersistedState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return defaultState();
    const parsed = JSON.parse(raw) as PersistedState;
    if (!parsed.computers?.length) return defaultState();

    if (parsed.version !== CURRENT_STATE_VERSION) {
      // One-time snapshot before mutating anything, purely as a recovery
      // path — this migration only normalizes/backfills fields, it never
      // merges identities (that needs the async local-daemon probe result,
      // which isn't available this early — see mergeComputers/adoptPeerId,
      // run later as a runtime effect once the probe resolves).
      try {
        localStorage.setItem(`${STORAGE_KEY}.backup`, raw);
      } catch {
        // Best-effort — a full localStorage quota shouldn't block loading.
      }
      // Migrate computers persisted before `mode` existed (they were always
      // direct-URL connections), and normalize the old "p2p uses wsUrl: ''"
      // convention to the new optional-wsUrl shape.
      parsed.computers = parsed.computers.map((c) => ({
        mode: 'direct' as const,
        ...c,
        wsUrl: c.wsUrl || undefined,
      }));
      parsed.version = CURRENT_STATE_VERSION;
    }

    if (!parsed.paneNames) parsed.paneNames = {};
    return parsed;
  } catch {
    return defaultState();
  }
}

const SEEDED_DEFAULT_NAME = 'This Machine';

/** Pure reducer behind both `mergeComputers` and `adoptPeerId`'s delegated
 * case — kept outside the component (and outside `useMemo`) so it has no
 * closure over component state and can't accidentally read anything but
 * its `s` argument. See `mergeComputers`'s doc comment on `WorkspaceContextValue`
 * for the field-resolution and workspace-reparenting rules this implements. */
function mergeComputersReducer(
  s: PersistedState,
  survivorId: string,
  absorbedId: string
): PersistedState {
  if (survivorId === absorbedId) return s;
  const survivor = s.computers.find((c) => c.id === survivorId);
  const absorbed = s.computers.find((c) => c.id === absorbedId);
  if (!survivor || !absorbed) return s;

  if (survivor.serverId != null && absorbed.serverId != null && survivor.serverId !== absorbed.serverId) {
    console.warn(
      `[WorkspaceContext] Refusing to merge computers ${survivorId}/${absorbedId}: ` +
        `both have a different serverId — the account server considers them distinct computers.`
    );
    return s;
  }

  const preferServerName = survivor.serverId != null;
  const survivorIsDefault = survivor.name === SEEDED_DEFAULT_NAME;
  const name = preferServerName
    ? survivor.name
    : survivorIsDefault && absorbed.name !== SEEDED_DEFAULT_NAME
      ? absorbed.name
      : survivor.name;

  // Prefer whichever side has a non-localhost wsUrl (a real tunnel/LAN
  // address is information worth keeping); otherwise take whichever exists.
  const survivorWsIsLocal = !survivor.wsUrl || isLocalhostUrl(survivor.wsUrl);
  const wsUrl = survivorWsIsLocal && absorbed.wsUrl && !isLocalhostUrl(absorbed.wsUrl)
    ? absorbed.wsUrl
    : (survivor.wsUrl ?? absorbed.wsUrl);

  const merged: ComputerConfig = {
    ...survivor,
    name,
    wsUrl,
    peerId: survivor.peerId ?? absorbed.peerId,
    relayUrl: survivor.relayUrl ?? absorbed.relayUrl,
    serverId: survivor.serverId ?? absorbed.serverId,
  };

  // Reparent — never delete-and-recreate. Preserving workspace.id is what
  // lets the daemon-session rename path reattach to the live session
  // instead of orphaning it once `sessionKey` changes underneath it.
  const reparented = s.workspaces.map((w) =>
    w.computerId === absorbedId ? { ...w, computerId: survivorId } : w
  );

  // Dedupe within the survivor by name — the daemon resolves a session by
  // "<computer>/<workspace>" name, so two workspaces with the same name
  // under the same (now-merged) computer are always the same underlying
  // daemon session, serverId or not. Keep whichever has a serverId (the
  // synced truth), else the active workspace, else the first seen.
  const survivorWorkspaces = reparented.filter((w) => w.computerId === survivorId);
  const groups = new Map<string, WorkspaceConfig[]>();
  for (const w of survivorWorkspaces) {
    const group = groups.get(w.name);
    if (group) group.push(w);
    else groups.set(w.name, [w]);
  }

  const dropped = new Set<string>();
  const paneNameRedirects = new Map<string, string>();
  for (const group of groups.values()) {
    if (group.length < 2) continue;
    const keep =
      group.find((w) => w.serverId != null) ??
      group.find((w) => w.id === s.activeWorkspaceId) ??
      group[0];
    for (const w of group) {
      if (w.id === keep.id) continue;
      dropped.add(w.id);
      paneNameRedirects.set(w.id, keep.id);
    }
  }

  const workspaces = reparented.filter((w) => !dropped.has(w.id));

  const paneNames = { ...s.paneNames };
  for (const [droppedId, keepId] of paneNameRedirects) {
    if (paneNames[droppedId] && !paneNames[keepId]) {
      paneNames[keepId] = paneNames[droppedId];
    }
    delete paneNames[droppedId];
  }

  const computers = s.computers.filter((c) => c.id !== absorbedId).map((c) => (c.id === survivorId ? merged : c));

  const activeComputerId = s.activeComputerId === absorbedId ? survivorId : s.activeComputerId;
  const activeWorkspaceId = dropped.has(s.activeWorkspaceId ?? '')
    ? (paneNameRedirects.get(s.activeWorkspaceId ?? '') ?? s.activeWorkspaceId)
    : s.activeWorkspaceId;

  return { ...s, computers, workspaces, paneNames, activeComputerId, activeWorkspaceId };
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
          let idx = s.computers.findIndex((c) => c.peerId === peerId);
          // No computer known by this peerId yet — check whether the
          // locally-probed daemon (if any) IS this peerId, and if so
          // whether an existing peerId-less computer's wsUrl points at
          // that exact same probe URL. That's what folds the seeded
          // "This Machine" entry (or a hand-typed localhost URL) into the
          // account-synced identity on first login, instead of creating a
          // permanent duplicate the merge effect would otherwise need to
          // clean up later.
          let foldedInByHostMatch = false;
          if (idx === -1) {
            const local = getLocalDaemon();
            if (local && local.peerId === peerId) {
              idx = s.computers.findIndex(
                (c) => !c.peerId && c.wsUrl && isSameHostPort(c.wsUrl, local.url)
              );
              foldedInByHostMatch = idx !== -1;
            }
          }
          if (idx !== -1) {
            const existing = s.computers[idx];
            // First-time identity fold (no prior serverId — this device's
            // own copy of the name is what the user actually set, e.g. they
            // renamed the seeded "This Machine") must not clobber it with
            // the server's name — same rule mergeComputersReducer applies
            // for the same reason. A computer already synced (has a
            // serverId) is a genuine re-sync of an already-linked identity,
            // where the server name is the cross-device truth and should
            // win, propagating a rename made on another device.
            const resolvedName =
              foldedInByHostMatch && existing.serverId == null && existing.name !== SEEDED_DEFAULT_NAME
                ? existing.name
                : name;
            if (
              existing.serverId === serverId &&
              existing.peerId === peerId &&
              existing.name === resolvedName
            ) {
              return s;
            }
            const computers = s.computers.slice();
            computers[idx] = { ...existing, peerId, relayUrl, serverId, name: resolvedName };
            return { ...s, computers };
          }
          const computer: ComputerConfig = {
            id: makeId(),
            name,
            mode: 'p2p',
            peerId,
            relayUrl,
            serverId,
          };
          return { ...s, computers: [...s.computers, computer] };
        });
      },

      adoptPeerId: (computerId, peerId) => {
        setState((s) => {
          const target = s.computers.find((c) => c.id === computerId);
          if (!target || target.peerId === peerId) return s;
          const owner = s.computers.find((c) => c.peerId === peerId);
          if (owner) {
            // Someone else already has this identity — this is really a
            // merge, not a fresh adoption. Delegate rather than duplicate.
            return mergeComputersReducer(s, owner.id, computerId);
          }
          return {
            ...s,
            computers: s.computers.map((c) => (c.id === computerId ? { ...c, peerId } : c)),
          };
        });
      },

      mergeComputers: (survivorId, absorbedId) => {
        setState((s) => mergeComputersReducer(s, survivorId, absorbedId));
      },

      mergeServerWorkspaces: (computerId, serverWorkspaces) => {
        setState((s) => {
          const byId = new Map(serverWorkspaces.map((w) => [w.id, w]));
          const byName = new Map(serverWorkspaces.map((w) => [w.name, w]));
          let changed = false;

          // Pass 1: reconcile every local workspace already linked to a
          // server row (has serverId) by that ID, not by name — a rename
          // on another device changes the server row's name but not its
          // id, and matching by name only would miss it entirely, leaving
          // this device pointed at a now-abandoned session under the
          // stale name (the daemon resolves sessions purely by name, so a
          // name mismatch here means genuinely talking to a different,
          // orphaned session). A local workspace with no serverId yet is
          // still matched by name once, to link up on first sync.
          const workspaces = s.workspaces.map((w) => {
            if (w.computerId !== computerId) return w;
            if (w.serverId != null) {
              const match = byId.get(w.serverId);
              if (match && match.name !== w.name) {
                changed = true;
                return { ...w, name: match.name };
              }
              return w;
            }
            const match = byName.get(w.name);
            if (match) {
              changed = true;
              return { ...w, serverId: match.id };
            }
            return w;
          });

          // Pass 2: any server workspace not now linked to a local one
          // (by id, after pass 1) is genuinely new to this device.
          const linkedServerIds = new Set(
            workspaces
              .filter((w) => w.computerId === computerId && w.serverId != null)
              .map((w) => w.serverId)
          );
          const toAdd = serverWorkspaces.filter((w) => !linkedServerIds.has(w.id));

          if (!changed && toAdd.length === 0) return s;

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
