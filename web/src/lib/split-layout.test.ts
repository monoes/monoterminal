import { describe, it, expect } from 'vitest';
import {
  makeLeaf,
  makeSplit,
  findLeaf,
  containsLeaf,
  firstLeafId,
  countLeaves,
  replaceLeaf,
  removeLeaf,
  setRatio,
} from './split-layout';

describe('split-layout', () => {
  it('finds a leaf by terminal id', () => {
    const tree = makeSplit('row', makeLeaf('a'), makeLeaf('b'));
    expect(findLeaf(tree, 'a')?.id).toBe('a');
    expect(findLeaf(tree, 'missing')).toBeNull();
  });

  it('reports whether a subtree contains a leaf', () => {
    const tree = makeSplit('row', makeLeaf('a'), makeSplit('col', makeLeaf('b'), makeLeaf('c')));
    expect(containsLeaf(tree, 'c')).toBe(true);
    expect(containsLeaf(tree, 'z')).toBe(false);
  });

  it('counts leaves and finds the first one', () => {
    const tree = makeSplit('row', makeLeaf('a'), makeSplit('col', makeLeaf('b'), makeLeaf('c')));
    expect(countLeaves(tree)).toBe(3);
    expect(firstLeafId(tree)).toBe('a');
  });

  it('splits a leaf into a new split node via replaceLeaf', () => {
    const tree = makeLeaf('a');
    const split = replaceLeaf(tree, 'a', (leaf) => makeSplit('row', leaf, makeLeaf('b')));
    expect(split.type).toBe('split');
    expect(countLeaves(split)).toBe(2);
    expect(findLeaf(split, 'a')).not.toBeNull();
    expect(findLeaf(split, 'b')).not.toBeNull();
  });

  it('removing a leaf collapses its parent split into the sibling', () => {
    const tree = makeSplit('row', makeLeaf('a'), makeLeaf('b'));
    const result = removeLeaf(tree, 'a');
    expect(result).toEqual(makeLeaf('b'));
  });

  it('removing the last leaf returns null', () => {
    expect(removeLeaf(makeLeaf('a'), 'a')).toBeNull();
  });

  it('removing a nested leaf collapses only its own parent, not the whole tree', () => {
    const tree = makeSplit('row', makeLeaf('a'), makeSplit('col', makeLeaf('b'), makeLeaf('c')));
    const result = removeLeaf(tree, 'b');
    expect(result?.type).toBe('split');
    expect(countLeaves(result!)).toBe(2);
    expect(findLeaf(result, 'a')).not.toBeNull();
    expect(findLeaf(result, 'c')).not.toBeNull();
    expect(findLeaf(result, 'b')).toBeNull();
  });

  it('removing a leaf not present in the tree is a no-op', () => {
    const tree = makeSplit('row', makeLeaf('a'), makeLeaf('b'));
    const result = removeLeaf(tree, 'z');
    expect(result).toEqual(tree);
  });

  it('sets the ratio on the matching split node only', () => {
    const inner = makeSplit('col', makeLeaf('b'), makeLeaf('c'), 0.5);
    const tree = makeSplit('row', makeLeaf('a'), inner, 0.5);
    const result = setRatio(tree, inner.id, 0.75);
    expect(result.type).toBe('split');
    if (result.type !== 'split') throw new Error('expected split');
    expect(result.ratio).toBe(0.5); // outer split untouched
    if (result.b.type !== 'split') throw new Error('expected nested split');
    expect(result.b.ratio).toBe(0.75);
  });
});
