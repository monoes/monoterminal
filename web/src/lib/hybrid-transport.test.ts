/**
 * HybridTransport tests — cascade/failover logic, exercised against fake
 * TerminalTransport stand-ins (never real WebSocket/WebRTC connections) via
 * the constructor's `buildClientOverride` test hook.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { HybridTransport } from './hybrid-transport';
import { ConnectionState } from './websocket-client';
import type { TerminalTransport } from './transport';
import type { Route } from '../state/WorkspaceContext';

/** A controllable fake transport: starts DISCONNECTED, and `connect()` does
 * nothing on its own — the test drives it to CONNECTED/ERROR explicitly via
 * `emit`, so cascade timing is deterministic under fake timers. */
class FakeTransport implements TerminalTransport {
  state: ConnectionState = ConnectionState.DISCONNECTED;
  private listeners = new Set<(state: ConnectionState) => void>();
  connectCalls = 0;
  disconnectCalls = 0;

  connect(): void {
    this.connectCalls++;
  }

  disconnect(): void {
    this.disconnectCalls++;
    this.emit(ConnectionState.DISCONNECTED);
  }

  emit(state: ConnectionState): void {
    this.state = state;
    for (const l of this.listeners) l(state);
  }

  onStateChange(listener: (state: ConnectionState) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  getState(): ConnectionState {
    return this.state;
  }

  setHandlers(): void {}
  attach(): void {}
  sendInput(): void {}
  resize(): void {}
  splitPane(): void {}
  closePane(): void {}
  focusPane(): void {}
}

const WS_ROUTE: Route = { kind: 'ws', url: 'wss://localhost:54321' };
const P2P_ROUTE: Route = { kind: 'p2p', peerId: 'abc', relayUrl: 'wss://relay' };

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

/** Flushes pending microtasks — `emit()` resolves a Promise inside
 * `attemptConnect`, and resuming the `await`ing async cascade function
 * needs a tick even though no real timer is involved. */
async function flush() {
  await Promise.resolve();
  await Promise.resolve();
}

describe('HybridTransport', () => {
  it('connects on the first route and never constructs the second', async () => {
    const fakes: FakeTransport[] = [];
    const build = () => {
      const f = new FakeTransport();
      fakes.push(f);
      return f;
    };
    const transport = new HybridTransport(() => [WS_ROUTE, P2P_ROUTE], build);

    transport.connect();
    expect(fakes).toHaveLength(1);
    fakes[0].emit(ConnectionState.CONNECTED);
    await flush();

    expect(transport.getState()).toBe(ConnectionState.CONNECTED);
    expect(fakes).toHaveLength(1);
  });

  it('falls through to the second route when the first times out, staying CONNECTING throughout', async () => {
    const fakes: FakeTransport[] = [];
    const build = () => {
      const f = new FakeTransport();
      fakes.push(f);
      return f;
    };
    const transport = new HybridTransport(() => [WS_ROUTE, P2P_ROUTE], build);
    const states: ConnectionState[] = [];
    transport.onStateChange((s) => states.push(s));

    transport.connect();
    expect(fakes).toHaveLength(1);

    await vi.advanceTimersByTimeAsync(6001); // WS_CONNECT_TIMEOUT_MS
    expect(fakes).toHaveLength(2); // second route now attempted
    expect(transport.getState()).toBe(ConnectionState.CONNECTING);
    expect(states).not.toContain(ConnectionState.ERROR);

    fakes[1].emit(ConnectionState.CONNECTED);
    await flush();
    expect(transport.getState()).toBe(ConnectionState.CONNECTED);
  });

  it('advances to the next route immediately when the active route dies post-connect', async () => {
    const fakes: FakeTransport[] = [];
    const build = () => {
      const f = new FakeTransport();
      fakes.push(f);
      return f;
    };
    const transport = new HybridTransport(() => [WS_ROUTE, P2P_ROUTE], build);

    transport.connect();
    fakes[0].emit(ConnectionState.CONNECTED);
    await flush();
    expect(transport.getState()).toBe(ConnectionState.CONNECTED);

    fakes[0].emit(ConnectionState.ERROR);
    await flush();
    expect(fakes).toHaveLength(2); // moved on to the p2p route

    fakes[1].emit(ConnectionState.CONNECTED);
    await flush();
    expect(transport.getState()).toBe(ConnectionState.CONNECTED);
  });

  it('background-retries the primary route while a later route is active, and switches back on success', async () => {
    const fakes: FakeTransport[] = [];
    const build = () => {
      const f = new FakeTransport();
      fakes.push(f);
      return f;
    };
    const transport = new HybridTransport(() => [WS_ROUTE, P2P_ROUTE], build);
    const states: ConnectionState[] = [];

    transport.connect();
    fakes[0].emit(ConnectionState.ERROR); // primary fails immediately
    await flush();
    fakes[1].emit(ConnectionState.CONNECTED); // p2p takes over
    await flush();
    transport.onStateChange((s) => states.push(s));

    await vi.advanceTimersByTimeAsync(3001); // INITIAL_RETRY_DELAY_MS — background retry fires
    expect(fakes).toHaveLength(3); // a fresh attempt at the primary route
    fakes[2].emit(ConnectionState.CONNECTED);
    await flush();

    // Switching back must re-emit CONNECTED (via a CONNECTING blip) so a
    // consumer's "attach on CONNECTED transition" logic actually re-fires —
    // silently swapping the active client without this would leave the new
    // one never attached to anything.
    expect(states).toContain(ConnectionState.CONNECTING);
    expect(states[states.length - 1]).toBe(ConnectionState.CONNECTED);
    expect(fakes[1].disconnectCalls).toBeGreaterThan(0); // old active torn down
  });

  it('goes to ERROR and does not throw when there are no routes at all', async () => {
    const transport = new HybridTransport(() => [], () => new FakeTransport());
    expect(() => transport.connect()).not.toThrow();
    await flush();
    expect(transport.getState()).toBe(ConnectionState.ERROR);
  });

  it('disconnect() invalidates in-flight attempts so a stale timeout cannot resurrect the transport', async () => {
    const fakes: FakeTransport[] = [];
    const build = () => {
      const f = new FakeTransport();
      fakes.push(f);
      return f;
    };
    const transport = new HybridTransport(() => [WS_ROUTE], build);

    transport.connect();
    transport.disconnect();
    expect(transport.getState()).toBe(ConnectionState.DISCONNECTED);

    // The in-flight attempt's own connect timeout firing later must not
    // reanimate a cascade for a generation that's already been superseded.
    await vi.advanceTimersByTimeAsync(6001);
    expect(transport.getState()).toBe(ConnectionState.DISCONNECTED);
  });

  it('discovers a newly-available better route even though the current connection never dies', async () => {
    const fakes: FakeTransport[] = [];
    const build = () => {
      const f = new FakeTransport();
      fakes.push(f);
      return f;
    };
    // Starts p2p-only (e.g. a computer with no local match yet) — routeKey
    // comparison must not freeze "already optimal" at activation time,
    // since getRoutes() can reshape later without this connection ever
    // failing over.
    let routes: Route[] = [P2P_ROUTE];
    const transport = new HybridTransport(() => routes, build);

    transport.connect();
    fakes[0].emit(ConnectionState.CONNECTED);
    await flush();
    expect(transport.getState()).toBe(ConnectionState.CONNECTED);

    // A local match is discovered mid-session (mergeComputers/adoptPeerId)
    // — the ws route now outranks the still-healthy p2p one. The route
    // watch's very first check after activation runs at the (short) retry
    // delay, not the steady-state interval — reacting quickly the first
    // time it notices a mismatch, well before falling back to slow polling
    // once it's confirmed already-optimal.
    routes = [WS_ROUTE, P2P_ROUTE];

    await vi.advanceTimersByTimeAsync(3001); // INITIAL_RETRY_DELAY_MS
    expect(fakes).toHaveLength(2); // route watch noticed the new primary and tried it
    fakes[1].emit(ConnectionState.CONNECTED);
    await flush();

    expect(transport.getState()).toBe(ConnectionState.CONNECTED);
    expect(fakes[0].disconnectCalls).toBeGreaterThan(0); // old p2p client torn down
  });

  it('restarts the whole cascade on failover instead of skipping a route that only just became available', async () => {
    const built: Route[] = [];
    const fakes: FakeTransport[] = [];
    const build = (route: Route) => {
      built.push(route);
      const f = new FakeTransport();
      fakes.push(f);
      return f;
    };
    let routes: Route[] = [P2P_ROUTE];
    const transport = new HybridTransport(() => routes, build);

    transport.connect();
    fakes[0].emit(ConnectionState.CONNECTED);
    await flush();

    // Route list reshapes (ws route now available) — then the active p2p
    // connection dies. Naively resuming at "the position after wherever we
    // were" (index 1 in the old array) would retry p2p again and skip the
    // newly-available ws route entirely.
    routes = [WS_ROUTE, P2P_ROUTE];
    fakes[0].emit(ConnectionState.ERROR);
    await flush();

    expect(built[1]).toEqual(WS_ROUTE); // tried the fresh best route first, not p2p again
  });

  it('disconnect() immediately tears down a client still mid-attempt, not just the active one', async () => {
    const fakes: FakeTransport[] = [];
    const build = () => {
      const f = new FakeTransport();
      fakes.push(f);
      return f;
    };
    // Slow (p2p-shaped) route so the attempt is still pending when disconnect() runs.
    const transport = new HybridTransport(() => [P2P_ROUTE], build);

    transport.connect();
    expect(fakes).toHaveLength(1);
    expect(fakes[0].disconnectCalls).toBe(0); // still mid-attempt, nothing torn down yet

    transport.disconnect();

    // Without tracking pending (not-yet-activated) clients, this client would
    // only get disconnected once its own 12s connect timeout eventually
    // fired — a real WebRTC negotiation left running long after the
    // transport was supposed to be fully torn down.
    expect(fakes[0].disconnectCalls).toBeGreaterThan(0);
  });
});
