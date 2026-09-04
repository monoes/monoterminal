/**
 * WorkspaceContext tests — migration (pre-identity-unification persisted
 * state must keep working unchanged) and the new merge/adopt primitives
 * that fold a locally-detected daemon into its account-synced identity.
 */

import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  WorkspaceProvider,
  isSameHostPort,
  resolveRoutes,
  useWorkspace,
  type ComputerConfig,
} from './WorkspaceContext';
import * as localDaemon from '../lib/local-daemon';

const STORAGE_KEY = 'monoterminal.workspaces.v1';

function seedStorage(blob: unknown) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(blob));
}

function renderWorkspace() {
  return renderHook(() => useWorkspace(), { wrapper: WorkspaceProvider });
}

beforeEach(() => {
  localStorage.clear();
});

describe('resolveRoutes', () => {
  it('yields a single ws route for a direct-only computer, unaffected by identity unification', () => {
    const computer: ComputerConfig = { id: 'c1', name: 'This Machine', wsUrl: 'wss://localhost:54321' };
    expect(resolveRoutes(computer, null)).toEqual([{ kind: 'ws', url: 'wss://localhost:54321' }]);
  });

  it('yields a single p2p route for a p2p-only computer with no local match', () => {
    const computer: ComputerConfig = { id: 'c1', name: 'Studio', peerId: 'abc', relayUrl: 'wss://relay' };
    expect(resolveRoutes(computer, null)).toEqual([{ kind: 'p2p', peerId: 'abc', relayUrl: 'wss://relay' }]);
  });

  it('prefers the local route when the probe matches this computer\'s peerId', () => {
    const computer: ComputerConfig = { id: 'c1', name: 'Studio', peerId: 'abc', relayUrl: 'wss://relay' };
    const routes = resolveRoutes(computer, { peerId: 'abc', url: 'wss://localhost:54321' });
    expect(routes[0]).toEqual({ kind: 'ws', url: 'wss://localhost:54321' });
    expect(routes).toContainEqual({ kind: 'p2p', peerId: 'abc', relayUrl: 'wss://relay' });
  });

  it('does not add a local route when the probe peerId does not match', () => {
    const computer: ComputerConfig = { id: 'c1', name: 'Studio', peerId: 'abc', relayUrl: 'wss://relay' };
    const routes = resolveRoutes(computer, { peerId: 'different', url: 'wss://localhost:54321' });
    expect(routes).toEqual([{ kind: 'p2p', peerId: 'abc', relayUrl: 'wss://relay' }]);
  });
});

describe('isSameHostPort', () => {
  it('treats localhost and 127.0.0.1 as equivalent', () => {
    expect(isSameHostPort('wss://localhost:54321', 'wss://127.0.0.1:54321')).toBe(true);
  });

  it('rejects a different port', () => {
    expect(isSameHostPort('wss://localhost:54321', 'wss://localhost:9999')).toBe(false);
  });

  it('rejects a LAN IP as not provably the same machine', () => {
    expect(isSameHostPort('wss://localhost:54321', 'wss://192.168.1.5:54321')).toBe(false);
  });
});

