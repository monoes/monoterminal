/**
 * A single, app-wide `AuthService` instance — not one per `WebSocketClient`.
 *
 * `HybridTransport` builds a fresh `WebSocketClient` on every connect
 * attempt, every failover, and every background route-watch probe (see
 * hybrid-transport.ts). A per-instance `AuthService` would call
 * `loadOrGenerateKeypair()` on each of those; even though IndexedDB
 * persistence means it would usually just *load* the existing key rather
 * than generate a new one, a cold-IndexedDB race between two concurrent
 * probes could interleave a generate-and-store and produce two different
 * `ed25519:...` identities for what should be one stable one. The
 * singleton also lets the current JWT survive a route failover, since
 * `AuthService` deliberately keeps it in memory only (see its own doc
 * comments) rather than persisting it.
 *
 * Mirrors the same singleton rationale as local-daemon.ts.
 */

import { createAuthService, type AuthService } from './index';

let cached: Promise<AuthService> | null = null;

export function getAuthService(): Promise<AuthService> {
  if (!cached) {
    cached = createAuthService();
  }
  return cached;
}
