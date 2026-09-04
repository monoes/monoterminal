/**
 * Detects whether a daemon is reachable at `wss://localhost:54321` — the
 * browser's own machine — so the app can prefer a fast direct connection to
 * it over relaying through P2P for whichever synced computer turns out to
 * share its peer_id (see `resolveRoutes` in WorkspaceContext.tsx and
 * `HybridTransport`). Deliberately localhost-only: a browser has no way to
 * discover a daemon elsewhere on the LAN without that daemon first
 * advertising itself somewhere reachable, which is out of scope here.
 *
 * A single module-level singleton, not a hook or per-component instance —
 * three independent consumers (Sidebar's merge effect, transport route
 * resolution, App's auto-link effect) all need the same in-flight probe and
 * the same cached result, not three separate connection attempts.
 */

import { ConnectionState, WebSocketClient } from './websocket-client';

const LOCAL_WS_URL = import.meta.env.VITE_LOCAL_WS_URL || 'wss://localhost:54321';

/** How long to wait for the local daemon's WebSocket to open (now including
 * the Ed25519 challenge/auth handshake that runs before CONNECTED — see
 * websocket-client.ts's authenticate()) before giving up. Short — this runs
 * on every app load, and a machine with no local daemon at all should fail
 * fast rather than hang the UI. */
const CONNECT_TIMEOUT_MS = 4000;

/** How long to wait for the `account_peer_id` dashboard reply after the
 * socket opens. `sendDashboardRequest` itself times out at 10s; this races
 * a tighter bound around it so a slow/wedged daemon doesn't stall callers. */
const REQUEST_TIMEOUT_MS = 3000;

/** Re-probe at most this often on 'online'/visibility-change triggers —
 * these can fire in bursts (e.g. several tabs waking at once). */
const MIN_REPROBE_INTERVAL_MS = 60_000;

export interface LocalDaemon {
  peerId: string;
  url: string;
}

let cached: Promise<LocalDaemon | null> | null = null;
let lastProbeAt = 0;
let lastResult: LocalDaemon | null = null;
// `lastResult` starts as `null`, the same value a completed-but-empty probe
// produces — without tracking this separately, the very first notification
// would be wrongly suppressed as a "no change" whenever the first probe
// finds no daemon (the common case), since `null === null`.
let hasProbed = false;
const listeners = new Set<(daemon: LocalDaemon | null) => void>();

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('timeout')), ms);
    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      (err) => {
        clearTimeout(timer);
        reject(err);
      }
    );
  });
}

async function runProbe(): Promise<LocalDaemon | null> {
  const client = new WebSocketClient({ url: LOCAL_WS_URL, autoReconnect: false });

  try {
    const connected = new Promise<void>((resolve, reject) => {
      if (client.getState() === ConnectionState.CONNECTED) {
        resolve();
        return;
      }
      const unsubscribe = client.onStateChange((state) => {
        if (state === ConnectionState.CONNECTED) {
          unsubscribe();
          resolve();
        } else if (state === ConnectionState.ERROR || state === ConnectionState.DISCONNECTED) {
          unsubscribe();
          reject(new Error('connection failed'));
        }
      });
    });
    client.connect();
    await withTimeout(connected, CONNECT_TIMEOUT_MS);

    const response = await withTimeout(
      client.sendDashboardRequest({ command: 'account_peer_id' }),
      REQUEST_TIMEOUT_MS
    );
    if (response.error !== 0) return null; // pre-Phase-1 daemon build, or genuinely unavailable

    const { peer_id: peerId } = JSON.parse(response.jsonData) as { peer_id: string };
    if (!peerId) return null;

    return { peerId, url: LOCAL_WS_URL };
  } catch (err) {
    console.debug('[local-daemon] probe failed:', err);
    return null;
  } finally {
    client.disconnect();
  }
}

function sameResult(a: LocalDaemon | null, b: LocalDaemon | null): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return a.peerId === b.peerId && a.url === b.url;
}

/** No-ops when the result hasn't actually changed — a `refreshLocalDaemon()`
 * from a tab-focus or `online` event fires often and would otherwise notify
 * every subscriber (e.g. App.tsx's Monomind-panel effect) even when nothing
 * relevant changed for them, causing an unnecessary teardown/reconnect on
 * every such event instead of only when the probe result is genuinely new. */
function notify(daemon: LocalDaemon | null): void {
  if (hasProbed && sameResult(lastResult, daemon)) return;
  hasProbed = true;
  lastResult = daemon;
  for (const fn of listeners) fn(daemon);
}

/** Single-flight, memoized probe of the local daemon. Repeated calls while
 * one is already resolved return the cached result; call `refreshLocalDaemon`
 * to force a fresh attempt. */
export function probeLocalDaemon(): Promise<LocalDaemon | null> {
  if (!cached) {
    lastProbeAt = Date.now();
    cached = runProbe().then((result) => {
      notify(result);
      return result;
    });
  }
  return cached;
}

/** Synchronous read of the last resolved probe result — `null` both before
 * the first probe resolves and when no daemon was found. */
export function getLocalDaemon(): LocalDaemon | null {
  return lastResult;
}

/** Clears the memoized result and probes again. Safe to call often — it's
 * a no-op cheap guard, not a fresh network attempt, when called more often
 * than `MIN_REPROBE_INTERVAL_MS` since the last probe. */
export function refreshLocalDaemon(): Promise<LocalDaemon | null> {
  if (cached && Date.now() - lastProbeAt < MIN_REPROBE_INTERVAL_MS) {
    return cached;
  }
  cached = null;
  return probeLocalDaemon();
}

/** Subscribe to probe results (initial and any subsequent refresh).
 * Returns an unsubscribe function. */
export function subscribeLocalDaemon(fn: (daemon: LocalDaemon | null) => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}
