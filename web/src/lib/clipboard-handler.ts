/**
 * Clipboard Handler for bidirectional OSC 52 support (ADR-020)
 * Phase 4: Advanced clipboard features
 */

export interface ClipboardGetResponse {
  requestId: string;
  content: string;
  mimeType: string;
  authorized: boolean;
  error?: string;
}

export interface ClipboardPermissionModalProps {
  sessionId: string;
  onAllow: () => void;
  onDeny: () => void;
}

export type ShowPermissionModal = (
  sessionId: string
) => Promise<boolean>;

export interface ClipboardHandlerConfig {
  showPermissionModal: ShowPermissionModal;
  onClipboardResponse: (response: ClipboardGetResponse) => void;
  sessionId: string;
}

export class ClipboardHandler {
  private sessionRemembered = false; // Session-scoped "Remember" checkbox
  private rememberedAt: number | null = null;
  private lastActivityAt: number = Date.now();
  private pendingRequests = new Map<
    string,
    { resolve: (authorized: boolean) => void; timeout: NodeJS.Timeout }
  >();
  private config: ClipboardHandlerConfig;

  // Rate limiting: Max 3 read requests per 60 seconds (Security control #1)
  private readRequestCount = 0;
  private readRequestResetTimer: NodeJS.Timeout | null = null;
  private static readonly MAX_REQUESTS_PER_WINDOW = 3;
  private static readonly RATE_LIMIT_WINDOW_MS = 60000; // 60 seconds, not 10

  // Exponential backoff on denials (Security control #1)
  private denyCount = 0;
  private cooldownUntil: number = 0;
  private static readonly BACKOFF_SCHEDULE = [10000, 30000, 60000]; // 10s, 30s, 60s

  // Request timeout: 30 seconds
  private static readonly REQUEST_TIMEOUT_MS = 30000;

  // Session remember timeout: 30 minutes of inactivity (Security control #3)
  private static readonly SESSION_REMEMBER_TIMEOUT_MS = 30 * 60 * 1000;

  // Clipboard size limit: 1MB (Security control #5)
  private static readonly MAX_CLIPBOARD_SIZE_BYTES = 1024 * 1024;

  // Frequency tracking for modal warnings (Security control #6)
  private recentRequests: number[] = [];

  constructor(config: ClipboardHandlerConfig) {
    this.config = config;
  }

  /**
   * Handle OSC 52 write (server → client clipboard)
   * \e]52;c;<base64>\e\
   */
  async handleOSC52Write(content: string, selection: string): Promise<boolean> {
    if (selection !== 'c') {
      console.warn(
        `[ClipboardHandler] Only clipboard selection supported, ignoring: ${selection}`
      );
      return false;
    }

    if (!navigator.clipboard) {
      console.error('[ClipboardHandler] Clipboard API not available (HTTPS required)');
      return this.fallbackCopyToClipboard(content);
    }

    try {
      await navigator.clipboard.writeText(content);
      console.log('[ClipboardHandler] Clipboard write successful');
      return true;
    } catch (err) {
      console.error('[ClipboardHandler] Clipboard write failed:', err);
      // Fallback to execCommand if available
      return this.fallbackCopyToClipboard(content);
    }
  }

  /**
   * Handle clipboard read request (server queries client clipboard)
   * \e]52;c;?\e\
   *
   * Security control #2: Server-initiated read (requires modal)
   */
  async handleClipboardGetRequest(requestId: string): Promise<void> {
    this.lastActivityAt = Date.now();
    this.trackRequestFrequency();

    // Check exponential backoff cooldown (Security control #1)
    if (this.isInCooldown()) {
      const remainingMs = this.cooldownUntil - Date.now();
      console.warn(`[ClipboardHandler] In cooldown for ${Math.ceil(remainingMs / 1000)}s`);
      this.sendClipboardResponse({
        requestId,
        content: '',
        mimeType: 'text/plain',
        authorized: false,
        error: `Too many denials. Try again in ${Math.ceil(remainingMs / 1000)} seconds.`,
      });
      return;
    }

    // Rate limiting check (Security control #1)
    if (!this.checkRateLimit()) {
      console.warn('[ClipboardHandler] Rate limit exceeded for clipboard reads');
      this.sendClipboardResponse({
        requestId,
        content: '',
        mimeType: 'text/plain',
        authorized: false,
        error: 'Rate limit exceeded (max 3 requests per minute)',
      });
      return;
    }

    // Check session remember + inactivity timeout (Security control #3)
    if (this.sessionRemembered && this.isSessionRememberValid()) {
      console.log('[ClipboardHandler] Using session-scoped remember permission');
      await this.readAndSendClipboard(requestId);
      return;
    }

    // Request user authorization
    let userGranted = false;
    try {
      userGranted = await this.requestUserAuthorization(requestId);
    } catch (err) {
      console.error('[ClipboardHandler] Authorization request failed:', err);
      this.sendClipboardResponse({
        requestId,
        content: '',
        mimeType: 'text/plain',
        authorized: false,
        error: 'Authorization request failed',
      });
      return;
    }

    if (!userGranted) {
      console.log('[ClipboardHandler] User denied clipboard access');
      this.handleDenial(); // Exponential backoff
      this.sendClipboardResponse({
        requestId,
        content: '',
        mimeType: 'text/plain',
        authorized: false,
        error: 'User denied clipboard access',
      });
      return;
    }

    // User approved - reset deny count
    this.denyCount = 0;
    this.cooldownUntil = 0;

    await this.readAndSendClipboard(requestId);
  }

