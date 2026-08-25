import { useCallback, useEffect, useRef, useState } from 'react';
import type { LayoutNode } from '../lib/split-layout';
import { containsLeaf } from '../lib/split-layout';
import type { TerminalTransport } from '../lib/transport';
import { Terminal } from './Terminal';
import type { TerminalHandle } from './Terminal';
import { IconClose, IconSplitDown, IconSplitRight } from './icons';
import './PaneGrid.css';

interface SharedProps {
  client: TerminalTransport;
  focusedPaneId: string | null;
  isNarrow: boolean;
  onFocus: (paneId: string) => void;
  onSplit: (paneId: string, dir: 'row' | 'col') => void;
  onClose: (paneId: string) => void;
  onRatioChange: (splitId: string, ratio: number) => void;
  registerTerminalRef: (paneId: string, handle: TerminalHandle | null) => void;
  /** Resolves a pane's display label — client-side only (see
   * WorkspaceContext's paneNames); the underlying pane id never changes. */
  getPaneName: (paneId: string) => string;
  onRenamePane: (paneId: string, name: string) => void;
}

interface PaneGridProps extends SharedProps {
  layout: LayoutNode;
}

/** True below the app's existing mobile breakpoint (see App.css) — panes
 * collapse to a single focused one there instead of tiling, since a phone
 * screen can't usefully show a split. */
function useIsNarrow(): boolean {
  const [isNarrow, setIsNarrow] = useState(() => window.matchMedia('(max-width: 768px)').matches);
  useEffect(() => {
    const mql = window.matchMedia('(max-width: 768px)');
    const handler = (e: MediaQueryListEvent) => setIsNarrow(e.matches);
    mql.addEventListener('change', handler);
    return () => mql.removeEventListener('change', handler);
  }, []);
  return isNarrow;
}

export function PaneGrid(props: PaneGridProps) {
  const { layout, ...rest } = props;
  return (
    <div className="pane-host">
      <LayoutNodeView node={layout} {...rest} />
    </div>
  );
}

function LayoutNodeView({ node, ...rest }: { node: LayoutNode } & SharedProps) {
  return node.type === 'leaf' ? (
    <PaneLeaf paneId={node.id} {...rest} />
  ) : (
    <SplitView node={node} {...rest} />
  );
}

function SplitView({
  node,
  ...rest
}: { node: Extract<LayoutNode, { type: 'split' }> } & SharedProps) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const aRef = useRef<HTMLDivElement>(null);
  const bRef = useRef<HTMLDivElement>(null);
  const ratioRef = useRef(node.ratio);
  ratioRef.current = node.ratio;

  function onPointerDown(e: React.PointerEvent<HTMLDivElement>) {
    e.preventDefault();
    const resizer = e.currentTarget;
    resizer.setPointerCapture(e.pointerId);
    resizer.classList.add('dragging');

    const isRow = node.dir === 'row';
    const rect = wrapRef.current!.getBoundingClientRect();
    const size = isRow ? rect.width : rect.height;
    const startPos = isRow ? e.clientX : e.clientY;
    const startRatio = ratioRef.current;

    function onMove(ev: PointerEvent) {
      const pos = isRow ? ev.clientX : ev.clientY;
      const delta = (pos - startPos) / size;
      const ratio = Math.min(0.85, Math.max(0.15, startRatio + delta));
      ratioRef.current = ratio;
      if (aRef.current) aRef.current.style.flex = `${ratio} 1 0%`;
      if (bRef.current) bRef.current.style.flex = `${1 - ratio} 1 0%`;
    }
    function onUp(ev: PointerEvent) {
      onMove(ev);
      resizer.classList.remove('dragging');
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      rest.onRatioChange(node.id, ratioRef.current);
    }
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }

  const focusedId = rest.focusedPaneId;
  const aHasFocus = focusedId != null && containsLeaf(node.a, focusedId);
  const bHasFocus = focusedId != null && containsLeaf(node.b, focusedId);

  return (
    <div className={`split-wrap split-${node.dir}`} ref={wrapRef}>
      <div
        className="split-child"
        ref={aRef}
        style={{ flex: `${node.ratio} 1 0%` }}
        data-contains-focus={aHasFocus}
      >
        <LayoutNodeView node={node.a} {...rest} />
      </div>
      <div
        className={`resizer ${node.dir === 'row' ? 'resizer-v' : 'resizer-h'}`}
        onPointerDown={onPointerDown}
      />
      <div
        className="split-child"
        ref={bRef}
        style={{ flex: `${1 - node.ratio} 1 0%` }}
        data-contains-focus={bHasFocus}
      >
        <LayoutNodeView node={node.b} {...rest} />
      </div>
    </div>
  );
}