describe('loadState migration', () => {
  it('preserves route output for a pre-v2 direct-only record', () => {
    seedStorage({
      computers: [{ id: 'c1', name: 'This Machine', mode: 'direct', wsUrl: 'wss://localhost:54321' }],
      workspaces: [{ id: 'w1', computerId: 'c1', name: 'Default' }],
      activeComputerId: 'c1',
      activeWorkspaceId: 'w1',
      paneNames: {},
    });

    const { result } = renderWorkspace();
    expect(result.current.computers).toHaveLength(1);
    expect(resolveRoutes(result.current.computers[0], null)).toEqual([
      { kind: 'ws', url: 'wss://localhost:54321' },
    ]);
  });

  it('normalizes a pre-v2 p2p record\'s wsUrl:"" and preserves its single p2p route', () => {
    seedStorage({
      computers: [
        { id: 'c1', name: 'Studio', mode: 'p2p', wsUrl: '', peerId: 'abc', relayUrl: 'wss://relay', serverId: 5 },
      ],
      workspaces: [{ id: 'w1', computerId: 'c1', name: 'Default' }],
      activeComputerId: 'c1',
      activeWorkspaceId: 'w1',
      paneNames: {},
    });

    const { result } = renderWorkspace();
    const computer = result.current.computers[0];
    expect(computer.wsUrl).toBeUndefined();
    expect(resolveRoutes(computer, null)).toEqual([{ kind: 'p2p', peerId: 'abc', relayUrl: 'wss://relay' }]);
  });

  it('writes a one-time backup snapshot of the pre-migration blob', () => {
    const raw = {
      computers: [{ id: 'c1', name: 'This Machine', mode: 'direct', wsUrl: 'wss://localhost:54321' }],
      workspaces: [{ id: 'w1', computerId: 'c1', name: 'Default' }],
      activeComputerId: 'c1',
      activeWorkspaceId: 'w1',
      paneNames: {},
    };
    seedStorage(raw);
    renderWorkspace();
    expect(JSON.parse(localStorage.getItem(`${STORAGE_KEY}.backup`)!)).toEqual(raw);
  });
});

describe('mergeComputers', () => {
  function seedTwoComputers() {
    seedStorage({
      version: 2,
      computers: [
        { id: 'direct-1', name: 'This Machine', wsUrl: 'wss://localhost:54321' },
        { id: 'p2p-1', name: 'Studio', peerId: 'abc', relayUrl: 'wss://relay', serverId: 5 },
      ],
      workspaces: [
        { id: 'w-direct-default', computerId: 'direct-1', name: 'Default' },
        { id: 'w-p2p-default', computerId: 'p2p-1', name: 'Default', serverId: 100 },
        { id: 'w-p2p-work', computerId: 'p2p-1', name: 'Work', serverId: 101 },
      ],
      activeComputerId: 'direct-1',
      activeWorkspaceId: 'w-direct-default',
      paneNames: { 'w-direct-default': { 'pane-0': 'shell' } },
    });
  }

  it('combines a direct-localhost entry and its p2p-linked counterpart into one, server name winning', () => {
    seedTwoComputers();
    const { result } = renderWorkspace();

    act(() => result.current.mergeComputers('p2p-1', 'direct-1'));

    expect(result.current.computers).toHaveLength(1);
    const merged = result.current.computers[0];
    expect(merged.id).toBe('p2p-1');
    expect(merged.name).toBe('Studio'); // server name wins — survivor has a serverId
    expect(merged.peerId).toBe('abc');
    expect(merged.wsUrl).toBe('wss://localhost:54321'); // adopted from the absorbed direct entry
  });

  it('reparents workspaces from the absorbed computer, deduping same-named pairs, preserving the kept id', () => {
    seedTwoComputers();
    const { result } = renderWorkspace();

    act(() => result.current.mergeComputers('p2p-1', 'direct-1'));

    const survivorWorkspaces = result.current.workspaces.filter((w) => w.computerId === 'p2p-1');
    // "Default" existed on both sides — deduped to one, the serverId-bearing one.
    const defaults = survivorWorkspaces.filter((w) => w.name === 'Default');
    expect(defaults).toHaveLength(1);
    expect(defaults[0].id).toBe('w-p2p-default');
    expect(defaults[0].serverId).toBe(100);
    // "Work" only existed on the p2p side — untouched.
    expect(survivorWorkspaces.some((w) => w.id === 'w-p2p-work')).toBe(true);
    // No workspace still points at the removed computer.
    expect(result.current.workspaces.some((w) => w.computerId === 'direct-1')).toBe(false);
  });

  it('moves paneNames from a dropped duplicate workspace to the kept one', () => {
    seedTwoComputers();
    const { result } = renderWorkspace();

    act(() => result.current.mergeComputers('p2p-1', 'direct-1'));

    expect(result.current.paneNames['w-direct-default']).toBeUndefined();
    expect(result.current.paneNames['w-p2p-default']).toEqual({ 'pane-0': 'shell' });
  });

  it('updates activeComputerId/activeWorkspaceId when they pointed at the absorbed side', () => {
    seedTwoComputers();
    const { result } = renderWorkspace();

    act(() => result.current.mergeComputers('p2p-1', 'direct-1'));

    expect(result.current.activeComputerId).toBe('p2p-1');
    expect(result.current.activeWorkspaceId).toBe('w-p2p-default');
  });

  it('refuses to merge when both computers have a different serverId', () => {
    seedStorage({
      version: 2,
      computers: [
        { id: 'a', name: 'A', peerId: 'peer-a', relayUrl: 'wss://relay', serverId: 1 },
        { id: 'b', name: 'B', peerId: 'peer-b', relayUrl: 'wss://relay', serverId: 2 },
      ],
      workspaces: [],
      activeComputerId: 'a',
      activeWorkspaceId: null,
      paneNames: {},
    });
    const { result } = renderWorkspace();

    act(() => result.current.mergeComputers('a', 'b'));

    expect(result.current.computers).toHaveLength(2);
  });
});