  /**
   * Read clipboard and send response with size validation
   * Security control #5: 1MB size limit
   */
  private async readAndSendClipboard(requestId: string): Promise<void> {
    if (!navigator.clipboard) {
      this.sendClipboardResponse({
        requestId,
        content: '',
        mimeType: 'text/plain',
        authorized: false,
        error: 'Clipboard API not available (HTTPS required)',
      });
      return;
    }

    try {
      const text = await navigator.clipboard.readText();

      // Size validation (Security control #5)
      const sizeBytes = new TextEncoder().encode(text).length;
      if (sizeBytes > ClipboardHandler.MAX_CLIPBOARD_SIZE_BYTES) {
        const sizeMB = (sizeBytes / 1024 / 1024).toFixed(2);
        console.warn(`[ClipboardHandler] Clipboard too large: ${sizeMB} MB`);
        this.sendClipboardResponse({
          requestId,
          content: '',
          mimeType: 'text/plain',
          authorized: false,
          error: `Clipboard too large (${sizeMB} MB). Maximum: 1 MB. Paste directly instead.`,
        });
        return;
      }

      this.sendClipboardResponse({
        requestId,
        content: text,
        mimeType: 'text/plain',
        authorized: true,
      });
      console.log(`[ClipboardHandler] Clipboard read successful (${sizeBytes} bytes)`);
    } catch (err) {
      console.error('[ClipboardHandler] Clipboard read failed:', err);
      this.sendClipboardResponse({
        requestId,
        content: '',
        mimeType: 'text/plain',
        authorized: false,
        error: err instanceof Error ? err.message : 'Unknown error',
      });
    }
  }

  /**
   * Request user authorization with timeout
   */
  private async requestUserAuthorization(requestId: string): Promise<boolean> {
    return new Promise((resolve) => {
      const timeout = setTimeout(() => {
        this.pendingRequests.delete(requestId);
        console.warn('[ClipboardHandler] Authorization request timed out');
        resolve(false);
      }, ClipboardHandler.REQUEST_TIMEOUT_MS);

      this.pendingRequests.set(requestId, { resolve, timeout });

      // Show permission modal (provided by React component)
      this.config
        .showPermissionModal(this.config.sessionId)
        .then((granted) => {
          const pending = this.pendingRequests.get(requestId);
          if (pending) {
            clearTimeout(pending.timeout);
            this.pendingRequests.delete(requestId);
            resolve(granted);
          }
        })
        .catch((err) => {
          console.error('[ClipboardHandler] Permission modal error:', err);
          const pending = this.pendingRequests.get(requestId);
          if (pending) {
            clearTimeout(pending.timeout);
            this.pendingRequests.delete(requestId);
          }
          resolve(false);
        });
    });
  }

  /**
   * Send clipboard response to server
   */
  private sendClipboardResponse(response: ClipboardGetResponse): void {
    this.config.onClipboardResponse(response);
  }

  /**
   * Rate limiting: Check if request is allowed
   */
  private checkRateLimit(): boolean {
    // Reset counter after window expires
    if (this.readRequestResetTimer === null) {
      this.readRequestResetTimer = setTimeout(() => {
        this.readRequestCount = 0;
        this.readRequestResetTimer = null;
      }, ClipboardHandler.RATE_LIMIT_WINDOW_MS);
    }

    if (this.readRequestCount >= ClipboardHandler.MAX_REQUESTS_PER_WINDOW) {
      return false; // Rate limit exceeded
    }

    this.readRequestCount++;
    return true;
  }

