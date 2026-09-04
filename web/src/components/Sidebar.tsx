import { useEffect, useRef, useState } from 'react';
import { isSameHostPort, resolveRoutes, useWorkspace } from '../state/WorkspaceContext';
import type { ComputerConfig } from '../state/WorkspaceContext';
import { getLocalDaemon, subscribeLocalDaemon } from '../lib/local-daemon';
import { IconClose, IconFolder, IconMonitor, IconPlus, IconPrompt } from './icons';
import {
  createWorkspace,
  deleteWorkspaceRemote,
  getStoredAuth,
  linkComputer,
  listComputers,
  listWorkspaces,
  logout,
  renameComputerRemote,
  renameWorkspaceRemote,
  toRelayWsUrl,
} from '../lib/accounts-client';
import type { LinkedComputer } from '../lib/accounts-client';
import type { WorkspacePanes } from './WorkspaceSession';
import './Sidebar.css';

/** `null` when this computer has no known route at all (shouldn't happen in
 * practice — every computer has at least a wsUrl or a peerId+relayUrl) or
 * when its best route is a plain configured URL, matching today's "no
 * badge for a direct computer" behavior. */
function routeBadge(computer: ComputerConfig): 'Local' | 'P2P' | null {
  const local = getLocalDaemon();
  const top = resolveRoutes(computer, local)[0];
  if (!top) return null;
  if (top.kind === 'p2p') return 'P2P';
  if (local && computer.peerId && local.peerId === computer.peerId) return 'Local';
  return null;
}

interface SidebarProps {
  /** Mobile only: whether the off-canvas drawer is open. Ignored at desktop
   * widths, where the sidebar is always visible inline. */
  isOpen: boolean;
  onClose: () => void;
  /** Each workspace's current live panes, as last reported by its
   * WorkspaceSession — so a workspace's sessions are visible (and
   * selectable) here even on mobile, where the pane grid itself collapses
   * to a single focused pane. */
  panesByWorkspace: Map<string, WorkspacePanes>;
  onSelectPane: (workspaceId: string, paneId: string) => void;
}

type PendingRemoval =
  | { kind: 'computer'; id: string; label: string }
  | { kind: 'workspace'; id: string; label: string };

