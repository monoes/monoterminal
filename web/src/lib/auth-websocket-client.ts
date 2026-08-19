/**
 * Integrated WebSocket client with automatic authentication
 * Combines WebSocketClient with AuthService for seamless JWT lifecycle
 * Phase 3: Token Lifecycle Integration
 */

import { WebSocketClient, ConnectionConfig, ConnectionState, type MessageHandler } from './websocket-client';
import { AuthService } from './auth';

export interface AuthWebSocketConfig extends Omit<ConnectionConfig, 'jwtAuth'> {
  keypairId?: string;
  autoAuthenticate?: boolean;
  autoRefresh?: boolean;
  refreshBeforeExpiry?: number; // Refresh N seconds before expiry (default: 60s)
  onAuthError?: (error: AuthError) => void;
  onAuthSuccess?: () => void;
  maxAuthRetries?: number; // Max retries for auth failures (default: 3)
  devMode?: boolean; // Dev mode: skip auth, backend auto-issues JWT (default: false)
}

export enum AuthErrorCode {
  CHALLENGE_EXPIRED = 'CHALLENGE_EXPIRED',
  INVALID_SIGNATURE = 'INVALID_SIGNATURE',
  REFRESH_FAILED = 'REFRESH_FAILED',
  CONNECTION_LOST = 'CONNECTION_LOST',
  AUTH_TIMEOUT = 'AUTH_TIMEOUT',
  MAX_RETRIES_EXCEEDED = 'MAX_RETRIES_EXCEEDED',
}

export interface AuthError {
  code: AuthErrorCode;
  message: string;
  canRetry: boolean;
  originalError?: any;
}

/**
 * WebSocket client with integrated authentication lifecycle
 */
export class AuthWebSocketClient {
  private client: WebSocketClient;
  private auth: AuthService;
  private config: Required<AuthWebSocketConfig>;
  private refreshTimer: number | null = null;
  private isAuthenticating = false;
  private currentRefreshAuth = '';
  private authRetries = 0;
  private reconnectUnsubscribe: (() => void) | null = null;

  constructor(config: AuthWebSocketConfig) {
    this.config = {
      keypairId: 'default',
      autoAuthenticate: true,
      autoRefresh: true,
      refreshBeforeExpiry: 60,
      maxAuthRetries: 3,
      devMode: false,
      ...config,
    };

    // Create base WebSocket client without JWT (we'll manage it)
    this.client = new WebSocketClient({
      url: config.url,
      autoReconnect: config.autoReconnect,
      reconnectInterval: config.reconnectInterval,
      maxReconnectAttempts: config.maxReconnectAttempts,
      jwtAuth: '',
    });

    this.auth = new AuthService();
  }

  /**
   * Initialize auth service and connect
   * IMPORTANT: If autoAuthenticate is enabled, this method will not resolve
   * until authentication is complete. This ensures auth_token is available
   * before any attach/input/resize operations.
   */
  async connect(): Promise<void> {
    // Dev mode: skip auth initialization
    if (this.config.devMode) {
      console.log('Dev mode enabled - skipping authentication (backend auto-issues JWT)');
      this.client.connect();
      await this.waitForConnection();
      this.config.onAuthSuccess?.();
      return;
    }

    // Initialize auth service (loads or generates keypair)
    await this.auth.initialize(this.config.keypairId);

    // Setup reconnection handler
    this.setupReconnectionHandler();

    // Connect WebSocket
    this.client.connect();

    // Wait for connection
    await this.waitForConnection();

    // Auto-authenticate if enabled
    // CRITICAL: Wait for authentication to complete before resolving
    // This prevents race condition where attach() is called before JWT is available
    if (this.config.autoAuthenticate) {
      console.log('Auto-authenticating before resolving connect()...');
      await this.authenticate();
      console.log('Authentication complete, connect() resolved');
    }
  }