  /**
   * Fallback: Copy to clipboard using execCommand (legacy browsers)
   */
  private fallbackCopyToClipboard(text: string): boolean {
    const textarea = document.createElement('textarea');
    textarea.value = text;
    textarea.style.position = 'fixed';
    textarea.style.opacity = '0';
    document.body.appendChild(textarea);
    textarea.select();

    let success = false;
    try {
      success = document.execCommand('copy');
      console.log('[ClipboardHandler] Fallback copy successful');
    } catch (err) {
      console.error('[ClipboardHandler] Fallback copy failed:', err);
    }

    document.body.removeChild(textarea);
    return success;
  }

  /**
   * Detect platform capabilities
   */
  static getCapabilities(): {
    hasClipboardAPI: boolean;
    isIOSSafari: boolean;
    requiresGesture: boolean;
  } {
    const hasClipboardAPI = !!navigator.clipboard;
    const isIOSSafari = /iPhone|iPad|iPod/.test(navigator.userAgent) && /Safari/.test(navigator.userAgent);
    const requiresGesture = isIOSSafari;

    return {
      hasClipboardAPI,
      isIOSSafari,
      requiresGesture,
    };
  }

  /**
   * Security control #3: Enable session-scoped remember
   */
  enableSessionRemember(): void {
    this.sessionRemembered = true;
    this.rememberedAt = Date.now();
    this.lastActivityAt = Date.now();
    console.log('[ClipboardHandler] Session remember enabled');
  }

  /**
   * Security control #3: Revoke session-scoped remember
   */
  revokeSessionRemember(): void {
    this.sessionRemembered = false;
    this.rememberedAt = null;
    console.log('[ClipboardHandler] Session remember revoked');
  }

  /**
   * Check if session remember is still valid (30-minute inactivity timeout)
   */
  private isSessionRememberValid(): boolean {
    if (!this.sessionRemembered || !this.rememberedAt) {
      return false;
    }

    const inactiveMs = Date.now() - this.lastActivityAt;
    if (inactiveMs > ClipboardHandler.SESSION_REMEMBER_TIMEOUT_MS) {
      console.log('[ClipboardHandler] Session remember expired due to inactivity');
      this.revokeSessionRemember();
      return false;
    }

    return true;
  }

  /**
   * Get session remember status (for UI indicator)
   */
  getSessionRememberStatus(): {
    active: boolean;
    remainingMinutes: number | null;
  } {
    if (!this.sessionRemembered || !this.rememberedAt) {
      return { active: false, remainingMinutes: null };
    }

    const inactiveMs = Date.now() - this.lastActivityAt;
    const remainingMs = ClipboardHandler.SESSION_REMEMBER_TIMEOUT_MS - inactiveMs;
    const remainingMinutes = Math.ceil(remainingMs / 60000);

    return {
      active: remainingMs > 0,
      remainingMinutes: Math.max(0, remainingMinutes),
    };
  }

  /**
   * Security control #1: Handle denial with exponential backoff
   */
  private handleDenial(): void {
    this.denyCount++;
    const backoffIndex = Math.min(this.denyCount - 1, ClipboardHandler.BACKOFF_SCHEDULE.length - 1);
    const backoffMs = ClipboardHandler.BACKOFF_SCHEDULE[backoffIndex];
    this.cooldownUntil = Date.now() + backoffMs;

    console.log(
      `[ClipboardHandler] Denial #${this.denyCount} - cooldown ${backoffMs / 1000}s until ${new Date(this.cooldownUntil).toLocaleTimeString()}`
    );
  }

  /**
   * Check if currently in exponential backoff cooldown
   */
  private isInCooldown(): boolean {
    return Date.now() < this.cooldownUntil;
  }

  /**
   * Security control #6: Track request frequency for modal warnings
   */
  private trackRequestFrequency(): void {
    const now = Date.now();
    // Keep only requests from last 5 minutes
    this.recentRequests = this.recentRequests.filter((t) => now - t < 5 * 60 * 1000);
    this.recentRequests.push(now);
  }

  /**
   * Get frequency warning status for modal
   * Security control #6: Warn if >3 requests in 5 minutes
   */
  getFrequencyWarning(): { shouldWarn: boolean; requestCount: number } {
    const now = Date.now();
    const recentCount = this.recentRequests.filter((t) => now - t < 5 * 60 * 1000).length;
    return {
      shouldWarn: recentCount > 3,
      requestCount: recentCount,
    };
  }

  /**
   * Cleanup
   */
  dispose(): void {
    // Clear pending requests
    this.pendingRequests.forEach(({ timeout }) => clearTimeout(timeout));
    this.pendingRequests.clear();

    // Clear rate limit timer
    if (this.readRequestResetTimer) {
      clearTimeout(this.readRequestResetTimer);
      this.readRequestResetTimer = null;
    }

    // Revoke session remember
    this.revokeSessionRemember();
  }
}
