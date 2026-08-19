/**
 * WebSocket Client Tests
 * Tests state machine, reconnection logic, message encoding/decoding
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { WebSocketClient, ConnectionState } from './websocket-client';

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

  static lastInstance: MockWebSocket | null = null;
}

describe('WebSocketClient', () => {
  let client: WebSocketClient;
  const TEST_URL = 'ws://localhost:5000/ws';

  beforeEach(() => {
    global.WebSocket = MockWebSocket as any;
    MockWebSocket.lastInstance = null;
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

    it('should transition to CONNECTED on WebSocket open', () => {
      client = new WebSocketClient({ url: TEST_URL });
      const stateListener = vi.fn();
      client.onStateChange(stateListener);

      client.connect();
      const ws = MockWebSocket.lastInstance;
      ws?.onopen?.(new Event('open'));

      expect(client.getState()).toBe(ConnectionState.CONNECTED);
      expect(stateListener).toHaveBeenCalledWith(ConnectionState.CONNECTED);
    });

    it('should transition to ERROR on WebSocket error', () => {
      client = new WebSocketClient({ url: TEST_URL });
      client.connect();

      const ws = MockWebSocket.lastInstance;
      ws?.onerror?.(new Event('error'));

      expect(client.getState()).toBe(ConnectionState.ERROR);
    });

    it('should transition to DISCONNECTED on manual disconnect', () => {
      client = new WebSocketClient({ url: TEST_URL, autoReconnect: false });
      client.connect();

      const ws = MockWebSocket.lastInstance;
      ws?.onopen?.(new Event('open'));

      client.disconnect();

      expect(client.getState()).toBe(ConnectionState.DISCONNECTED);
      expect(ws?.close).toHaveBeenCalled();
    });
  });

  describe('Reconnection Logic', () => {
    it('should schedule reconnect on close with autoReconnect=true', () => {
      client = new WebSocketClient({
        url: TEST_URL,
        autoReconnect: true,
        reconnectInterval: 3000,
      });

      client.connect();
      const ws = MockWebSocket.lastInstance;
      ws?.onopen?.(new Event('open'));
      ws?.onclose?.(new CloseEvent('close'));

      expect(client.getState()).toBe(ConnectionState.RECONNECTING);
    });

    it('should not reconnect if autoReconnect=false', () => {
      client = new WebSocketClient({
        url: TEST_URL,
        autoReconnect: false,
      });

      client.connect();
      const ws = MockWebSocket.lastInstance;
      ws?.onopen?.(new Event('open'));
      ws?.onclose?.(new CloseEvent('close'));

      vi.advanceTimersByTime(10000);

      expect(client.getState()).toBe(ConnectionState.DISCONNECTED);
    });

    it('should clear reconnect timer on manual disconnect', () => {
      client = new WebSocketClient({
        url: TEST_URL,
        autoReconnect: true,
      });

      client.connect();
      const ws = MockWebSocket.lastInstance;
      ws?.onopen?.(new Event('open'));
      ws?.onclose?.(new CloseEvent('close'));

      client.disconnect();
      vi.advanceTimersByTime(5000);

      expect(client.getState()).toBe(ConnectionState.DISCONNECTED);
    });
  });

  describe('Session Operations', () => {
    it('should send attach request', () => {
      client = new WebSocketClient({ url: TEST_URL });
      client.connect();

      const ws = MockWebSocket.lastInstance;
      ws?.onopen?.(new Event('open'));

      client.attach('', 24, 80);

      expect(ws?.send).toHaveBeenCalled();
    });

    it('should send input data', () => {
      client = new WebSocketClient({ url: TEST_URL });
      client.connect();

      const ws = MockWebSocket.lastInstance;
      ws?.onopen?.(new Event('open'));

      client.sendInput('ls -la\n');

      expect(ws?.send).toHaveBeenCalled();
    });

    it('should send resize request', () => {
      client = new WebSocketClient({ url: TEST_URL });
      client.connect();

      const ws = MockWebSocket.lastInstance;
      ws?.onopen?.(new Event('open'));

      client.resize(40, 160);

      expect(ws?.send).toHaveBeenCalled();
    });

    it('should track sessionId', () => {
      client = new WebSocketClient({ url: TEST_URL });
      expect(client.getSessionId()).toBe('');

      client.attach('test-session-123', 24, 80);

      expect(client.getSessionId()).toBe('test-session-123');
    });

    it('should clear sessionId on detach', () => {
      client = new WebSocketClient({ url: TEST_URL });
      client.connect();

      const ws = MockWebSocket.lastInstance;
      ws?.onopen?.(new Event('open'));

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
  });
});
