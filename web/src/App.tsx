import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { AuthGate } from './components/AuthGate';
import { Sidebar } from './components/Sidebar';
import { WorkspaceSession } from './components/WorkspaceSession';
import type { WorkspacePanes, WorkspaceSessionHandle } from './components/WorkspaceSession';
import { MobileKeyboard } from './components/MobileKeyboard';
import { MonomindPanel } from './components/MonomindPanel';
import { InstallPrompt } from './components/InstallPrompt';
import { IconClose, IconMenu, IconPanel, IconPrompt } from './components/icons';
import { ConnectionState, WebSocketClient } from './lib/websocket-client';
import { getStoredAuth, linkComputer, listComputers } from './lib/accounts-client';
import { getLocalDaemon, probeLocalDaemon, refreshLocalDaemon, subscribeLocalDaemon } from './lib/local-daemon';
import { resolveRoutes, useWorkspace } from './state/WorkspaceContext';
import './App.css';

// Detect if running on mobile
const isMobile = /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(
  navigator.userAgent
);

function App() {
  const { computers, workspaces, activeWorkspaceId, adoptPeerId } = useWorkspace();
  const [showMonomindPanel, setShowMonomindPanel] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const sessionRefs = useRef<Map<string, WorkspaceSessionHandle | null>>(new Map());
  const [panesByWorkspace, setPanesByWorkspace] = useState<Map<string, WorkspacePanes>>(new Map());

  const handleSelectPane = useCallback((workspaceId: string, paneId: string) => {
    sessionRefs.current.get(workspaceId)?.focusPane(paneId);
  }, []);

  const activeWorkspace = useMemo(
    () => workspaces.find((w) => w.id === activeWorkspaceId),
    [workspaces, activeWorkspaceId]
  );
  const activeComputer = useMemo(
    () => computers.find((c) => c.id === activeWorkspace?.computerId),
    [computers, activeWorkspace]
  );

  const [monomindClient, setMonomindClient] = useState<WebSocketClient | null>(null);

  // Detect a daemon reachable at wss://localhost:54321 once per app load —
  // computers/transport code reads the result via getLocalDaemon() rather
  // than through component state (see local-daemon.ts's module doc comment
  // for why this is a singleton, not per-component state).
  // Bumped whenever the local-daemon probe resolves/refreshes. Exposed as a
  // value (not just used to force a render) so effects that read
  // getLocalDaemon() — like the Monomind-panel one below — can list it as a
  // dependency and actually re-run when the probe result changes; a bare
  // re-render alone does not re-execute an effect whose deps didn't change.
  const [probeTick, setProbeTick] = useState(0);
  useEffect(() => {
    probeLocalDaemon();
    const onVisible = () => {
      if (document.visibilityState === 'visible') refreshLocalDaemon();
    };
    window.addEventListener('online', refreshLocalDaemon);
    document.addEventListener('visibilitychange', onVisible);
    const unsubscribe = subscribeLocalDaemon(() => setProbeTick((n) => n + 1));
    return () => {
      window.removeEventListener('online', refreshLocalDaemon);
      document.removeEventListener('visibilitychange', onVisible);
      unsubscribe();
    };
  }, []);

  useEffect(() => {
    if (!activeComputer) return;
    // The Monomind panel talks straight to a daemon's WebSocket dashboard
    // command channel, not through HybridTransport/P2P — it only has
    // anything to connect to when this computer has a direct ws route
    // (a configured wsUrl, or a locally-probed match). A p2p-only computer
    // with no local match has no route here at all; the panel simply isn't
    // rendered for it (see the `monomindClient &&` check below) rather than
    // constructing a WebSocket against an empty URL, which used to throw
    // silently and leave the panel permanently non-functional.
    //
    // Depends on `probeTick` too, not just `activeComputer` — without it,
    // selecting a P2P-only computer *before* the local-daemon probe
    // resolves would find no ws route once, hide the panel, and never
    // re-check even after the probe later reveals the same daemon is also
    // reachable locally (the effect only reruns when its deps change, and
    // `activeComputer` itself wouldn't have changed).
    const route = resolveRoutes(activeComputer, getLocalDaemon()).find((r) => r.kind === 'ws');
    if (!route) {
      setMonomindClient(null);
      return;
    }
    const client = new WebSocketClient({
      url: route.url,
      autoReconnect: true,
      reconnectInterval: 3000,
      maxReconnectAttempts: 5,
    });
    client.connect();
    setMonomindClient(client);
    return () => client.disconnect();
  }, [activeComputer, probeTick]);

  // Auto-link this machine to the logged-in monoes.me account the first
  // time we get a direct, authenticated connection to its daemon — opening
  // that connection already proves this is the user's machine, and being
  // logged in proves which account, so no manual pairing code is needed
  // (unlike linking a genuinely different device, which still requires one).
  const autoLinkAttempted = useRef<Set<string>>(new Set());
  useEffect(() => {
    if (!monomindClient || !activeComputer || activeComputer.mode !== 'direct') return;
    if (!getStoredAuth()) return;

    const attemptAutoLink = () => {
      if (autoLinkAttempted.current.has(activeComputer.id)) return;
      autoLinkAttempted.current.add(activeComputer.id);

      (async () => {
        try {
          const peerIdResp = await monomindClient.sendDashboardRequest({ command: 'account_peer_id' });
          if (peerIdResp.error !== 0) return; // daemon too old to answer (pre identity-unification)
          const { peer_id: peerId } = JSON.parse(peerIdResp.jsonData) as { peer_id: string };

          // This computer is now provably reachable at this peer_id — give
          // it that identity (or merge, if some other local entry already
          // has it) even before checking whether the account itself has
          // linked it yet. Generalizes identity unification beyond
          // localhost: a VPN/tunnel direct URL to the same daemon gets
          // folded in here too, the moment it's actually connected to.
          adoptPeerId(activeComputer.id, peerId);

          const { computers: linked } = await listComputers();
          if (linked.some((c) => c.peer_id === peerId)) return; // already linked

          const codeResp = await monomindClient.sendDashboardRequest({ command: 'account_pairing_code' });
          if (codeResp.error !== 0) return;
          const { code } = JSON.parse(codeResp.jsonData) as { code: string };

          await linkComputer(code, activeComputer.name);
        } catch (err) {
          console.warn('Auto-link of this machine failed (will retry next load):', err);
        }
      })();
    };

    // The listener only fires on future transitions, so check the current
    // state too — the client may have already reached CONNECTED between
    // being created (previous effect) and this effect attaching a listener.
    if (monomindClient.getState() === ConnectionState.CONNECTED) {
      attemptAutoLink();
    }
    const unsubscribe = monomindClient.onStateChange((state) => {
      if (state === ConnectionState.CONNECTED) attemptAutoLink();
    });

    return unsubscribe;
  }, [monomindClient, activeComputer, adoptPeerId]);

  const handleMobileKey = useCallback(
    (key: string) => {
      if (!activeWorkspaceId) return;
      sessionRefs.current.get(activeWorkspaceId)?.sendInputToFocused(key);
    },
    [activeWorkspaceId]
  );

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ctrl+M or Cmd+M to toggle Monomind panel
      if ((e.ctrlKey || e.metaKey) && e.key === 'm') {
        e.preventDefault();
        setShowMonomindPanel((prev) => !prev);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  return (
    <AuthGate>
    <div className="app app-with-sidebar">
      {/* Header with controls */}
      <header className="app-header">
        <div className="header-left">
          <button
            className="sidebar-toggle-btn"
            onClick={() => setSidebarOpen(true)}
            aria-label="Open sidebar"
            title="Open sidebar"
          >
            <IconMenu width={20} height={20} />
          </button>
          <h1>MONOTERMINAL</h1>
          {activeWorkspace && (
            <span className="active-terminal-label">{activeWorkspace.name}</span>
          )}
        </div>
        <div className="header-right">
          <button
            className="panel-toggle-btn"
            onClick={() => setShowMonomindPanel(!showMonomindPanel)}
            aria-label="Toggle Monomind panel"
            title="Toggle Monomind panel (Ctrl+M)"
            data-testid="dashboard-toggle"
          >
            {showMonomindPanel ? <IconClose width={17} height={17} /> : <IconPanel width={17} height={17} />}
          </button>
        </div>
      </header>

      <div className="app-body">
        <Sidebar
          isOpen={sidebarOpen}
          onClose={() => setSidebarOpen(false)}
          panesByWorkspace={panesByWorkspace}
          onSelectPane={handleSelectPane}
        />

        {/* Main terminal area */}
        <main className={`app-main ${showMonomindPanel ? 'panel-open' : ''}`}>
          <div className="terminal-container">
            {workspaces.length === 0 ? (
              <div className="no-terminals">
                <IconPrompt width={28} height={28} />
                <p>No terminal open</p>
                <span>Use the sidebar to add a computer and a workspace.</span>
              </div>
            ) : (
              // Every workspace is mounted (hidden via CSS when not active),
              // not unmounted, so its connection and panes keep running in
              // the background while another workspace is shown — matching
              // real terminal multiplexers, where switching views never
              // kills the underlying sessions.
              workspaces.map((workspace) => {
                const computer = computers.find((c) => c.id === workspace.computerId);
                if (!computer) return null;
                // Derived from names, not a local random id, so the same
                // workspace opened from another browser/device/tab attaches
                // to the same live layout (see resolve_named_session).
                const sessionKey = `${computer.name}/${workspace.name}`;
                return (
                  <WorkspaceSession
                    key={workspace.id}
                    ref={(handle) => {
                      sessionRefs.current.set(workspace.id, handle);
                    }}
                    computer={computer}
                    workspaceId={workspace.id}
                    visible={workspace.id === activeWorkspaceId}
                    sessionKey={sessionKey}
                    onPanesChange={(panes) =>
                      setPanesByWorkspace((m) => new Map(m).set(workspace.id, panes))
                    }
                  />
                );
              })
            )}
          </div>

          {/* Monomind panel */}
          {monomindClient && (
            <MonomindPanel
              isVisible={showMonomindPanel}
              onClose={() => setShowMonomindPanel(false)}
              wsClient={monomindClient}
            />
          )}
        </main>
      </div>

      {/* Mobile keyboard (hidden on desktop) */}
      {isMobile && <MobileKeyboard onKey={handleMobileKey} />}

      {/* PWA Install Prompt (SRS §2.2: 2 visits + 5 min engagement) */}
      <InstallPrompt />
    </div>
    </AuthGate>
  );
}

export default App;
