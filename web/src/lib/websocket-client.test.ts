/**
 * WebSocket Client Tests
 * Tests state machine, reconnection logic, message encoding/decoding, and
 * the Ed25519 challenge-response auth handshake that now runs on every
 * connect before the public CONNECTED transition.
 *
 * The real crypto (`AuthService`) is mocked via `./auth/service-singleton`
 * — this file is about connection/message-ordering behavior, not signing
 * correctness (that's covered by the Rust wire tests in
 * crates/master/tests/auth_wire_flow.rs, plus web/src/lib/auth/*.test.ts
 * for the crypto helpers themselves, real-browser-only due to jsdom's
 * SubtleCrypto limitation — see that file's header comment).
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { WebSocketClient, ConnectionState } from './websocket-client';
import { decodeEnvelope } from './protocol';

// ---- Fake AuthService (mocked module) --------------------------------

const fakeAuthService = {
  jwt: null as string | null,
  jwtTimeRemaining: null as number | null,
  getJWT: vi.fn(() => fakeAuthService.jwt),
  getJWTTimeRemaining: vi.fn(() => fakeAuthService.jwtTimeRemaining),
  setJWT: vi.fn((token: string, expiresIn: number) => {
    fakeAuthService.jwt = token;
    fakeAuthService.jwtTimeRemaining = expiresIn;
  }),
  clearJWT: vi.fn(() => {
    fakeAuthService.jwt = null;
    fakeAuthService.jwtTimeRemaining = null;
  }),
  signChallenge: vi.fn(async (_challenge: { nonce: Uint8Array }) => ({
    signature: new Uint8Array(64).fill(1),
    publicKey: new Uint8Array(32).fill(2),
  })),
};

vi.mock('./auth/service-singleton', () => ({
  getAuthService: () => Promise.resolve(fakeAuthService),
}));

function resetFakeAuthService() {
  fakeAuthService.jwt = null;
  fakeAuthService.jwtTimeRemaining = null;
  fakeAuthService.getJWT.mockClear();
  fakeAuthService.getJWTTimeRemaining.mockClear();
  fakeAuthService.setJWT.mockClear();
  fakeAuthService.clearJWT.mockClear();
  fakeAuthService.signChallenge.mockClear();
}

/** Flushes pending microtasks — `authenticate()` awaits across several
 * steps (getAuthService(), sendChallengeRequest(), signChallenge(),
 * sendAuthRequest()) even on the fast (already-authenticated) path, so
 * asserting the resulting state immediately after `onopen` needs a tick. */
async function flush() {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

// Mock WebSocket globally
class MockWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;

  readyState = MockWebSocket.CONNECTING;
  binaryType: string = 'blob';
  onopen: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;

  constructor(public url: string) {
    // Store instance for test access
    MockWebSocket.lastInstance = this;
  }

  close = vi.fn();
  send = vi.fn();
  addEventListener = vi.fn();
  removeEventListener = vi.fn();

  /** Simulates the server replying to whatever the client just sent, by
   * decoding it and building a canned response of the matching type.
   * Only understands the auth handshake messages — enough for these tests. */
  respondToLastSend(overrides?: { challengeExpiresAt?: number; userId?: string }) {
    const lastCall = this.send.mock.calls[this.send.mock.calls.length - 1];
    const sent = decodeEnvelope(lastCall[0]);
    // Echo the request's own sequence number — the client's pendingRequests
    // map is keyed by it, so a mismatched value here would leave the
    // response silently unmatched (see this feature's own Phase 0a fix).
    const sequenceNumber = sent.sequenceNumber as number;
    let message: Record<string, unknown>;
    if (sent.challengeRequest) {
      message = {
        challengeResponse: {
          nonce: new Uint8Array(32).fill(9),
          expiresAt: overrides?.challengeExpiresAt ?? Math.floor(Date.now() / 1000) + 30,
        },
      };
    } else if (sent.authRequest) {
      message = {
        authResponse: {
          accessToken: 'access-token-value',
          refreshToken: 'refresh-token-value',
          accessExpiresAt: Math.floor(Date.now() / 1000) + 900,
          refreshExpiresAt: Math.floor(Date.now() / 1000) + 2_592_000,
          userId: overrides?.userId ?? 'ed25519:abcdef',
        },
      };
    } else if (sent.tokenRefreshRequest) {
      message = {
        tokenRefreshResponse: {
          accessToken: 'refreshed-access-token',
          refreshToken: 'refreshed-refresh-token',
          accessExpiresAt: Math.floor(Date.now() / 1000) + 900,
          refreshExpiresAt: Math.floor(Date.now() / 1000) + 2_592_000,
        },
      };
    } else {
      throw new Error('respondToLastSend: unrecognized request');
    }

    this.onmessage?.({
      data: encodeTestEnvelope(message, sequenceNumber),
    } as MessageEvent);
  }

  static lastInstance: MockWebSocket | null = null;
}