export function Sidebar({ isOpen, onClose, panesByWorkspace, onSelectPane }: SidebarProps) {
  const {
    computers,
    workspaces,
    paneNames,
    activeComputerId,
    activeWorkspaceId,
    addComputer,
    addLinkedComputer,
    mergeServerWorkspaces,
    setWorkspaceServerId,
    adoptPeerId,
    mergeComputers,
    isFirstRun,
    removeComputer,
    renameComputer,
    addWorkspace,
    removeWorkspace,
    renameWorkspace,
    renamePane,
    setActiveComputerId,
    setActiveWorkspaceId,
  } = useWorkspace();

  const [addingComputer, setAddingComputer] = useState(false);
  const [newComputerName, setNewComputerName] = useState('');
  const [newComputerMode, setNewComputerMode] = useState<'direct' | 'p2p'>('direct');
  const [newComputerUrl, setNewComputerUrl] = useState('wss://localhost:54321');
  const [newComputerPeerId, setNewComputerPeerId] = useState('');
  const [newComputerRelayUrl, setNewComputerRelayUrl] = useState('');

  const [renamingWorkspaceId, setRenamingWorkspaceId] = useState<string | null>(null);
  const [workspaceNameDraft, setWorkspaceNameDraft] = useState('');
  const [renamingComputerId, setRenamingComputerId] = useState<string | null>(null);
  const [computerNameDraft, setComputerNameDraft] = useState('');
  const [renamingPane, setRenamingPane] = useState<{ workspaceId: string; paneId: string } | null>(null);
  const [paneNameDraft, setPaneNameDraft] = useState('');

  const [pendingRemoval, setPendingRemoval] = useState<PendingRemoval | null>(null);

  // Mirrors `workspaces` for reading the *current* name from inside an
  // async callback (see handleAddWorkspace) — a plain closure over
  // `workspaces` would see whatever it was at the time the callback was
  // created, not whatever it's since become after a same-tick rename.
  const workspacesRef = useRef(workspaces);
  useEffect(() => {
    workspacesRef.current = workspaces;
  }, [workspaces]);

  const auth = getStoredAuth();
  const [linkedComputers, setLinkedComputers] = useState<LinkedComputer[]>([]);
  const [loadingLinked, setLoadingLinked] = useState(false);
  const [linkedError, setLinkedError] = useState<string | null>(null);
  const [showLinkForm, setShowLinkForm] = useState(false);
  const [linkCode, setLinkCode] = useState('');
  const [linkName, setLinkName] = useState('');
  const [linking, setLinking] = useState(false);
  const [linkFormError, setLinkFormError] = useState<string | null>(null);

  function refreshLinkedComputers() {
    if (!auth) return;
    setLoadingLinked(true);
    setLinkedError(null);
    listComputers()
      .then((res) => setLinkedComputers(res.computers))
      .catch((err) => setLinkedError(err instanceof Error ? err.message : 'Failed to load computers'))
      .finally(() => setLoadingLinked(false));
  }

  useEffect(() => {
    if (addingComputer && auth) {
      refreshLinkedComputers();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [addingComputer]);

  // Auto-discover computers already linked to this account so they show up
  // in the sidebar without the user having to open "Add Computer" — that
  // flow stays for linking a genuinely new device. AuthGate only mounts
  // this component after login succeeds, so a plain mount-time check is
  // sufficient here.
  useEffect(() => {
    if (auth) refreshLinkedComputers();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!auth) return;
    for (const c of linkedComputers) {
      addLinkedComputer(c.name || `Computer ${c.id}`, c.peer_id, toRelayWsUrl(auth.baseUrl), c.id);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [linkedComputers]);

  // Pull each linked computer's workspace list from the account server and
  // merge it in, so logging into the same account elsewhere shows the same
  // workspaces (see mergeServerWorkspaces's doc comment — additive-only,
  // fetch-on-load, not a live push). Depends on `computers` too so it
  // re-resolves once the merge effect above has actually added the local
  // ComputerConfig for a newly-discovered linked computer; `listWorkspaces`
  // + `mergeServerWorkspaces` are both cheap no-ops on a redundant re-fire,
  // so re-running this on unrelated `computers` changes (e.g. renaming a
  // different computer) is wasteful but harmless.
  useEffect(() => {
    if (!auth) return;
    for (const lc of linkedComputers) {
      const local = computers.find((c) => c.peerId === lc.peer_id);
      if (!local) continue;
      listWorkspaces(lc.id)
        .then((res) => mergeServerWorkspaces(local.id, res.workspaces))
        .catch(() => {
          // Best-effort: this device just keeps whatever workspaces it
          // already has locally and tries again next load.
        });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [linkedComputers, computers]);

  // On a device that's never used the app before, the seeded "This Machine"
  // (mode: 'direct', wsUrl pointing at localhost) is a meaningless
  // placeholder — nothing is actually listening there on, say, a phone.
  // Switch to the first account-linked computer once it appears so logging
  // in actually shows something reachable, instead of leaving the user
  // stuck on a dead default. Only ever fires once per session, and never on
  // a device with pre-existing state worth protecting.
  const firstRunAutoSelectDone = useRef(false);
  useEffect(() => {
    if (!isFirstRun || firstRunAutoSelectDone.current || linkedComputers.length === 0) return;
    const match = computers.find((c) => c.peerId === linkedComputers[0].peer_id);
    if (match) {
      firstRunAutoSelectDone.current = true;
      setActiveComputerId(match.id);
    }
  }, [isFirstRun, linkedComputers, computers, setActiveComputerId]);

  // Once the local-daemon probe (local-daemon.ts) resolves, check whether it
  // matches an already-synced computer (fold in any peerId-less duplicate —
  // typically the seeded "This Machine" entry) or a still-unidentified
  // direct computer (give it an identity, so the next account sync finds it
  // by peerId instead of creating a real duplicate). Strictly host+port
  // matched against the probe's own URL — never against any other direct
  // URL — so a LAN address is never treated as "this machine" (localhost-
  // only detection, per design).
  const processedIdentityPairs = useRef<Set<string>>(new Set());
  useEffect(() => {
    function reconcile() {
      const local = getLocalDaemon();
      if (!local) return;
      const owner = computers.find((c) => c.peerId === local.peerId);
      const candidates = computers.filter(
        (c) => !c.peerId && c.wsUrl && isSameHostPort(c.wsUrl, local.url)
      );
      if (owner) {
        for (const candidate of candidates) {
          const key = `${local.peerId}:${candidate.id}`;
          if (processedIdentityPairs.current.has(key)) continue;
          processedIdentityPairs.current.add(key);
          mergeComputers(owner.id, candidate.id);
        }
      } else if (candidates.length === 1) {
        const key = `${local.peerId}:${candidates[0].id}`;
        if (!processedIdentityPairs.current.has(key)) {
          processedIdentityPairs.current.add(key);
          adoptPeerId(candidates[0].id, local.peerId);
        }
      }
    }
    reconcile();
    return subscribeLocalDaemon(reconcile);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [computers]);

  function handlePickLinkedComputer(c: LinkedComputer) {
    // Auto-discovery (the effect above `refreshLinkedComputers` feeds) has
    // already created a local ComputerConfig for every linked computer —
    // this is a selector, not an adder. Using addComputer here used to
    // create a genuine duplicate (never carrying serverId) alongside it.
    const match = computers.find((comp) => comp.peerId === c.peer_id);
    if (match) setActiveComputerId(match.id);
    setAddingComputer(false);
  }

  async function handleLinkComputer() {
    const code = linkCode.trim();
    if (!code) return;
    setLinking(true);
    setLinkFormError(null);
    try {
      await linkComputer(code, linkName.trim() || undefined);
      setLinkCode('');
      setLinkName('');
      setShowLinkForm(false);
      refreshLinkedComputers();
    } catch (err) {
      setLinkFormError(err instanceof Error ? err.message : 'Failed to link computer');
    } finally {
      setLinking(false);
    }
  }

  function handleLogout() {
    logout();
    window.location.reload();
  }

  function handleAddComputer() {
    const name = newComputerName.trim();
    if (!name) return;

    if (newComputerMode === 'direct') {
      const url = newComputerUrl.trim();
      if (!url) return;
      addComputer(name, { mode: 'direct', wsUrl: url });
    } else {
      const peerId = newComputerPeerId.trim();
      const relayUrl = newComputerRelayUrl.trim();
      if (!peerId || !relayUrl) return;
      addComputer(name, { mode: 'p2p', peerId, relayUrl });
    }

    setNewComputerName('');
    setNewComputerUrl('wss://localhost:54321');
    setNewComputerPeerId('');
    setNewComputerRelayUrl('');
    setAddingComputer(false);
  }

  // Create the workspace immediately with an auto-generated name, then drop
  // straight into inline rename so the user can edit it without being forced
  // to type a name up front.
  function handleAddWorkspace(computerId: string) {
    const count = workspaces.filter((w) => w.computerId === computerId).length;
    const name = `Workspace ${count + 1}`;
    const id = addWorkspace(computerId, name);
    setRenamingWorkspaceId(id);
    setWorkspaceNameDraft(name);

    // Best-effort push to the account server so other devices pick this
    // workspace up on their next load — only possible for a linked (p2p)
    // computer, which is the only kind with a server-side identity.
    const computer = computers.find((c) => c.id === computerId);
    if (computer?.serverId) {
      createWorkspace(computer.serverId, name)
        .then((res) => {
          setWorkspaceServerId(id, res.workspace.id);
          // Rename-on-create means the user may have already renamed this
          // workspace locally before this request resolved — if so, the
          // server row is still stuck under the auto-generated name from
          // above; push the current name too instead of leaving it to
          // drift (mergeServerWorkspaces would otherwise re-add the stale
          // name as a spurious duplicate on the next sync).
          const current = workspacesRef.current.find((w) => w.id === id);
          if (current && current.name !== name) {
            renameWorkspaceRemote(res.workspace.id, current.name).catch(() => {});
          }
        })
        .catch(() => {
          // The workspace still exists locally either way; it'll sync on a
          // future load if this attempt failed (e.g. offline).
        });
    }
  }

  function commitWorkspaceRename() {
    if (renamingWorkspaceId) {
      const name = workspaceNameDraft.trim();
      if (name) {
        renameWorkspace(renamingWorkspaceId, name);
        const workspace = workspaces.find((w) => w.id === renamingWorkspaceId);
        if (workspace?.serverId) {
          renameWorkspaceRemote(workspace.serverId, name).catch(() => {});
        }
      }
    }
    setRenamingWorkspaceId(null);
  }

  function startComputerRename(id: string, currentName: string) {
    setRenamingComputerId(id);
    setComputerNameDraft(currentName);
  }

  function commitComputerRename() {
    if (renamingComputerId) {
      const name = computerNameDraft.trim();
      if (name) {
        renameComputer(renamingComputerId, name);
        const computer = computers.find((c) => c.id === renamingComputerId);
        if (computer?.serverId) {
          renameComputerRemote(computer.serverId, name).catch(() => {});
        }
      }
    }
    setRenamingComputerId(null);
  }

  function startPaneRename(workspaceId: string, paneId: string, currentName: string) {
    setRenamingPane({ workspaceId, paneId });
    setPaneNameDraft(currentName);
  }

  function commitPaneRename() {
    if (renamingPane) {
      const name = paneNameDraft.trim();
      if (name) renamePane(renamingPane.workspaceId, renamingPane.paneId, name);
    }
    setRenamingPane(null);
  }

  function confirmRemoval() {
    if (!pendingRemoval) return;
    if (pendingRemoval.kind === 'computer') {
      removeComputer(pendingRemoval.id);
    } else {
      const workspace = workspaces.find((w) => w.id === pendingRemoval.id);
      removeWorkspace(pendingRemoval.id);
      if (workspace?.serverId) {
        deleteWorkspaceRemote(workspace.serverId).catch(() => {});
      }
    }
    setPendingRemoval(null);
  }

  // Selecting a workspace or one of its panes is the "I'm done browsing,
  // show me the thing" action on mobile — close the drawer so the
  // workspace's panes take the full screen.
  function selectWorkspace(id: string) {
    setActiveWorkspaceId(id);
    onClose();
  }

  function selectPane(workspaceId: string, paneId: string) {
    setActiveWorkspaceId(workspaceId);
    onSelectPane(workspaceId, paneId);
    onClose();
  }

  return (
    <>
      <div
        className={`sidebar-backdrop ${isOpen ? 'visible' : ''}`}
        onClick={onClose}
        aria-hidden="true"
      />
      <nav
        className={`sidebar ${isOpen ? 'open' : ''}`}
        aria-label="Computers, workspaces, and terminals"
      >
        <div className="sidebar-header">
          <span>Computers</span>
          <div className="sidebar-header-actions">
            {auth && (
              <button
                className="sidebar-logout-btn"
                onClick={handleLogout}
                aria-label={`Log out of ${auth.email}`}
                title={`Log out of ${auth.email}`}
              >
                Log out
              </button>
            )}
            <button
              className="sidebar-add-btn"
              onClick={() => setAddingComputer(true)}
              aria-label="Add computer"
              title="Add computer"
            >
              <IconPlus width={14} height={14} />
            </button>
          </div>
        </div>

        {addingComputer && (
          <div className="sidebar-add-form">
            {auth ? (
              // Logged in: every computer worth adding has (or can get) an
              // account-synced identity, so pairing-code linking is the
              // only path — a hand-typed direct URL or raw peer/relay pair
              // would just be a duplicate the merge effect above has to
              // clean up later. The seeded local daemon (if any) still
              // shows up here via auto-discovery, not a separate form.
              <div className="sidebar-linked-picker">
                {loadingLinked && <div className="sidebar-empty">Loading your computers...</div>}
                {linkedError && <div className="sidebar-linked-error">{linkedError}</div>}
                {!loadingLinked && !linkedError && linkedComputers.length === 0 && !showLinkForm && (
                  <div className="sidebar-empty">No linked computers yet.</div>
                )}
                {!loadingLinked &&
                  linkedComputers.map((c) => (
                    <button
                      key={c.id}
                      type="button"
                      className="sidebar-linked-computer"
                      onClick={() => handlePickLinkedComputer(c)}
                    >
                      <span
                        className={`sidebar-online-dot ${c.online ? 'online' : 'offline'}`}
                        aria-hidden="true"
                      />
                      <span className="sidebar-label">{c.name}</span>
                    </button>
                  ))}

                {showLinkForm ? (
                  <div className="sidebar-link-form">
                    <input
                      autoFocus
                      placeholder="Pairing code"
                      value={linkCode}
                      onChange={(e) => setLinkCode(e.target.value)}
                      onKeyDown={(e) => e.key === 'Enter' && handleLinkComputer()}
                    />
                    <input
                      placeholder="Name (optional)"
                      value={linkName}
                      onChange={(e) => setLinkName(e.target.value)}
                      onKeyDown={(e) => e.key === 'Enter' && handleLinkComputer()}
                    />
                    {linkFormError && <div className="sidebar-linked-error">{linkFormError}</div>}
                    <div className="sidebar-add-actions">
                      <button type="button" onClick={handleLinkComputer} disabled={linking}>
                        {linking ? 'Linking...' : 'Link'}
                      </button>
                      <button type="button" onClick={() => setShowLinkForm(false)}>
                        Cancel
                      </button>
                    </div>
                  </div>
                ) : (
                  <button
                    type="button"
                    className="sidebar-link-new-btn"
                    onClick={() => setShowLinkForm(true)}
                  >
                    <IconPlus width={12} height={12} />
                    Link a new computer
                  </button>
                )}

                <div className="sidebar-add-actions">
                  <button onClick={() => setAddingComputer(false)}>Close</button>
                </div>
              </div>
            ) : (
              // Logged out: nothing can be synced, so this stays exactly
              // the original manual entry flow — direct URL or a raw
              // peer/relay pair for someone running their own relay.
              <>
                <input
                  autoFocus
                  placeholder="Name"
                  value={newComputerName}
                  onChange={(e) => setNewComputerName(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleAddComputer()}
                />
                <div className="sidebar-mode-toggle" role="radiogroup" aria-label="Connection type">
                  <button
                    type="button"
                    role="radio"
                    aria-checked={newComputerMode === 'direct'}
                    className={newComputerMode === 'direct' ? 'active' : ''}
                    onClick={() => setNewComputerMode('direct')}
                  >
                    Direct
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={newComputerMode === 'p2p'}
                    className={newComputerMode === 'p2p' ? 'active' : ''}
                    onClick={() => setNewComputerMode('p2p')}
                  >
                    P2P (remote)
                  </button>
                </div>
                {newComputerMode === 'direct' ? (
                  <input
                    placeholder="wss://host:54321"
                    value={newComputerUrl}
                    onChange={(e) => setNewComputerUrl(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && handleAddComputer()}
                  />
                ) : (
                  <>
                    <input
                      placeholder="Peer ID (from the daemon's tray/dashboard)"
                      value={newComputerPeerId}
                      onChange={(e) => setNewComputerPeerId(e.target.value)}
                      onKeyDown={(e) => e.key === 'Enter' && handleAddComputer()}
                    />
                    <input
                      placeholder="Relay URL, e.g. ws://relay.example.com:9000"
                      value={newComputerRelayUrl}
                      onChange={(e) => setNewComputerRelayUrl(e.target.value)}
                      onKeyDown={(e) => e.key === 'Enter' && handleAddComputer()}
                    />
                  </>
                )}
                <div className="sidebar-add-actions">
                  <button onClick={handleAddComputer}>Add</button>
                  <button onClick={() => setAddingComputer(false)}>Cancel</button>
                </div>
              </>
            )}
          </div>
        )}

        <div className="sidebar-tree">
          {computers.length === 0 && (
            <div className="sidebar-empty">
              No computers yet. Tap <strong>+</strong> above to connect to one.
            </div>
          )}

          {computers.map((computer) => {
            const computerWorkspaces = workspaces.filter((w) => w.computerId === computer.id);
            const isActiveComputer = computer.id === activeComputerId;

            return (
              <div key={computer.id} className="sidebar-computer">
                <div
                  className={`sidebar-row sidebar-computer-row ${isActiveComputer ? 'active' : ''}`}
                  onClick={() => setActiveComputerId(computer.id)}
                >
                  <IconMonitor className="sidebar-icon" width={15} height={15} />
                  {renamingComputerId === computer.id ? (
                    <input
                      autoFocus
                      className="sidebar-rename-input"
                      value={computerNameDraft}
                      onClick={(e) => e.stopPropagation()}
                      onChange={(e) => setComputerNameDraft(e.target.value)}
                      onBlur={commitComputerRename}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') commitComputerRename();
                        else if (e.key === 'Escape') setRenamingComputerId(null);
                      }}
                    />
                  ) : (
                    <span
                      className="sidebar-label"
                      title={computer.wsUrl || (computer.peerId ? `P2P: ${computer.peerId}` : undefined)}
                      onDoubleClick={(e) => {
                        e.stopPropagation();
                        startComputerRename(computer.id, computer.name);
                      }}
                    >
                      {computer.name}
                      {routeBadge(computer) && (
                        <span
                          className="sidebar-mode-badge"
                          title={
                            routeBadge(computer) === 'Local'
                              ? 'Connected directly to this machine’s daemon'
                              : 'Connected via WebRTC P2P'
                          }
                        >
                          {routeBadge(computer)}
                        </span>
                      )}
                    </span>
                  )}
                  <button
                    className="sidebar-row-action"
                    title="Add workspace"
                    aria-label={`Add workspace to ${computer.name}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      setActiveComputerId(computer.id);
                      handleAddWorkspace(computer.id);
                    }}
                  >
                    <IconPlus width={13} height={13} />
                  </button>
                  <button
                    className="sidebar-row-action sidebar-row-action-danger"
                    title="Remove computer"
                    aria-label={`Remove ${computer.name}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      setPendingRemoval({ kind: 'computer', id: computer.id, label: computer.name });
                    }}
                  >
                    <IconClose width={13} height={13} />
                  </button>
                </div>

                {isActiveComputer &&
                  computerWorkspaces.map((workspace) => {
                    const isActiveWorkspace = workspace.id === activeWorkspaceId;
                    const isRenaming = renamingWorkspaceId === workspace.id;

                    return (
                      <div key={workspace.id} className="sidebar-workspace">
                        <div
                          className={`sidebar-row sidebar-workspace-row ${isActiveWorkspace ? 'active' : ''}`}
                          onClick={() => !isRenaming && selectWorkspace(workspace.id)}
                        >
                          <IconFolder className="sidebar-icon" width={14} height={14} />
                          {isRenaming ? (
                            <input
                              autoFocus
                              className="sidebar-rename-input"
                              value={workspaceNameDraft}
                              onClick={(e) => e.stopPropagation()}
                              onChange={(e) => setWorkspaceNameDraft(e.target.value)}
                              onBlur={commitWorkspaceRename}
                              onKeyDown={(e) => {
                                if (e.key === 'Enter') commitWorkspaceRename();
                                else if (e.key === 'Escape') setRenamingWorkspaceId(null);
                              }}
                            />
                          ) : (
                            <span
                              className="sidebar-label"
                              onDoubleClick={(e) => {
                                e.stopPropagation();
                                setRenamingWorkspaceId(workspace.id);
                                setWorkspaceNameDraft(workspace.name);
                              }}
                              title="Double-click to rename"
                            >
                              {workspace.name}
                            </span>
                          )}
                          <button
                            className="sidebar-row-action sidebar-row-action-danger"
                            title="Remove workspace"
                            aria-label={`Remove ${workspace.name}`}
                            onClick={(e) => {
                              e.stopPropagation();
                              setPendingRemoval({
                                kind: 'workspace',
                                id: workspace.id,
                                label: workspace.name,
                              });
                            }}
                          >
                            <IconClose width={13} height={13} />
                          </button>
                        </div>

                        {isActiveWorkspace &&
                          (panesByWorkspace.get(workspace.id)?.paneIds ?? []).map((paneId) => (
                            <div
                              key={paneId}
                              className={`sidebar-row sidebar-terminal-row ${
                                paneId === panesByWorkspace.get(workspace.id)?.focusedPaneId
                                  ? 'active'
                                  : ''
                              }`}
                              onClick={() =>
                                !(
                                  renamingPane?.workspaceId === workspace.id &&
                                  renamingPane?.paneId === paneId
                                ) && selectPane(workspace.id, paneId)
                              }
                            >
                              <IconPrompt className="sidebar-icon" width={14} height={14} />
                              {renamingPane?.workspaceId === workspace.id &&
                              renamingPane?.paneId === paneId ? (
                                <input
                                  autoFocus
                                  className="sidebar-rename-input"
                                  value={paneNameDraft}
                                  onClick={(e) => e.stopPropagation()}
                                  onChange={(e) => setPaneNameDraft(e.target.value)}
                                  onBlur={commitPaneRename}
                                  onKeyDown={(e) => {
                                    if (e.key === 'Enter') commitPaneRename();
                                    else if (e.key === 'Escape') setRenamingPane(null);
                                  }}
                                />
                              ) : (
                                <span
                                  className="sidebar-label"
                                  onDoubleClick={(e) => {
                                    e.stopPropagation();
                                    startPaneRename(
                                      workspace.id,
                                      paneId,
                                      paneNames[workspace.id]?.[paneId] ?? paneId
                                    );
                                  }}
                                  title="Double-click to rename"
                                >
                                  {paneNames[workspace.id]?.[paneId] ?? paneId}
                                </span>
                              )}
                            </div>
                          ))}
                      </div>
                    );
                  })}
              </div>
            );
          })}
        </div>
      </nav>

      {pendingRemoval && (
        <div className="confirm-modal-backdrop" onClick={() => setPendingRemoval(null)}>
          <div
            className="confirm-modal"
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="confirm-modal-title"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 id="confirm-modal-title">Remove {pendingRemoval.kind}?</h2>
            <p>
              {pendingRemoval.kind === 'computer' &&
                `This will remove "${pendingRemoval.label}" along with all of its workspaces and panes.`}
              {pendingRemoval.kind === 'workspace' &&
                `This will remove "${pendingRemoval.label}" along with all of its panes.`}
            </p>
            <div className="confirm-modal-actions">
              <button className="confirm-modal-cancel" onClick={() => setPendingRemoval(null)}>
                Cancel
              </button>
              <button className="confirm-modal-confirm" onClick={confirmRemoval} autoFocus>
                Remove
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
