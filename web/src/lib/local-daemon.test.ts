/**
 * local-daemon.ts tests — specifically the `notify()` first-call regression:
 * `lastResult` starts as `null`, the same value a completed-but-empty probe
 * produces, so a naive "suppress if unchanged" check would wrongly swallow
 * the very first notification whenever the first probe finds no daemon
 * (the common case on most machines).
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// Mirrors websocket-client.test.ts's mock — a WebSocket that never opens,
// simulating "nothing listening on wss://localhost:54321" (no daemon).
class NeverOpensWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;
  readyState = NeverOpensWebSocket.CONNECTING;
  binaryType = 'blob';
  onopen: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  constructor(public url: string) {}
  close = vi.fn();
  send = vi.fn();
}

describe('local-daemon notify() first-call semantics', () => {
  beforeEach(() => {
    global.WebSocket = NeverOpensWebSocket as unknown as typeof WebSocket;
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('notifies once for the first probe even when it finds no daemon, and suppresses an identical second result', async () => {
    vi.resetModules();
    const { probeLocalDaemon, refreshLocalDaemon, subscribeLocalDaemon } = await import(
      './local-daemon'
    );

    const seen: unknown[] = [];
    subscribeLocalDaemon((daemon) => seen.push(daemon));

    const first = probeLocalDaemon();
    await vi.advanceTimersByTimeAsync(4001); // CONNECT_TIMEOUT_MS
    expect(await first).toBeNull();

    // The regression: sameResult(null, null) is true, so a version of
    // notify() with no separate "has this ever probed" flag would treat
    // this first, genuinely-new "no daemon" result as unchanged from the
    // pre-probe default and never call any listener at all.
    expect(seen).toEqual([null]);

    // Second probe, same outcome (still no daemon) — this one SHOULD be
    // suppressed as a genuine no-change, unlike the first.
    vi.advanceTimersByTime(61_000); // past MIN_REPROBE_INTERVAL_MS
    const second = refreshLocalDaemon();
    await vi.advanceTimersByTimeAsync(4001);
    expect(await second).toBeNull();
    expect(seen).toEqual([null]); // still just the one notification
  });
});