// Re-import encodeEnvelope for building canned server responses.
import { encodeEnvelope } from './protocol';
function encodeTestEnvelope(message: Record<string, unknown>, sequenceNumber = 1): ArrayBuffer {
  const bytes = encodeEnvelope({ sequenceNumber, ...message });
  // Re-copy into a Uint8Array allocated in this (jsdom) realm — protobufjs's
  // encoder returns bytes backed by Node's real ArrayBuffer, whose identity
  // differs from jsdom's simulated `ArrayBuffer` global even within the same
  // test file. websocket-client.ts's `event.data instanceof ArrayBuffer`
  // check (running in the jsdom realm) fails on the cross-realm original,
  // silently dropping every canned response — the same jsdom realm split
  // documented for SubtleCrypto in auth/challenge.test.ts.
  const copy = new Uint8Array(bytes);
  return copy.buffer;
}

/** Fires `onopen`, first flipping `readyState` to OPEN — a real browser
 * transitions readyState before dispatching the 'open' event; sendEnvelope()
 * checks readyState, so a test driving onopen directly must replicate that
 * ordering or every send inside the handshake silently no-ops. */
function triggerOpen(ws: MockWebSocket) {
  ws.readyState = MockWebSocket.OPEN;
  ws.onopen?.(new Event('open'));
}

/** Drives a client through open + a full (non-fast-path) auth round trip. */
async function openAndAuthenticate(ws: MockWebSocket) {
  triggerOpen(ws);
  await flush();
  ws.respondToLastSend(); // ChallengeResponse
  await flush();
  ws.respondToLastSend(); // AuthResponse
  await flush();
}

