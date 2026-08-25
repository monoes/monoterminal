import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { AuthGate } from './components/AuthGate';
import { Sidebar } from './components/Sidebar';
import { WorkspaceSession } from './components/WorkspaceSession';
import type { WorkspacePanes, WorkspaceSessionHandle } from './components/WorkspaceSession';
import { MobileKeyboard } from './components/MobileKeyboard';
import { MonomindPanel } from './components/MonomindPanel';
import { InstallPrompt } from './components/InstallPrompt';
import { IconClose, IconMenu, IconPanel, IconPrompt } from './components/icons';
import { WebSocketClient } from './lib/websocket-client';
import { useWorkspace } from './state/WorkspaceContext';
import './App.css';

// Detect if running on mobile
const isMobile = /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(
  navigator.userAgent
);

function App() {
  const { computers, workspaces, activeWorkspaceId } = useWorkspace();
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

  useEffect(() => {
    if (!activeComputer) return;
    const client = new WebSocketClient({
      url: activeComputer.wsUrl,
      autoReconnect: true,
      reconnectInterval: 3000,
      maxReconnectAttempts: 5,
    });
    client.connect();
    setMonomindClient(client);
    return () => client.disconnect();
  }, [activeComputer]);

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
