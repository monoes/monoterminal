import { useState, useEffect, useCallback } from 'react';
import { Terminal } from './components/Terminal';
import { MobileKeyboard } from './components/MobileKeyboard';
import { MonomindPanel } from './components/MonomindPanel';
import { MonomindSuggestion } from './components/MonomindSuggestion';
import { ConnectionStatus } from './components/ConnectionStatus';
import { InstallPrompt } from './components/InstallPrompt';
import { WebSocketClient, ConnectionState, DetectionResponse } from './lib/websocket-client';
import './App.css';

// Detect if running on mobile
const isMobile = /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(
  navigator.userAgent
);

function App() {
  const [connectionState, setConnectionState] = useState<ConnectionState>(
    ConnectionState.DISCONNECTED
  );
  const [wsClient] = useState(
    () =>
      new WebSocketClient({
        url: import.meta.env.VITE_WS_URL || 'wss://localhost:5000',
        autoReconnect: true,
        reconnectInterval: 3000,
        maxReconnectAttempts: 5,
      })
  );
  const [showMonomindPanel, setShowMonomindPanel] = useState(false);
  const [sessionId, setSessionId] = useState<string>();
  const [detectionData, setDetectionData] = useState<DetectionResponse | null>(null);
  const [showSuggestion, setShowSuggestion] = useState(false);

  // Terminal data handler
  const handleTerminalData = useCallback(
    (data: string) => {
      wsClient.sendInput(data);
    },
    [wsClient]
  );

  // Handle terminal resize
  const handleTerminalResize = useCallback(
    (cols: number, rows: number) => {
      wsClient.resize(rows, cols);
    },
    [wsClient]
  );

  // Mobile keyboard key handler
  const handleMobileKey = useCallback(
    (key: string) => {
      handleTerminalData(key);
    },
    [handleTerminalData]
  );

  // Monomind suggestion handlers
  const handleSuggestionDismiss = () => {
    setShowSuggestion(false);
    // TODO: Send dismiss request to backend to create .monoterminal-dismiss file
    // This will be implemented when backend dismiss API is ready
  };

  const handleSuggestionOpenDashboard = () => {
    setShowSuggestion(false);
    setShowMonomindPanel(true);
  };

  // WebSocket message handlers
  useEffect(() => {
    wsClient.setHandlers({
      onAttachResponse: async (response) => {
        console.log('Attached to session:', response.sessionId);
        setSessionId(response.sessionId);

        // Render scrollback (last 10k lines per SRS §3.1.1)
        response.scrollback.forEach((line) => {
          const decoder = new TextDecoder();
          const text = decoder.decode(line.data);
          (window as any).terminal?.write(text);
        });

        // Per SRS §2.4.1: Run detection on session attach
        try {
          const detection = await wsClient.sendDetectionRequest({
            projectDir: response.metadata.workingDir || '',
          });
          setDetectionData(detection);

          // Show suggestion if found and not dismissed
          if (detection.found && detection.suggestInstall && !detection.dismissFileExists) {
            setShowSuggestion(true);
          }
        } catch (error) {
          console.error('Detection request failed:', error);
        }
      },
      onOutputData: (data) => {
        // Decode and write terminal output
        const decoder = new TextDecoder();
        const text = decoder.decode(data.data);
        (window as any).terminal?.write(text);
      },
      onErrorResponse: (error) => {
        console.error('Server error:', error.code, error.message);
        // TODO: Show error UI
      },
    });
  }, [wsClient]);

  // Connection state listener and auto-attach
  useEffect(() => {
    const unsubscribe = wsClient.onStateChange((state) => {
      setConnectionState(state);

      // Auto-attach to session when connected
      if (state === ConnectionState.CONNECTED) {
        // Get terminal dimensions
        const cols = (window as any).terminal?.cols || 80;
        const rows = (window as any).terminal?.rows || 24;

        // Attach to session (empty sessionId = create new)
        wsClient.attach('', rows, cols);
      }
    });

    // Auto-connect on mount
    wsClient.connect();

    return () => {
      unsubscribe();
      wsClient.disconnect();
    };
  }, [wsClient]);

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
    <div className="app">
      {/* Header with connection status and controls */}
      <header className="app-header">
        <div className="header-left">
          <h1>MONOTERMINAL</h1>
        </div>
        <div className="header-center">
          <ConnectionStatus state={connectionState} onReconnect={() => wsClient.connect()} />
        </div>
        <div className="header-right">
          <button
            className="panel-toggle-btn"
            onClick={() => setShowMonomindPanel(!showMonomindPanel)}
            aria-label="Toggle Monomind panel"
            title="Toggle Monomind panel (Ctrl+M)"
            data-testid="dashboard-toggle"
          >
            {showMonomindPanel ? '✗' : '☰'}
          </button>
        </div>
      </header>

      {/* Monomind detection suggestion banner */}
      {showSuggestion && detectionData && (
        <MonomindSuggestion
          bannerText={detectionData.bannerText || 'Monomind project detected!'}
          monomindRoot={detectionData.monomindRoot}
          onDismiss={handleSuggestionDismiss}
          onOpenDashboard={handleSuggestionOpenDashboard}
        />
      )}

      {/* Main terminal area */}
      <main className={`app-main ${showMonomindPanel ? 'panel-open' : ''}`}>
        <div className="terminal-container">
          <Terminal onData={handleTerminalData} onResize={handleTerminalResize} />
        </div>

        {/* Monomind panel */}
        <MonomindPanel
          sessionId={sessionId}
          isVisible={showMonomindPanel}
          onClose={() => setShowMonomindPanel(false)}
          wsClient={wsClient}
        />
      </main>

      {/* Mobile keyboard (hidden on desktop) */}
      {isMobile && <MobileKeyboard onKey={handleMobileKey} />}

      {/* PWA Install Prompt (SRS §2.2: 2 visits + 5 min engagement) */}
      <InstallPrompt />
    </div>
  );
}

export default App;