describe('WebSocketClient', () => {
  let client: WebSocketClient;
  const TEST_URL = 'ws://localhost:5000/ws';

  beforeEach(() => {
    global.WebSocket = MockWebSocket as any;
    (global.WebSocket as any).OPEN = MockWebSocket.OPEN;
    MockWebSocket.lastInstance = null;
    resetFakeAuthService();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
    client?.disconnect();
  });

  describe('Connection Lifecycle', () => {
    it('should start in DISCONNECTED state', () => {
      client = new WebSocketClient({ url: TEST_URL });
      expect(client.getState()).toBe(ConnectionState.DISCONNECTED);
    });

    it('should transition to CONNECTING when connect() is called', () => {
      client = new WebSocketClient({ url: TEST_URL });
      const stateListener = vi.fn();
      client.onStateChange(stateListener);

      client.connect();

      expect(client.getState()).toBe(ConnectionState.CONNECTING);
      expect(stateListener).toHaveBeenCalledWith(ConnectionState.CONNECTING);
      expect(MockWebSocket.lastInstance?.binaryType).toBe('arraybuffer');
    });

    it('should not reach CONNECTED until the auth handshake completes', async () => {
      client = new WebSocketClient({ url: TEST_URL });
      client.connect();
      const ws = MockWebSocket.lastInstance!;

      triggerOpen(ws);
      await flush();
      // Still not CONNECTED — no ChallengeResponse/AuthResponse yet. This is
      // the actual bug class being fixed: firing CONNECTED synchronously on
      // open would let WorkspaceSession's attach() race a JWT that doesn't
      // exist yet.
      expect(client.getState()).not.toBe(ConnectionState.CONNECTED);

      await openAndAuthenticate(ws);
      expect(client.getState()).toBe(ConnectionState.CONNECTED);
    });

    it('performs Challenge -> Auth in that order before any attach can be sent', async () => {
      client = new WebSocketClient({ url: TEST_URL });
      client.connect();
      const ws = MockWebSocket.lastInstance!;
      await openAndAuthenticate(ws);

      const sentTypes = ws.send.mock.calls.map((call) => {
        const decoded = decodeEnvelope(call[0]);
        return Object.keys(decoded).find((k) => k !== 'sequenceNumber');
      });
      expect(sentTypes).toEqual(['challengeRequest', 'authRequest']);
    });

    it('skips the round trip and connects immediately when already authenticated', async () => {
      fakeAuthService.jwt = 'already-valid-jwt';
      fakeAuthService.jwtTimeRemaining = 600;

      client = new WebSocketClient({ url: TEST_URL });
      client.connect();
      const ws = MockWebSocket.lastInstance!;
      triggerOpen(ws);
      await flush();

      expect(client.getState()).toBe(ConnectionState.CONNECTED);
      expect(ws.send).not.toHaveBeenCalled(); // no challenge/auth round trip
    });

    it('transitions to ERROR and closes the socket when authentication fails', async () => {
      fakeAuthService.signChallenge.mockRejectedValueOnce(new Error('signing failed'));

      client = new WebSocketClient({ url: TEST_URL });
      client.connect();
      const ws = MockWebSocket.lastInstance!;
      triggerOpen(ws);
      await flush();
      ws.respondToLastSend(); // ChallengeResponse — signChallenge() then rejects
      await flush();

      expect(client.getState()).toBe(ConnectionState.ERROR);
      expect(ws.close).toHaveBeenCalled();
    });

    it('should transition to ERROR on WebSocket error', () => {
      client = new WebSocketClient({ url: TEST_URL });
      client.connect();

      const ws = MockWebSocket.lastInstance;
      ws?.onerror?.(new Event('error'));

      expect(client.getState()).toBe(ConnectionState.ERROR);
    });

    it('should transition to DISCONNECTED on manual disconnect', async () => {
      client = new WebSocketClient({ url: TEST_URL, autoReconnect: false });
      client.connect();
      const ws = MockWebSocket.lastInstance!;
      await openAndAuthenticate(ws);

      client.disconnect();

      expect(client.getState()).toBe(ConnectionState.DISCONNECTED);
      expect(ws?.close).toHaveBeenCalled();
    });
  });

  describe('Reconnection Logic', () => {
    it('should schedule reconnect on close with autoReconnect=true', async () => {
      client = new WebSocketClient({
        url: TEST_URL,
        autoReconnect: true,
        reconnectInterval: 3000,
      });

      client.connect();
      const ws = MockWebSocket.lastInstance!;
      await openAndAuthenticate(ws);
      ws?.onclose?.(new CloseEvent('close'));

      expect(client.getState()).toBe(ConnectionState.RECONNECTING);
    });

    it('should not reconnect if autoReconnect=false', async () => {
      client = new WebSocketClient({
        url: TEST_URL,
        autoReconnect: false,
      });

      client.connect();
      const ws = MockWebSocket.lastInstance!;
      await openAndAuthenticate(ws);
      ws?.onclose?.(new CloseEvent('close'));

      vi.advanceTimersByTime(10000);

      expect(client.getState()).toBe(ConnectionState.DISCONNECTED);
    });

    it('should clear reconnect timer on manual disconnect', async () => {
      client = new WebSocketClient({
        url: TEST_URL,
        autoReconnect: true,
      });

      client.connect();
      const ws = MockWebSocket.lastInstance!;
      await openAndAuthenticate(ws);
      ws?.onclose?.(new CloseEvent('close'));

      client.disconnect();
      vi.advanceTimersByTime(5000);

      expect(client.getState()).toBe(ConnectionState.DISCONNECTED);
    });
  });

  describe('JWT refresh', () => {
    it('schedules a proactive refresh and re-arms after it fires', async () => {
      client = new WebSocketClient({ url: TEST_URL });
      client.connect();
      const ws = MockWebSocket.lastInstance!;
      await openAndAuthenticate(ws); // access token expires in 900s (see canned response)

      // Fires ~2 minutes before expiry: (900 - 120) * 1000 = 780000ms.
      await vi.advanceTimersByTimeAsync(780_001);
      ws.respondToLastSend(); // TokenRefreshResponse
      await flush();

      const sentTypes = ws.send.mock.calls.map((call) => {
        const decoded = decodeEnvelope(call[0]);
        return Object.keys(decoded).find((k) => k !== 'sequenceNumber');
      });
      expect(sentTypes).toContain('tokenRefreshRequest');
      expect(client.getState()).toBe(ConnectionState.CONNECTED); // still healthy
    });

    it('clears the refresh timer on disconnect so it never fires against a dead socket', async () => {
      client = new WebSocketClient({ url: TEST_URL, autoReconnect: false });
      client.connect();
      const ws = MockWebSocket.lastInstance!;
      await openAndAuthenticate(ws);

      client.disconnect();
      const sendCallsBeforeAdvance = ws.send.mock.calls.length;

      await vi.advanceTimersByTimeAsync(900_000);

      expect(ws.send.mock.calls.length).toBe(sendCallsBeforeAdvance);
    });

    it('falls back to a full re-authenticate when refresh itself fails', async () => {
      client = new WebSocketClient({ url: TEST_URL });
      client.connect();
      const ws = MockWebSocket.lastInstance!;
      await openAndAuthenticate(ws);

      await vi.advanceTimersByTimeAsync(780_001);
      // Refresh request times out / errors — never respond to it, just let
      // its own request timeout (3s, per sendRequestWithResponse) elapse.
      await vi.advanceTimersByTimeAsync(5000);
      await flush();

      const sentTypes = ws.send.mock.calls.map((call) => {
        const decoded = decodeEnvelope(call[0]);
        return Object.keys(decoded).find((k) => k !== 'sequenceNumber');
      });
      // Original handshake + refresh attempt + a fresh challengeRequest
      // from the fallback re-authenticate.
      expect(sentTypes.filter((t) => t === 'challengeRequest').length).toBeGreaterThanOrEqual(2);
    });
  });

  describe('Session Operations', () => {
    it('should send attach request', async () => {
      client = new WebSocketClient({ url: TEST_URL });
      client.connect();
      const ws = MockWebSocket.lastInstance!;
      await openAndAuthenticate(ws);

      client.attach('', 24, 80);

      expect(ws?.send).toHaveBeenCalled();
    });

    it('should send input data', async () => {
      client = new WebSocketClient({ url: TEST_URL });
      client.connect();
      const ws = MockWebSocket.lastInstance!;
      await openAndAuthenticate(ws);

      client.sendInput('ls -la\n');

      expect(ws?.send).toHaveBeenCalled();
    });

    it('should send resize request', async () => {
      client = new WebSocketClient({ url: TEST_URL });
      client.connect();
      const ws = MockWebSocket.lastInstance!;
      await openAndAuthenticate(ws);

      client.resize(40, 160);

      expect(ws?.send).toHaveBeenCalled();
    });

    it('should track sessionId', () => {
      client = new WebSocketClient({ url: TEST_URL });
      expect(client.getSessionId()).toBe('');

      client.attach('test-session-123', 24, 80);

      expect(client.getSessionId()).toBe('test-session-123');
    });

    it('should clear sessionId on detach', async () => {
      client = new WebSocketClient({ url: TEST_URL });
      client.connect();
      const ws = MockWebSocket.lastInstance!;
      await openAndAuthenticate(ws);

      client.attach('session-789', 24, 80);
      expect(client.getSessionId()).toBe('session-789');

      client.detach();
      expect(client.getSessionId()).toBe('');
    });
  });

  describe('State Listeners', () => {
    it('should notify state listeners', () => {
      client = new WebSocketClient({ url: TEST_URL });
      const listener = vi.fn();

      client.onStateChange(listener);
      client.connect();

      expect(listener).toHaveBeenCalledWith(ConnectionState.CONNECTING);
    });

    it('should allow unsubscribing', () => {
      client = new WebSocketClient({ url: TEST_URL });
      const listener = vi.fn();

      const unsubscribe = client.onStateChange(listener);
      unsubscribe();

      client.connect();

      expect(listener).not.toHaveBeenCalled();
    });
  });

  describe('Message Handlers', () => {
    it('should set message handlers', () => {
      client = new WebSocketClient({ url: TEST_URL });
      const handlers = {
        onAttachResponse: vi.fn(),
        onOutputData: vi.fn(),
        onErrorResponse: vi.fn(),
      };

      client.setHandlers(handlers);

      // Handlers are set successfully (verified by no errors)
      expect(client).toBeDefined();
    });

    it('rejects a pending request on errorResponse even with no onErrorResponse handler registered', async () => {
      // Regression test: this used to only reject the pending request when
      // an onErrorResponse handler was ALSO set — a caller with none (e.g.
      // local-daemon.ts's probe client) never got its promise rejected and
      // just hung until the request's own timeout.
      client = new WebSocketClient({ url: TEST_URL });
      client.connect();
      const ws = MockWebSocket.lastInstance!;
      await openAndAuthenticate(ws);

      const pending = client.sendChallengeRequest();
      const sentSeq = decodeEnvelope(ws.send.mock.calls[ws.send.mock.calls.length - 1][0])
        .sequenceNumber as number;
      ws.onmessage?.({
        data: encodeTestEnvelope({ errorResponse: { code: 6, message: 'boom' } }, sentSeq),
      } as MessageEvent);

      await expect(pending).rejects.toThrow('boom');
    });
  });
});
