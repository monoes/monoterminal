/**
 * Wraps a computer's ordered list of reachable routes (see `resolveRoutes`
 * in WorkspaceContext.tsx) behind the single `TerminalTransport` interface,
 * trying the fastest route first (a locally-detected daemon) and falling
 * back through the rest — currently just P2P — on failure or timeout.
 *
 * Takes a *thunk*, not a fixed route list: transports are constructed
 * synchronously inside a `useState` initializer (see WorkspaceSession.tsx),
 * long before the async local-daemon probe could possibly resolve. Every
 * `connect()` attempt re-evaluates `getRoutes()`, so a probe that resolves
 * after mount is picked up on the very next connection attempt with no
 * remount and no prop plumbing back into React.
 *
 * Failover policy (locked in): if the active route dies after connecting,
 * advance to the next route immediately so the user isn't stuck — but keep
 * periodically retrying the primary (index 0) route in the background, and
 * switch back to it the moment it's reachable again. This also gives
 * WebRtcClient its first-ever auto-reconnect as a side effect (today a
 * dropped DataChannel just sits in ERROR forever with nothing retrying it).
 */

import { ConnectionState, WebSocketClient } from './websocket-client';
import { WebRtcClient } from './webrtc-client';
import type { MessageHandler, SplitDirection } from './protocol';
import type { Route } from '../state/WorkspaceContext';
import type { TerminalTransport } from './transport';

// Covers the raw WS handshake AND the Ed25519 challenge/auth round trip
// that now runs before CONNECTED (see websocket-client.ts's authenticate())
// — two protobuf round trips plus @noble/ed25519 signing and a possibly-
// cold IndexedDB read, on top of the socket opening. One combined deadline,
// same reasoning as P2P_CONNECT_TIMEOUT_MS below covering WebRTC's own
// multi-stage connect.
const WS_CONNECT_TIMEOUT_MS = 6000;
// WebRTC's connect() is multi-stage (open relay WS -> wait 'connected' ->
// SDP/ICE negotiation -> DataChannel open) — genuinely slower than a single
// WebSocket handshake, so it gets a longer budget before being judged dead.
const P2P_CONNECT_TIMEOUT_MS = 12000;

const INITIAL_RETRY_DELAY_MS = 3000;
const MAX_RETRY_DELAY_MS = 30000;
// How often to re-check for a better route once already on the best one
// currently known — deliberately much slower than the degraded-retry
// backoff above, since this is a steady-state "just keep an eye out" poll,
// not an active recovery attempt.
const STEADY_WATCH_INTERVAL_MS = 30000;

/** Identifies a route by what it actually connects to, not its position in
 * the array `getRoutes()` returns — that array can reshape at any time
 * (e.g. a computer gains a local match well after a pane is already open),
 * so treating an index as a stable identity across calls is wrong: the
 * route that was "index 0" a minute ago may not be anymore. */
function routeKey(route: Route): string {
  return route.kind === 'ws' ? `ws:${route.url}` : `p2p:${route.peerId}:${route.relayUrl}`;
}

function buildClient(route: Route): TerminalTransport {
  if (route.kind === 'ws') {
    // autoReconnect is off on every inner client — HybridTransport owns all
    // retry/failover policy itself; an inner client silently retrying on
    // its own would make "still retrying" and "dead" indistinguishable
    // from the outside.
    return new WebSocketClient({ url: route.url, autoReconnect: false });
  }
  return new WebRtcClient({ relayUrl: route.relayUrl, peerId: route.peerId });
}

export class HybridTransport implements TerminalTransport {
  private readonly getRoutes: () => Route[];
  private handlers: MessageHandler = {};
  private stateListeners = new Set<(state: ConnectionState) => void>();
  private state: ConnectionState = ConnectionState.DISCONNECTED;

  private active: TerminalTransport | null = null;
  /** Identity (see `routeKey`) of the route `active` was connected via —
   * NOT a position, since `getRoutes()` can reorder/reshape at any time. */
  private activeRouteKey: string | null = null;

  /** Bumped on every connect()/disconnect() — every async step (timeouts,
   * background retries, in-flight attempts) checks this before touching
   * shared state, so a stale attempt from a superseded generation can never
   * clobber a newer one. Mirrors the same pattern WebSocketClient itself
   * uses (comparing `this.ws !== socket`) for the identical class of bug. */
  private generation = 0;

  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private retryDelay = INITIAL_RETRY_DELAY_MS;

  /** Every client currently mid-`attemptConnect` (not yet activated) — a
   * route can take up to `P2P_CONNECT_TIMEOUT_MS` (12s) to settle on its
   * own, so without tracking these, `disconnect()` calling only
   * `this.active?.disconnect()` would leave a real WebRTC negotiation
   * running in the background for up to that long after the transport was
   * supposed to be fully torn down. */
  private readonly pendingClients = new Set<TerminalTransport>();

