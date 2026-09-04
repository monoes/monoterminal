import { forwardRef, useCallback, useEffect, useImperativeHandle, useMemo, useRef, useState } from 'react';
import { PaneGrid, useIsNarrow } from './PaneGrid';
import { ConnectionStatus } from './ConnectionStatus';
import { ConnectionState } from '../lib/websocket-client';
import type { TerminalTransport } from '../lib/transport';
import { createTransport } from '../lib/transport';
import type { LayoutUpdate } from '../lib/protocol';
import { collectPaneIds, protoLayoutToLayoutNode } from '../lib/layout-proto';
import type { ComputerConfig } from '../state/WorkspaceContext';
import { useWorkspace } from '../state/WorkspaceContext';
import type { TerminalHandle } from './Terminal';
import './WorkspaceSession.css';

/** A workspace's current panes, as last reported by the server — used by
 * the sidebar to list a workspace's live sessions (see App.tsx). */
export interface WorkspacePanes {
  paneIds: string[];
  focusedPaneId: string | null;
}

interface WorkspaceSessionProps {
  computer: ComputerConfig;
  /** Used to scope this workspace's client-side pane display names
   * (see WorkspaceContext's paneNames). */
  workspaceId: string;
  visible: boolean;
  /** Stable logical key (e.g. "computer/workspace") used to find-or-create
   * the backend's root session for this workspace, so the same workspace
   * opened from another browser/device/tab attaches to the same live
   * layout instead of spawning an independent one. */
  sessionKey: string;
  /** Called whenever the server reports a new layout (split/close/focus),
   * so the sidebar can show this workspace's current panes. */
  onPanesChange?: (panes: WorkspacePanes) => void;
}

export interface WorkspaceSessionHandle {
  /** Sends input to whichever pane the server currently has focused —
   * used by the on-screen mobile keyboard. */
  sendInputToFocused: (data: string) => void;
  /** Focuses a specific pane — used by the sidebar's pane list. */
  focusPane: (paneId: string) => void;
}

/**
 * Owns one connection for an entire workspace and renders every pane of its
 * server-owned split layout (Phase 4: Splits/Tabs, ADR-018) via PaneGrid.
 * Replaces the old one-connection-per-terminal TerminalSession — a
 * workspace with a single pane is just a layout with one leaf, so there's
 * no separate "unsplit" mode to special-case.
 */