describe('adoptPeerId', () => {
  it('sets peerId on a computer that has none', () => {
    seedStorage({
      version: 2,
      computers: [{ id: 'c1', name: 'This Machine', wsUrl: 'wss://localhost:54321' }],
      workspaces: [],
      activeComputerId: 'c1',
      activeWorkspaceId: null,
      paneNames: {},
    });
    const { result } = renderWorkspace();

    act(() => result.current.adoptPeerId('c1', 'abc'));

    expect(result.current.computers[0].peerId).toBe('abc');
  });

  it('delegates to a merge instead of creating a duplicate identity when the peerId is already owned', () => {
    seedStorage({
      version: 2,
      computers: [
        { id: 'direct-1', name: 'This Machine', wsUrl: 'wss://localhost:54321' },
        { id: 'p2p-1', name: 'Studio', peerId: 'abc', relayUrl: 'wss://relay', serverId: 5 },
      ],
      workspaces: [],
      activeComputerId: 'direct-1',
      activeWorkspaceId: null,
      paneNames: {},
    });
    const { result } = renderWorkspace();

    act(() => result.current.adoptPeerId('direct-1', 'abc'));

    expect(result.current.computers).toHaveLength(1);
    expect(result.current.computers[0].id).toBe('p2p-1');
    expect(result.current.computers[0].wsUrl).toBe('wss://localhost:54321');
  });
});

describe('addLinkedComputer', () => {
  it('preserves a user-customized name when first folding a local computer into its synced identity', () => {
    seedStorage({
      version: 2,
      computers: [{ id: 'c1', name: 'Work Laptop', wsUrl: 'wss://localhost:54321' }],
      workspaces: [],
      activeComputerId: 'c1',
      activeWorkspaceId: null,
      paneNames: {},
    });
    vi.spyOn(localDaemon, 'getLocalDaemon').mockReturnValue({
      peerId: 'abc',
      url: 'wss://localhost:54321',
    });
    const { result } = renderWorkspace();

    // Simulates the account sync effect discovering this peer_id is linked
    // under a generic server-side name — the user's own name (set before
    // ever linking an account) must not be silently overwritten by it.
    act(() => result.current.addLinkedComputer('Computer 5', 'abc', 'wss://relay', 5));

    expect(result.current.computers).toHaveLength(1);
    expect(result.current.computers[0].name).toBe('Work Laptop');
    expect(result.current.computers[0].peerId).toBe('abc');
    expect(result.current.computers[0].serverId).toBe(5);

    vi.restoreAllMocks();
  });

  it('lets the server name win on a normal re-sync of an already-linked computer', () => {
    seedStorage({
      version: 2,
      computers: [{ id: 'c1', name: 'Old Name', peerId: 'abc', relayUrl: 'wss://relay', serverId: 5 }],
      workspaces: [],
      activeComputerId: 'c1',
      activeWorkspaceId: null,
      paneNames: {},
    });
    const { result } = renderWorkspace();

    act(() => result.current.addLinkedComputer('Renamed On Another Device', 'abc', 'wss://relay', 5));

    expect(result.current.computers[0].name).toBe('Renamed On Another Device');
  });
});