  private readonly buildClient: (route: Route) => TerminalTransport;

  /** `buildClientOverride` exists only for tests — production code always
   * uses the real `buildClient`, which constructs actual WebSocket/WebRTC
   * connections and is impractical to exercise cascade/failover timing
   * against in a unit test. */
  constructor(getRoutes: () => Route[], buildClientOverride?: (route: Route) => TerminalTransport) {
    this.getRoutes = getRoutes;
    this.buildClient = buildClientOverride ?? buildClient;
  }

  connect(): void {
    const generation = ++this.generation;
    this.clearRetryTimer();
    void this.runCascade(0, generation);
  }

  disconnect(): void {
    this.generation++; // invalidates every in-flight attempt/timer
    this.clearRetryTimer();
    this.active?.disconnect();
    this.active = null;
    this.activeRouteKey = null;
    for (const client of this.pendingClients) client.disconnect();
    this.pendingClients.clear();
    this.setState(ConnectionState.DISCONNECTED);
  }

  private async runCascade(startIndex: number, generation: number): Promise<void> {
    const routes = this.getRoutes();
    if (routes.length === 0) {
      if (generation !== this.generation) return;
      this.setState(ConnectionState.ERROR);
      this.scheduleCascadeRestart(generation);
      return;
    }

    if (generation === this.generation) this.setState(ConnectionState.CONNECTING);

    for (let i = startIndex; i < routes.length; i++) {
      if (generation !== this.generation) return;
      const route = routes[i];
      const client = this.buildClient(route);
      client.setHandlers(this.handlers);
      this.pendingClients.add(client);
      const connected = await this.attemptConnect(client, route);
      this.pendingClients.delete(client);
      if (generation !== this.generation) {
        client.disconnect();
        return;
      }
      if (connected) {
        this.activateClient(client, route, generation);
        return;
      }
      client.disconnect();
    }

    if (generation !== this.generation) return;
    this.setState(ConnectionState.ERROR);
    this.scheduleCascadeRestart(generation);
  }