export const WorkspaceSession = forwardRef<WorkspaceSessionHandle, WorkspaceSessionProps>(
  function WorkspaceSession({ computer, workspaceId, visible, sessionKey, onPanesChange }, ref) {
    const { paneNames, renamePane } = useWorkspace();
    const [connectionState, setConnectionState] = useState<ConnectionState>(
      ConnectionState.DISCONNECTED
    );
    // Read via a ref, not the `computer` prop directly, inside the
    // transport's route thunk (see createTransport's doc comment) — the
    // transport is built once and outlives any later identity update
    // (mergeComputers/adoptPeerId) to this same computer.
    const computerRef = useRef(computer);
    computerRef.current = computer;
    const [client] = useState<TerminalTransport>(() => createTransport(() => computerRef.current));
    const [layoutUpdate, setLayoutUpdate] = useState<LayoutUpdate | null>(null);
    const terminalRefs = useRef<Map<string, TerminalHandle | null>>(new Map());
    // Every pane's output, buffered client-side (capped) so it can be
    // replayed into a freshly-mounted xterm instance. Splitting or closing
    // ANY pane changes the tree shape, which can reposition an unrelated
    // sibling pane to a different depth/parent — React has no way to
    // preserve a component across that kind of move, so its Terminal
    // legitimately unmounts and remounts with a blank xterm. Without this
    // buffer that remount would look like the pane's text vanished.
    const scrollbackByPane = useRef<Map<string, string>>(new Map());
    const isNarrow = useIsNarrow();

    const MAX_BUFFERED_CHARS = 200_000;
    function appendScrollback(paneId: string, text: string) {
      const existing = scrollbackByPane.current.get(paneId) ?? '';
      const next = existing + text;
      scrollbackByPane.current.set(
        paneId,
        next.length > MAX_BUFFERED_CHARS ? next.slice(next.length - MAX_BUFFERED_CHARS) : next
      );
    }

    useImperativeHandle(
      ref,
      () => ({
        sendInputToFocused: (data: string) => {
          const focusedPaneId = layoutUpdate?.focusedPaneId;
          client.sendInput(data, focusedPaneId);
        },
        focusPane: (paneId: string) => client.focusPane(paneId),
      }),
      [client, layoutUpdate]
    );

    useEffect(() => {
      if (!layoutUpdate?.root) return;
      onPanesChange?.({
        paneIds: collectPaneIds(layoutUpdate.root),
        focusedPaneId: layoutUpdate.focusedPaneId,
      });
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [layoutUpdate]);

    useEffect(() => {
      client.setHandlers({
        onAttachResponse: (response) => {
          // Scrollback replay is for the root pane ("pane-0") only — panes
          // created later via split start with a fresh shell, same as
          // opening a new terminal.
          const decoder = new TextDecoder();
          const text = response.scrollback.map((line) => decoder.decode(line.data)).join('');
          appendScrollback('pane-0', text);
          terminalRefs.current.get('pane-0')?.write(text);

          // The daemon only pushes a LayoutUpdate in response to a pane
          // command (split/close/focus) — never proactively on attach. A
          // harmless re-focus of the already-focused root pane is the
          // cheapest way to fetch the initial layout tree; without this the
          // pane view never learns any layout at all and stays stuck on
          // "Connecting…" even though the socket is fully connected.
          client.focusPane('pane-0');
        },
        onOutputData: (data) => {
          const decoder = new TextDecoder();
          const paneId = data.paneId || 'pane-0';
          const text = decoder.decode(data.data);
          appendScrollback(paneId, text);
          terminalRefs.current.get(paneId)?.write(text);
        },
        onErrorResponse: (error) => {
          console.error('Server error:', error.code, error.message);
        },
        onLayoutUpdate: (update) => setLayoutUpdate(update),
      });
    }, [client]);

    // WorkspaceSession is mounted once per workspace (keyed by workspace.id
    // in App.tsx, never remounted just to switch which workspace is shown —
    // see that file's comment), so `sessionKey` only ever changes here when
    // this workspace or its owning computer gets renamed. Tracking the
    // value it changed FROM lets us tell the server "this is the same live
    // session, just renamed" (see AttachRequest.previous_session_name) —
    // without it, a rename reconnects under a brand-new name the server
    // has never seen, orphaning the old session and handing back an empty
    // fresh one in its place.
    const previousSessionKeyRef = useRef(sessionKey);

    useEffect(() => {
      const previousSessionKey = previousSessionKeyRef.current;
      previousSessionKeyRef.current = sessionKey;

      const unsubscribe = client.onStateChange((state) => {
        setConnectionState(state);
        if (state === ConnectionState.CONNECTED) {
          client.attach(
            '',
            24,
            80,
            sessionKey,
            previousSessionKey !== sessionKey ? previousSessionKey : undefined
          );
        }
      });

      client.connect();

      return () => {
        unsubscribe();
        client.disconnect();
      };
    }, [client, sessionKey]);

    const layoutNode = useMemo(
      () => (layoutUpdate?.root ? protoLayoutToLayoutNode(layoutUpdate.root) : null),
      [layoutUpdate]
    );

    // Stable across re-renders — same reasoning as PaneGrid's handleData/
    // handleResize memoization: PaneGrid wraps this in its own per-pane
    // useCallback keyed on this reference, so if THIS were a fresh inline
    // closure every render (e.g. every focus-click's LayoutUpdate), that
    // memoization would be defeated too, and React would detach+reattach
    // every Terminal's ref on every render — each reattach unconditionally
    // replays the pane's full buffered scrollback, duplicating everything
    // already on screen. Only refs are captured below, so no reactive
    // dependency is needed.
    const registerTerminalRef = useCallback((paneId: string, handle: TerminalHandle | null) => {
      terminalRefs.current.set(paneId, handle);
      // A freshly-mounted xterm (first attach, or a remount forced by the
      // pane moving to a new spot in the tree — see scrollbackByPane's doc
      // comment) starts blank; immediately replay whatever output this
      // pane has produced so far so it never looks like the text vanished.
      const buffered = handle ? scrollbackByPane.current.get(paneId) : undefined;
      if (handle && buffered) handle.write(buffered);
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    return (
      <div className="workspace-session" style={{ display: visible ? 'flex' : 'none' }}>
        {/* Quiet corner indicator, not permanent chrome — a healthy
            connection shouldn't visually compete with the pane header's own
            split/close buttons, which now occupy this same corner for a
            single-pane (unsplit) workspace. Only worth interrupting for
            when there's actually something to report. */}
        {connectionState !== ConnectionState.CONNECTED && (
          <div className="workspace-session-status">
            <ConnectionStatus state={connectionState} onReconnect={() => client.connect()} />
          </div>
        )}
        <div className="workspace-session-body">
          {layoutNode ? (
            <PaneGrid
              layout={layoutNode}
              client={client}
              focusedPaneId={layoutUpdate?.focusedPaneId ?? null}
              isNarrow={isNarrow}
              onFocus={(paneId) => client.focusPane(paneId)}
              onSplit={(paneId, dir) => client.splitPane(paneId, dir)}
              onClose={(paneId) => client.closePane(paneId)}
              getPaneName={(paneId) => paneNames[workspaceId]?.[paneId] ?? paneId}
              onRenamePane={(paneId, name) => renamePane(workspaceId, paneId, name)}
              onRatioChange={() => {
                /* Client-side only (see PaneGrid) — the protocol has no
                   ratio-persistence command yet (ADR-018 §5.3 defers this
                   to v1.1), so a dragged ratio doesn't survive the next
                   LayoutUpdate from an unrelated split/close. */
              }}
              registerTerminalRef={registerTerminalRef}
            />
          ) : (
            <div className="workspace-session-loading">Connecting…</div>
          )}
        </div>
      </div>
    );
  }
);