  /**
   * Perform full authentication flow with error handling and retries
   */
  async authenticate(): Promise<void> {
    if (this.isAuthenticating) {
      throw new Error('Authentication already in progress');
    }

    this.isAuthenticating = true;

    try {
      // Step 1: Request challenge
      const challengeResponse = await this.client.sendChallengeRequest().catch((error) => {
        throw this.handleAuthError(AuthErrorCode.AUTH_TIMEOUT, 'Challenge request timeout', error);
      });

      // Check challenge expiry
      const now = Math.floor(Date.now() / 1000);
      if (challengeResponse.expiresAt < now) {
        throw this.handleAuthError(
          AuthErrorCode.CHALLENGE_EXPIRED,
          'Challenge expired before signing',
          null
        );
      }

      // Step 2: Sign challenge
      const signature = await this.auth.signChallenge({
        nonce: challengeResponse.nonce,
        expiresAt: challengeResponse.expiresAt,
      }).catch((error) => {
        throw this.handleAuthError(AuthErrorCode.INVALID_SIGNATURE, 'Challenge signing failed', error);
      });

      // Step 3: Send auth request
      const authResponse = await this.client.sendAuthRequest({
        signature: this.base64ToBytes(signature.signature),
        publicKey: this.base64ToBytes(signature.publicKey),
        nonce: challengeResponse.nonce,
      }).catch((error) => {
        // Parse error to determine if it's an invalid signature
        const errorMsg = error?.message || '';
        if (errorMsg.includes('signature') || errorMsg.includes('invalid')) {
          throw this.handleAuthError(AuthErrorCode.INVALID_SIGNATURE, 'Invalid signature', error);
        }
        throw this.handleAuthError(AuthErrorCode.AUTH_TIMEOUT, 'Auth request failed', error);
      });

      // Step 4: Store JWT in auth service
      const expiresIn = authResponse.accessExpiresAt - Math.floor(Date.now() / 1000);
      this.auth.setJWT(authResponse.accessToken, expiresIn);

      // Store refresh credentials
      this.currentRefreshAuth = authResponse.refreshToken;

      // Step 5: Update WebSocket client config with JWT
      this.updateClientJWT(authResponse.accessToken);

      // Step 6: Schedule auto-refresh if enabled
      if (this.config.autoRefresh) {
        this.scheduleRefresh(expiresIn);
      }

      // Reset retry counter on success
      this.authRetries = 0;

      console.log('Authentication successful');
      this.config.onAuthSuccess?.();
    } catch (error) {
      // Handle retries for transient errors
      if (error instanceof Error && (error as any).canRetry) {
        this.authRetries++;
        if (this.authRetries < (this.config.maxAuthRetries || 3)) {
          console.log(`Auth failed, retrying (${this.authRetries}/${this.config.maxAuthRetries})...`);
          this.isAuthenticating = false;
          return this.authenticate();
        } else {
          const maxRetriesError = this.handleAuthError(
            AuthErrorCode.MAX_RETRIES_EXCEEDED,
            `Max auth retries (${this.config.maxAuthRetries}) exceeded`,
            error
          );
          this.config.onAuthError?.(maxRetriesError);
          throw maxRetriesError;
        }
      }

      this.config.onAuthError?.(error as AuthError);
      throw error;
    } finally {
      this.isAuthenticating = false;
    }
  }

  /**
   * Manually refresh the access credentials with error handling
   */
  async refresh(): Promise<void> {
    if (!this.currentRefreshAuth) {
      const error = this.handleAuthError(
        AuthErrorCode.REFRESH_FAILED,
        'No refresh credentials available. Must authenticate first.',
        null
      );
      this.config.onAuthError?.(error);
      throw error;
    }

    try {
      const refreshResponse = await this.client.refreshJWT(this.currentRefreshAuth);

      // Update stored JWT
      const expiresIn = refreshResponse.accessExpiresAt - Math.floor(Date.now() / 1000);
      this.auth.setJWT(refreshResponse.accessToken, expiresIn);

      // Update refresh credentials (they rotate)
      this.currentRefreshAuth = refreshResponse.refreshToken;

      // Update WebSocket client
      this.updateClientJWT(refreshResponse.accessToken);

      // Reschedule next refresh
      if (this.config.autoRefresh) {
        this.scheduleRefresh(expiresIn);
      }

      console.log('JWT refreshed successfully');
    } catch (error) {
      const refreshError = this.handleAuthError(
        AuthErrorCode.REFRESH_FAILED,
        'JWT refresh failed',
        error
      );
      this.config.onAuthError?.(refreshError);

      // On refresh failure, re-authenticate
      if (this.config.autoAuthenticate) {
        console.log('Refresh failed, attempting re-authentication...');
        try {
          await this.authenticate();
        } catch (authError) {
          // If re-auth also fails, throw the original refresh error
          throw refreshError;
        }
      } else {
        throw refreshError;
      }
    }
  }

  /**
   * Disconnect and clear auth state
   */
  disconnect(): void {
    if (this.refreshTimer !== null) {
      clearTimeout(this.refreshTimer);
      this.refreshTimer = null;
    }

    if (this.reconnectUnsubscribe) {
      this.reconnectUnsubscribe();
      this.reconnectUnsubscribe = null;
    }

    this.auth.clearJWT();
    this.currentRefreshAuth = '';
    this.authRetries = 0;
    this.client.disconnect();
  }

  /**
   * Get the underlying WebSocket client for terminal operations
   * @deprecated Use attach/sendInput/resize methods on AuthWebSocketClient instead
   */
  getClient(): WebSocketClient {
    return this.client;
  }

  /**
   * Attach to a session (with auth enforcement)
   */
  attach(sessionId: string, rows: number, cols: number): void {
    if (!this.auth.isAuthenticated() && !this.config.devMode) {
      throw new Error('Cannot attach to session: not authenticated. Call authenticate() first or enable devMode.');
    }
    this.client.attach(sessionId, rows, cols);
  }

