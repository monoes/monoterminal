/**
 * Converts the server's PaneLayout tree (protocol.ts's PaneLayoutNode,
 * decoded from a LayoutUpdate) into the client's generic split-pane model
 * (split-layout.ts's LayoutNode) that PaneGrid renders.
 *
 * The backend (crates/master/src/layout/mod.rs) always produces exactly two
 * children per split — split_pane() only ever replaces one leaf with
 * [old, new] — so this assumes that invariant rather than handling an
 * arbitrary-arity split.
 */

import type { PaneLayoutNode, SplitDirection } from './protocol';
import type { LayoutNode } from './split-layout';
import { makeLeaf, makeSplit } from './split-layout';

export function protoLayoutToLayoutNode(node: PaneLayoutNode): LayoutNode {
  if ('terminal' in node && node.terminal) {
    return makeLeaf(node.terminal.paneId);
  }

  const split = (node as Extract<PaneLayoutNode, { split: unknown }>).split;
  const dir: SplitDirection = split.direction === 0 ? 'row' : 'col';
  const a = protoLayoutToLayoutNode(split.children[0]);
  const b = protoLayoutToLayoutNode(split.children[1]);
  const ratio = split.ratios[0] ?? 0.5;
  return makeSplit(dir, a, b, ratio);
}

/** Every pane id in a layout tree, in tree order — used to list a
 * workspace's current panes (e.g. in the sidebar) independent of the
 * split-tree shape. */
export function collectPaneIds(node: PaneLayoutNode): string[] {
  if ('terminal' in node && node.terminal) {
    return [node.terminal.paneId];
  }
  const split = (node as Extract<PaneLayoutNode, { split: unknown }>).split;
  return split.children.flatMap(collectPaneIds);
}
