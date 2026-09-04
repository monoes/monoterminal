import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { WebglAddon } from '@xterm/addon-webgl';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { Unicode11Addon } from '@xterm/addon-unicode11';
import '@xterm/xterm/css/xterm.css';
import './Terminal.css';
import { ClipboardHandler } from '../lib/clipboard-handler';
import type { ClipboardGetResponse, ShowPermissionModal } from '../lib/clipboard-handler';
import { ClipboardPermissionModal } from './ClipboardPermissionModal';
import { ClipboardPasteButton } from './ClipboardPasteButton';
import { ClipboardStatusIndicator } from './ClipboardStatusIndicator';

interface TerminalProps {
  onData?: (data: string) => void;
  onResize?: (cols: number, rows: number) => void;
  onClipboardResponse?: (response: ClipboardGetResponse) => void;
  sessionId?: string;
  /** Whether this terminal's container is currently shown (vs. display:none
   * while a sibling tab is active). xterm.js stops rendering while its
   * container has zero size and doesn't automatically resume on its own
   * when the container becomes visible again — fit() + refresh() must be
   * called explicitly, or the terminal reappears blank. */
  visible?: boolean;
}

export interface TerminalHandle {
  write: (data: string) => void;
  writeln: (data: string) => void;
  clear: () => void;
}