  /**
   * Send terminal input (with auth enforcement)
   */
  sendInput(data: string | Uint8Array): void {
    if (!this.auth.isAuthenticated() && !this.config.devMode) {
      throw new Error('Cannot send input: not authenticated. Call authenticate() first or enable devMode.');
    }
    this.client.sendInput(data);
  }

  /**
   * Send resize request (with auth enforcement)
   */
  resize(rows: number, cols: number): void {
    if (!this.auth.isAuthenticated() && !this.config.devMode) {
      throw new Error('Cannot resize: not authenticated. Call authenticate() first or enable devMode.');
    }
    this.client.resize(rows, cols);
  }

  /**
   * Detach from session
   */
  detach(): void {
    this.client.detach();
  }

  /**
   * Get the auth service for manual operations
   */
  getAuth(): AuthService {
    return this.auth;
  }

  /**
   * Check if currently authenticated
   */
  isAuthenticated(): boolean {
    return this.auth.isAuthenticated();
  }

  /**
   * Get connection state
   */
  getState(): ConnectionState {
    return this.client.getState();
  }

  /**
   * Set message handlers
   */
  setHandlers(handlers: MessageHandler): void {
    this.client.setHandlers(handlers);
  }

  /**
   * Listen for state changes
   */
  onStateChange(listener: (state: ConnectionState) => void): () => void {
    return this.client.onStateChange(listener);
  }

  private waitForConnection(): Promise<void> {
    return new Promise((resolve, reject) => {
      if (this.client.getState() === ConnectionState.CONNECTED) {
        resolve();
        return;
      }

      const cleanup = this.client.onStateChange((state) => {
        if (state === ConnectionState.CONNECTED) {
          cleanup();
          resolve();
        } else if (state === ConnectionState.ERROR || state === ConnectionState.DISCONNECTED) {
          cleanup();
          reject(new Error(`Connection failed: ${state}`));
        }
      });

      // Timeout after 10s
      setTimeout(() => {
        cleanup();
        reject(new Error('Connection timeout'));
      }, 10000);
    });
  }

  private updateClientJWT(jwt: string): void {
    // Update the client's config (accessing private via type assertion)
    (this.client as any).config.jwtAuth = jwt;
  }

  private scheduleRefresh(expiresIn: number): void {
    if (this.refreshTimer !== null) {
      clearTimeout(this.refreshTimer);
    }

    // Schedule refresh before expiry
    const refreshIn = Math.max(0, expiresIn - this.config.refreshBeforeExpiry);
    const refreshMs = refreshIn * 1000;

    console.log(`Scheduling JWT refresh in ${refreshIn}s`);

    this.refreshTimer = window.setTimeout(() => {
      this.refreshTimer = null;
      this.refresh().catch((error) => {
        console.error('Auto-refresh failed:', error);
      });
    }, refreshMs);
  }

  private base64ToBytes(base64: string): Uint8Array {
    const binaryString = atob(base64);
    const bytes = new Uint8Array(binaryString.length);
    for (let i = 0; i < binaryString.length; i++) {
      bytes[i] = binaryString.charCodeAt(i);
    }
    return bytes;
  }

  private handleAuthError(code: AuthErrorCode, message: string, originalError: any): AuthError {
    const canRetry = code === AuthErrorCode.AUTH_TIMEOUT || code === AuthErrorCode.CHALLENGE_EXPIRED;
    const error: AuthError = {
      code,
      message,
      canRetry,
      originalError,
    };
    console.error(`Auth error [${code}]:`, message, originalError);
    return error;
  }

  private setupReconnectionHandler(): void {
    // Skip in dev mode
    if (this.config.devMode) {
      return;
    }

    // Clean up existing listener
    if (this.reconnectUnsubscribe) {
      this.reconnectUnsubscribe();
    }

    // Listen for reconnection events
    this.reconnectUnsubscribe = this.client.onStateChange(async (state) => {
      if (state === ConnectionState.CONNECTED && this.config.autoAuthenticate) {
        // On reconnect, re-authenticate if we had a session before
        if (this.auth.isAuthenticated() || this.authRetries > 0) {
          console.log('WebSocket reconnected, re-authenticating...');
          try {
            await this.authenticate();
          } catch (error) {
            console.error('Re-authentication after reconnect failed:', error);
            const connError = this.handleAuthError(
              AuthErrorCode.CONNECTION_LOST,
              'Re-authentication failed after reconnection',
              error
            );
            this.config.onAuthError?.(connError);
          }
        }
      } else if (state === ConnectionState.DISCONNECTED || state === ConnectionState.ERROR) {
        // Clear refresh timer on disconnect
        if (this.refreshTimer !== null) {
          clearTimeout(this.refreshTimer);
          this.refreshTimer = null;
        }
      }
    });
  }
}

/**
 * Create and initialize an authenticated WebSocket client
 * Convenience function for common use case
 */
export async function createAuthWebSocketClient(
  config: AuthWebSocketConfig
): Promise<AuthWebSocketClient> {
  const client = new AuthWebSocketClient(config);
  await client.connect();
  return client;
}
