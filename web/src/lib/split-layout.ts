/**
 * Pure data model for a tmux/VS Code-style binary split-pane tree. A
 * workspace's terminals are arranged as either a single leaf (one terminal)
 * or a split of two subtrees divided 'row' (side by side) or 'col' (stacked),
 * each with an adjustable size ratio. No React/DOM here — WorkspaceContext
 * owns one LayoutNode per workspace, PaneGrid renders it.
 */

export type SplitDirection = 'row' | 'col';

export interface LeafNode {
  type: 'leaf';
  /** A leaf's id IS the terminal's id — no separate identity to keep in sync. */
  id: string;
}

export interface SplitNode {
  type: 'split';
  id: string;
  dir: SplitDirection;
  /** Fraction of space given to `a`; `b` gets the remainder. */
  ratio: number;
  a: LayoutNode;
  b: LayoutNode;
}

export type LayoutNode = LeafNode | SplitNode;

let counter = 0;
function nodeId(): string {
  counter += 1;
  return `split-${counter}-${Math.random().toString(36).slice(2, 7)}`;
}

export function makeLeaf(terminalId: string): LeafNode {
  return { type: 'leaf', id: terminalId };
}

export function makeSplit(dir: SplitDirection, a: LayoutNode, b: LayoutNode, ratio = 0.5): SplitNode {
  return { type: 'split', id: nodeId(), dir, ratio, a, b };
}

export function findLeaf(node: LayoutNode | null, terminalId: string): LeafNode | null {
  if (!node) return null;
  if (node.type === 'leaf') return node.id === terminalId ? node : null;
  return findLeaf(node.a, terminalId) ?? findLeaf(node.b, terminalId);
}

export function containsLeaf(node: LayoutNode, terminalId: string): boolean {
  return node.type === 'leaf' ? node.id === terminalId : containsLeaf(node.a, terminalId) || containsLeaf(node.b, terminalId);
}

export function firstLeafId(node: LayoutNode): string {
  return node.type === 'leaf' ? node.id : firstLeafId(node.a);
}

export function countLeaves(node: LayoutNode): number {
  return node.type === 'leaf' ? 1 : countLeaves(node.a) + countLeaves(node.b);
}

/** Replaces the leaf with id `terminalId` using `fn`, which typically turns
 * it into a new split (e.g. splitting a pane). No-op if not found. */
export function replaceLeaf(node: LayoutNode, terminalId: string, fn: (leaf: LeafNode) => LayoutNode): LayoutNode {
  if (node.type === 'leaf') return node.id === terminalId ? fn(node) : node;
  return { ...node, a: replaceLeaf(node.a, terminalId, fn), b: replaceLeaf(node.b, terminalId, fn) };
}

/** Removes the leaf with id `terminalId`, collapsing its parent split into
 * whichever sibling remains. Returns null if the whole tree was that leaf. */
export function removeLeaf(node: LayoutNode, terminalId: string): LayoutNode | null {
  if (node.type === 'leaf') return node.id === terminalId ? null : node;
  const a = removeLeaf(node.a, terminalId);
  const b = removeLeaf(node.b, terminalId);
  if (a === null) return b;
  if (b === null) return a;
  if (a === node.a && b === node.b) return node;
  return { ...node, a, b };
}

export function setRatio(node: LayoutNode, splitId: string, ratio: number): LayoutNode {
  if (node.type === 'leaf') return node;
  if (node.id === splitId) return { ...node, ratio };
  return { ...node, a: setRatio(node.a, splitId, ratio), b: setRatio(node.b, splitId, ratio) };
}