export const Terminal = forwardRef<TerminalHandle, TerminalProps>(function Terminal(
  { onData, onResize, onClipboardResponse, sessionId = 'default', visible = true },
  ref
) {
  const terminalRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const clipboardHandlerRef = useRef<ClipboardHandler | null>(null);
  const clipboardResolveRef = useRef<((granted: boolean) => void) | null>(null); // Security fix: no window global
  const resizeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [useWebGL, setUseWebGL] = useState(true);

  // Clipboard state (ADR-020)
  const [showPermissionModal, setShowPermissionModal] = useState(false);
  const [currentClipboardRequest, setCurrentClipboardRequest] = useState<string | null>(null);
  const [clipboardAccessActive, setClipboardAccessActive] = useState(false);
  const [lastClipboardActivity, setLastClipboardActivity] = useState(0);

  // Platform detection
  const capabilities = ClipboardHandler.getCapabilities();
  const useIOSPasteButton = capabilities.isIOSSafari && capabilities.requiresGesture;

  useEffect(() => {
    if (!terminalRef.current) return;

    // Initialize xterm.js with WebGL addon
    const term = new XTerm({
      cursorBlink: true,
      // `ui-monospace` resolves to the OS's native terminal/UI monospace
      // font (SF Mono on macOS, Cascadia Mono on Windows 11, etc.) — the
      // previous stack (`Consolas, "Courier New"`) only had a real
      // monospace font on Windows; everywhere else it fell straight
      // through to Courier New, a typewriter-style serif face nothing
      // like what any actual terminal emulator renders with. Named
      // fallbacks cover the platforms/browsers where `ui-monospace` isn't
      // supported yet (older Firefox, some Linux configs).
      fontFamily:
        'ui-monospace, Menlo, Monaco, "Cascadia Mono", Consolas, "SF Mono", ' +
        '"DejaVu Sans Mono", "Liberation Mono", monospace',
      fontSize: 14,
      lineHeight: 1.2,
      theme: {
        background: '#1e1e1e',
        foreground: '#d4d4d4',
        cursor: '#d4d4d4',
        black: '#000000',
        red: '#cd3131',
        green: '#0dbc79',
        yellow: '#e5e510',
        blue: '#2472c8',
        magenta: '#bc3fbc',
        cyan: '#11a8cd',
        white: '#e5e5e5',
        brightBlack: '#666666',
        brightRed: '#f14c4c',
        brightGreen: '#23d18b',
        brightYellow: '#f5f543',
        brightBlue: '#3b8eea',
        brightMagenta: '#d670d6',
        brightCyan: '#29b8db',
        brightWhite: '#ffffff',
      },
      scrollback: 10000,
      allowProposedApi: true,
    });

    // Add fit addon
    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    fitAddonRef.current = fitAddon;

    // Add web links addon
    term.loadAddon(new WebLinksAddon());

    // xterm.js's built-in character-width table only covers Unicode 6 —
    // without this, wide characters (CJK, most emoji) measure as 1 column
    // instead of 2, so the cursor drifts out of alignment with whatever
    // the shell/TUI app actually drew the moment any such character
    // appears. Every real terminal emulator gets this right natively.
    const unicode11Addon = new Unicode11Addon();
    term.loadAddon(unicode11Addon);
    term.unicode.activeVersion = '11';

    // Try to load WebGL addon
    if (useWebGL) {
      try {
        const webglAddon = new WebglAddon();
        webglAddon.onContextLoss(() => {
          console.warn('WebGL context lost, falling back to Canvas');
          setUseWebGL(false);
        });
        term.loadAddon(webglAddon);
      } catch (e) {
        console.warn('WebGL not supported, using Canvas fallback', e);
        setUseWebGL(false);
      }
    }

    // Initialize clipboard handler (ADR-020)
    const showPermissionModalFn: ShowPermissionModal = (sessionId: string) => {
      return new Promise((resolve) => {
        clipboardResolveRef.current = resolve; // Security fix: use ref instead of window global
        setCurrentClipboardRequest(sessionId);
        setShowPermissionModal(true);
      });
    };

    const handleClipboardResponse = (response: ClipboardGetResponse) => {
      if (onClipboardResponse) {
        onClipboardResponse(response);
      }
      setLastClipboardActivity(Date.now());
    };

    const clipboardHandler = new ClipboardHandler({
      showPermissionModal: showPermissionModalFn,
      onClipboardResponse: handleClipboardResponse,
      sessionId,
    });
    clipboardHandlerRef.current = clipboardHandler;

    // Register OSC 52 handler (ADR-020 §2.2)
    // Security control #2: Distinguish server-initiated from user-initiated
    term.parser.registerOscHandler(52, (data: string) => {
      const parts = data.split(';');
      if (parts.length < 2) return false;

      const selection = parts[0]; // "c" = clipboard, "p" = primary
      const content = parts[1];

      if (content === '?') {
        // OSC 52 query: Server requests clipboard (server-initiated READ)
        // Security control #2: Requires modal authorization
        const requestId = `osc52-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
        clipboardHandler.handleClipboardGetRequest(requestId);
        return true; // Handled
      } else {
        // OSC 52 set: Server sets clipboard (server-initiated WRITE)
        // Security control: Browser native permission (implicit)
        try {
          const decoded = atob(content); // Can throw on invalid base64
          clipboardHandler.handleOSC52Write(decoded, selection);
          return true; // Handled successfully
        } catch (err) {
          console.error('[Terminal] OSC 52 invalid base64:', err);
          return false; // Not handled (malformed)
        }
      }
    });

    // Open terminal
    term.open(terminalRef.current);
    xtermRef.current = term;

    // Initial fit
    fitAddon.fit();

    // Re-fit whenever this terminal's own container changes size — not just
    // on window resize. Dragging a split divider or splitting/closing a
    // sibling pane resizes this container without ever firing a window
    // resize event, so xterm would otherwise keep rendering at stale
    // rows/cols after a layout change.
    const resizeObserver = new ResizeObserver(() => {
      fitAddon.fit();
    });
    if (terminalRef.current) resizeObserver.observe(terminalRef.current);

    // Handle data input
    if (onData) {
      term.onData(onData);
    }

    // BEL (Ctrl-G / \x07) was previously a silent no-op — bellStyle
    // defaults to 'none'. Real terminals surface it somehow; a brief
    // visual flash (matching iTerm2/Windows Terminal's default "visual
    // bell") is the least surprising choice for a browser tab that may
    // not have unmuted audio or user-gesture permission to play a sound.
    const bellDiv = terminalRef.current;
    term.onBell(() => {
      bellDiv?.classList.add('terminal-bell-flash');
      setTimeout(() => bellDiv?.classList.remove('terminal-bell-flash'), 150);
    });

    // Handle resize — debounced. A split-divider drag or a sibling pane
    // splitting/closing can push through dozens of intermediate cols/rows
    // values as the container's size settles (one per animation frame while
    // dragging). Forwarding every one of those to the PTY makes a real TUI
    // app (e.g. an ncurses-style program) repaint its full screen on every
    // single intermediate size, and those repaints race with each other and
    // with buffered output — that's the "everything goes nuts" garbling.
    // Real terminals coalesce this the same way: only the final settled
    // size after the resize burst ends actually reaches the child process.
    if (onResize) {
      term.onResize(({ cols, rows }) => {
        if (resizeTimerRef.current) clearTimeout(resizeTimerRef.current);
        resizeTimerRef.current = setTimeout(() => onResize(cols, rows), 120);
      });
    }

    // Handle window resize
    const handleResize = () => {
      fitAddon.fit();
    };

    window.addEventListener('resize', handleResize);

    // Handle orientation change on mobile
    window.addEventListener('orientationchange', () => {
      setTimeout(handleResize, 100);
    });

    return () => {
      window.removeEventListener('resize', handleResize);
      resizeObserver.disconnect();
      if (resizeTimerRef.current) clearTimeout(resizeTimerRef.current);
      clipboardHandler.dispose();
      term.dispose();
    };
  }, [onData, onResize, onClipboardResponse, sessionId, useWebGL]);

  // Re-fit and force a repaint whenever this terminal becomes visible again
  // (e.g. switching back to this tab in a multi-terminal layout). Without
  // this, xterm.js stays blank: its renderer stops updating once the
  // container hits zero size under display:none and doesn't resume on its
  // own just because the container is visible again.
  useEffect(() => {
    if (!visible) return;
    const raf = requestAnimationFrame(() => {
      fitAddonRef.current?.fit();
      xtermRef.current?.refresh(0, (xtermRef.current.rows || 1) - 1);
    });
    return () => cancelAnimationFrame(raf);
  }, [visible]);

  // Monitor clipboard access status
  useEffect(() => {
    const interval = setInterval(() => {
      if (clipboardHandlerRef.current) {
        const status = clipboardHandlerRef.current.getSessionRememberStatus();
        setClipboardAccessActive(status.active);
      }
    }, 10000); // Check every 10 seconds

    return () => clearInterval(interval);
  }, []);

  // Clipboard permission modal handlers
  const handleClipboardAllow = (remember: boolean) => {
    if (clipboardHandlerRef.current && remember) {
      clipboardHandlerRef.current.enableSessionRemember();
      setClipboardAccessActive(true);
    }
    setShowPermissionModal(false);

    // Resolve the permission promise (Security fix: use ref instead of window global)
    if (clipboardResolveRef.current) {
      clipboardResolveRef.current(true);
      clipboardResolveRef.current = null;
    }
  };

  const handleClipboardDeny = () => {
    setShowPermissionModal(false);

    // Resolve the permission promise with false (Security fix: use ref instead of window global)
    if (clipboardResolveRef.current) {
      clipboardResolveRef.current(false);
      clipboardResolveRef.current = null;
    }
  };

  const handleClipboardRevoke = () => {
    if (clipboardHandlerRef.current) {
      clipboardHandlerRef.current.revokeSessionRemember();
      setClipboardAccessActive(false);
    }
  };

  // iOS Safari paste button handlers
  const handleIOSPaste = (requestId: string, content: string | null) => {
    if (content !== null && clipboardHandlerRef.current) {
      // Send clipboard content response
      if (onClipboardResponse) {
        onClipboardResponse({
          requestId,
          content,
          mimeType: 'text/plain',
          authorized: true,
        });
      }
    }
    setShowPermissionModal(false);
    setCurrentClipboardRequest(null);
  };

  const handleIOSDeny = (requestId: string) => {
    if (onClipboardResponse) {
      onClipboardResponse({
        requestId,
        content: '',
        mimeType: 'text/plain',
        authorized: false,
        error: 'User denied clipboard access',
      });
    }
    setShowPermissionModal(false);
    setCurrentClipboardRequest(null);
  };

  // Get frequency warning for modal
  const frequencyWarning = clipboardHandlerRef.current
    ? clipboardHandlerRef.current.getFrequencyWarning()
    : { shouldWarn: false, requestCount: 0 };

  // Public API for writing to terminal — exposed via ref, not a global, so
  // multiple simultaneously-mounted Terminal instances (multi-terminal UI)
  // don't clobber each other's write() target.
  useImperativeHandle(
    ref,
    () => ({
      write: (data: string) => xtermRef.current?.write(data),
      writeln: (data: string) => xtermRef.current?.writeln(data),
      clear: () => xtermRef.current?.clear(),
    }),
    []
  );


  return (
    <>
      {/* Clipboard status indicator (Security control #3) */}
      {clipboardAccessActive && (
        <ClipboardStatusIndicator
          active={clipboardAccessActive}
          lastActivityMs={Date.now() - lastClipboardActivity}
          onRevoke={handleClipboardRevoke}
        />
      )}

      {/* Terminal container */}
      <div
        ref={terminalRef}
        style={{
          width: '100%',
          height: '100%',
          padding: '4px',
          backgroundColor: '#1e1e1e',
        }}
      />

      {/* Clipboard permission modals (ADR-020) */}
      {showPermissionModal && !useIOSPasteButton && (
        <ClipboardPermissionModal
          sessionId={sessionId}
          frequencyWarning={frequencyWarning}
          onAllow={handleClipboardAllow}
          onDeny={handleClipboardDeny}
          visible={showPermissionModal}
        />
      )}

      {/* iOS Safari paste button (gesture workaround) */}
      {showPermissionModal && useIOSPasteButton && currentClipboardRequest && (
        <ClipboardPasteButton
          requestId={currentClipboardRequest}
          sessionId={sessionId}
          onPaste={handleIOSPaste}
          onDeny={handleIOSDeny}
        />
      )}
    </>
  );
});