  /** Races a single client's connect against a per-route deadline. Resolves
   * `true`/`false` only for this initial attempt — death *after* a
   * successful connect is handled separately by `activateClient`'s own
   * subscription, not by this promise. */
  private attemptConnect(client: TerminalTransport, route: Route): Promise<boolean> {
    return new Promise((resolve) => {
      let settled = false;
      const timeoutMs = route.kind === 'ws' ? WS_CONNECT_TIMEOUT_MS : P2P_CONNECT_TIMEOUT_MS;

      const finish = (result: boolean) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        unsubscribe();
        resolve(result);
      };

      const timer = setTimeout(() => finish(false), timeoutMs);
      // Deliberately no generation check here: this promise settling is
      // purely local bookkeeping for *this* attempt (clearing its own timer
      // and listener) — every caller already re-checks generation right
      // after awaiting it before acting on the result. Gating on generation
      // here instead meant an external disconnect() on a still-pending
      // client (see `pendingClients`) — which bumps generation *before*
      // calling client.disconnect() — left this listener ignoring the very
      // DISCONNECTED it caused, so the promise stayed unsettled until the
      // full connect timeout (up to 12s) elapsed instead of resolving
      // immediately.
      const unsubscribe = client.onStateChange((state) => {
        if (state === ConnectionState.CONNECTED) finish(true);
        else if (state === ConnectionState.ERROR || state === ConnectionState.DISCONNECTED) finish(false);
      });

      client.connect();
    });
  }

  /** Makes `client` (already CONNECTED, via `route`) the active transport,
   * forwards the CONNECTED transition upward — always through a CONNECTING
   * blip first, even when the public state is already CONNECTED (a
   * route-watch switchover), since `setState` only notifies listeners on
   * an actual change: WorkspaceSession's own re-`attach()` only fires on
   * that transition, so silently swapping the underlying client without
   * one would leave it attached to nothing. Watches for this client dying
   * afterward to drive failover, and always starts the route-watch loop
   * (see `scheduleRouteWatch`) — `getRoutes()` can later prepend a better
   * route than the one we're on right now even without this connection
   * ever dying (e.g. a local daemon match discovered mid-session), so
   * "already optimal" can only be known by asking again later, not frozen
   * at activation time. */
  private activateClient(client: TerminalTransport, route: Route, generation: number): void {
    const previous = this.active;
    this.active = client;
    this.activeRouteKey = routeKey(route);
    this.retryDelay = INITIAL_RETRY_DELAY_MS;

    this.setState(ConnectionState.CONNECTING);
    this.setState(ConnectionState.CONNECTED);
    previous?.disconnect();

    const unsubscribe = client.onStateChange((state) => {
      if (state === ConnectionState.CONNECTED) return;
      unsubscribe();
      if (generation !== this.generation || this.active !== client) return; // already superseded
      this.active = null;
      this.activeRouteKey = null;
      this.clearRetryTimer();
      // Always restart from the top, re-evaluating getRoutes() fresh —
      // never resume from "the position after wherever we were," which
      // could skip straight past a route that's only just become available
      // (e.g. `routeIndex + 1` would skip a newly-prepended local route
      // entirely, retrying the very P2P route that just died instead).
      void this.runCascade(0, generation);
    });

    this.scheduleRouteWatch(generation);
  }

  /** Periodically checks whether `getRoutes()[0]` — the current best-known
   * route — differs from the one we're actually on, and switches to it if
   * so. Doubles as both "actively retry a known-better route while
   * degraded" (short, backing-off interval) and "steady-state watch for a
   * route list that reshapes while we're already optimal and healthy"
   * (slow, fixed interval) — the two aren't distinguishable in advance
   * since `getRoutes()` can change for reasons entirely outside this
   * transport's knowledge (an identity merge, a probe resolving late). */
  private scheduleRouteWatch(generation: number): void {
    this.clearRetryTimer();

    const check = async () => {
      if (generation !== this.generation) return;
      const routes = this.getRoutes();
      if (routes.length === 0) {
        this.retryTimer = setTimeout(() => void check(), STEADY_WATCH_INTERVAL_MS);
        return;
      }

      const primary = routes[0];
      if (routeKey(primary) === this.activeRouteKey) {
        // Already on the best known route — reset backoff for whenever we
        // next actually need it, and just watch at a low, steady cadence.
        this.retryDelay = INITIAL_RETRY_DELAY_MS;
        this.retryTimer = setTimeout(() => void check(), STEADY_WATCH_INTERVAL_MS);
        return;
      }

      const client = this.buildClient(primary);
      client.setHandlers(this.handlers);
      this.pendingClients.add(client);
      const connected = await this.attemptConnect(client, primary);
      this.pendingClients.delete(client);
      if (generation !== this.generation) {
        client.disconnect();
        return;
      }
      if (connected) {
        this.activateClient(client, primary, generation); // reschedules the watch again
        return;
      }
      client.disconnect();
      this.retryDelay = Math.min(this.retryDelay * 2, MAX_RETRY_DELAY_MS);
      this.retryTimer = setTimeout(() => void check(), this.retryDelay);
    };

    this.retryTimer = setTimeout(() => void check(), this.retryDelay);
  }

  /** All routes just failed in the same cascade round — back off before
   * trying the whole thing again from the top. */
  private scheduleCascadeRestart(generation: number): void {
    this.clearRetryTimer();
    this.retryTimer = setTimeout(() => {
      if (generation !== this.generation) return;
      this.retryDelay = Math.min(this.retryDelay * 2, MAX_RETRY_DELAY_MS);
      void this.runCascade(0, generation);
    }, this.retryDelay);
  }

  private clearRetryTimer(): void {
    if (this.retryTimer !== null) {
      clearTimeout(this.retryTimer);
      this.retryTimer = null;
    }
  }

  private setState(newState: ConnectionState): void {
    if (this.state === newState) return;
    this.state = newState;
    for (const listener of this.stateListeners) {
      try {
        listener(newState);
      } catch (err) {
        console.error('HybridTransport state listener error:', err);
      }
    }
  }

  getState(): ConnectionState {
    return this.state;
  }

  onStateChange(listener: (state: ConnectionState) => void): () => void {
    this.stateListeners.add(listener);
    return () => this.stateListeners.delete(listener);
  }

  setHandlers(handlers: MessageHandler): void {
    this.handlers = handlers;
    this.active?.setHandlers(handlers);
  }

  attach(sessionId: string, rows: number, cols: number, sessionName?: string, previousSessionName?: string): void {
    this.active?.attach(sessionId, rows, cols, sessionName, previousSessionName);
  }

  sendInput(data: string | Uint8Array, paneId?: string): void {
    this.active?.sendInput(data, paneId);
  }

  resize(rows: number, cols: number, paneId?: string): void {
    this.active?.resize(rows, cols, paneId);
  }

  splitPane(paneId: string, direction: SplitDirection, newSessionShell?: string): void {
    this.active?.splitPane(paneId, direction, newSessionShell);
  }

  closePane(paneId: string): void {
    this.active?.closePane(paneId);
  }

  focusPane(paneId: string): void {
    this.active?.focusPane(paneId);
  }
}