function PaneLeaf({
  paneId,
  client,
  focusedPaneId,
  isNarrow,
  onFocus,
  onSplit,
  onClose,
  registerTerminalRef,
  getPaneName,
  onRenamePane,
}: { paneId: string } & SharedProps) {
  const isFocused = paneId === focusedPaneId;
  const [isRenaming, setIsRenaming] = useState(false);
  const [nameDraft, setNameDraft] = useState('');

  // Stable across re-renders (e.g. a focus click causing a fresh
  // LayoutUpdate) — Terminal's xterm-init effect depends on these by
  // reference, so a new closure every render tore down and rebuilt the
  // whole terminal (wiping content, losing keyboard focus) on every click.
  const handleData = useCallback((data: string) => client.sendInput(data, paneId), [client, paneId]);
  const handleResize = useCallback(
    (cols: number, rows: number) => client.resize(rows, cols, paneId),
    [client, paneId]
  );

  function startRenaming() {
    setNameDraft(getPaneName(paneId));
    setIsRenaming(true);
  }

  function commitRename() {
    const trimmed = nameDraft.trim();
    if (trimmed) onRenamePane(paneId, trimmed);
    setIsRenaming(false);
  }

  return (
    <div
      className={`pane ${isFocused ? 'is-focused' : ''}`}
      onMouseDownCapture={() => onFocus(paneId)}
    >
      <div className={`pane-head ${isFocused ? 'focused' : ''}`}>
        {isRenaming ? (
          <input
            autoFocus
            className="pane-title-input"
            value={nameDraft}
            onClick={(e) => e.stopPropagation()}
            onMouseDown={(e) => e.stopPropagation()}
            onChange={(e) => setNameDraft(e.target.value)}
            onBlur={commitRename}
            onKeyDown={(e) => {
              if (e.key === 'Enter') commitRename();
              else if (e.key === 'Escape') setIsRenaming(false);
            }}
          />
        ) : (
          <span
            className="pane-title"
            onDoubleClick={(e) => {
              e.stopPropagation();
              startRenaming();
            }}
            title="Double-click to rename"
          >
            {getPaneName(paneId)}
          </span>
        )}
        <div className="pane-actions">
          <button
            className="pane-btn split-btn"
            title="Split right"
            aria-label={`Split ${paneId} right`}
            onClick={(e) => {
              e.stopPropagation();
              onSplit(paneId, 'row');
            }}
          >
            <IconSplitRight width={13} height={13} />
          </button>
          <button
            className="pane-btn split-btn"
            title="Split down"
            aria-label={`Split ${paneId} down`}
            onClick={(e) => {
              e.stopPropagation();
              onSplit(paneId, 'col');
            }}
          >
            <IconSplitDown width={13} height={13} />
          </button>
          <button
            className="pane-btn danger"
            title="Close pane"
            aria-label={`Close ${paneId}`}
            onClick={(e) => {
              e.stopPropagation();
              onClose(paneId);
            }}
          >
            <IconClose width={11} height={11} />
          </button>
        </div>
      </div>
      <div className="pane-body">
        <Terminal
          ref={(handle) => registerTerminalRef(paneId, handle)}
          onData={handleData}
          onResize={handleResize}
          visible={!isNarrow || isFocused}
        />
      </div>
    </div>
  );
}

export { useIsNarrow };
