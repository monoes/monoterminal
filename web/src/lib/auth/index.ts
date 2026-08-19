/**
 * Browser Ed25519 Authentication Service
 * SRS §3.2.2: Client-side authentication implementation
 *
 * Main entry point for authentication operations:
 * - Keypair generation and storage
 * - Challenge-response signing
 * - JWT token management
 */

import {
  loadOrGenerateKeypair,
  generateKeypair,
  getPublicKeyFingerprint,
  publicKeyToBase64,
  type Ed25519Keypair,
} from './keys';
import {
  signChallenge,
  serializeChallengeResponse,
  parseChallenge,
  isChallengeExpired,
  type Challenge,
  type ChallengeResponse,
} from './challenge';
import { deleteKeypair } from './storage';

// Re-export types and utilities
export type { Ed25519Keypair, Challenge, ChallengeResponse };
export {
  generateKeypair,
  getPublicKeyFingerprint,
  publicKeyToBase64,
  parseChallenge,
  isChallengeExpired,
};

/**
 * Authentication service for Ed25519 challenge-response flow
 */
export class AuthService {
  private keypair: Ed25519Keypair | null = null;
  private jwt: string | null = null;
  private jwtExpiresAt: number | null = null;

  /**
   * Initialize auth service and load/generate keypair
   */
  async initialize(keypairId: string = 'default'): Promise<void> {
    this.keypair = await loadOrGenerateKeypair(keypairId);
    console.log('Auth service initialized');
  }

  /**
   * Get the public key (for display/transmission)
   */
  getPublicKey(): Uint8Array {
    if (!this.keypair) {
      throw new Error('Auth service not initialized. Call initialize() first.');
    }
    return this.keypair.publicKey;
  }

  /**
   * Get the public key fingerprint (hex string)
   */
  getPublicKeyFingerprint(): string {
    return getPublicKeyFingerprint(this.getPublicKey());
  }

  /**
   * Get the public key as base64 (for transmission)
   */
  getPublicKeyBase64(): string {
    return publicKeyToBase64(this.getPublicKey());
  }

  /**
   * Sign a challenge received from the server
   *
   * @param challengeData - Challenge data from server (JSON)
   * @returns Serialized challenge response (signature + public key as base64)
   */
  async signChallenge(challengeData: any): Promise<{ signature: string; publicKey: string }> {
    if (!this.keypair) {
      throw new Error('Auth service not initialized. Call initialize() first.');
    }

    const challenge = parseChallenge(challengeData);
    const response = await signChallenge(challenge, this.keypair.privateKey, this.keypair.publicKey);
    return serializeChallengeResponse(response);
  }

  /**
   * Store JWT token received after successful authentication
   *
   * @param token - JWT access token
   * @param expiresIn - Token lifetime in seconds (default: 900 = 15 minutes per SRS)
   */
  setJWT(token: string, expiresIn: number = 900): void {
    this.jwt = token;
    this.jwtExpiresAt = Date.now() + expiresIn * 1000;
    console.log(`JWT stored, expires at ${new Date(this.jwtExpiresAt).toISOString()}`);
  }

  /**
   * Get current JWT token
   *
   * @returns JWT token or null if not authenticated
   */
  getJWT(): string | null {
    // Check expiration
    if (this.jwt && this.jwtExpiresAt && Date.now() > this.jwtExpiresAt) {
      console.warn('JWT expired');
      this.jwt = null;
      this.jwtExpiresAt = null;
      return null;
    }

    return this.jwt;
  }

  /**
   * Check if currently authenticated (has valid JWT)
   */
  isAuthenticated(): boolean {
    return this.getJWT() !== null;
  }

  /**
   * Clear JWT token (logout)
   */
  clearJWT(): void {
    this.jwt = null;
    this.jwtExpiresAt = null;
    console.log('JWT cleared');
  }

  /**
   * Get time until JWT expiration (in seconds)
   */
  getJWTTimeRemaining(): number | null {
    if (!this.jwtExpiresAt) {
      return null;
    }

    const remaining = Math.max(0, (this.jwtExpiresAt - Date.now()) / 1000);
    return remaining > 0 ? remaining : null;
  }

  /**
   * Reset authentication (clear JWT and delete keypair)
   * Use with caution - this will require generating a new keypair
   */
  async reset(keypairId: string = 'default'): Promise<void> {
    this.clearJWT();
    await deleteKeypair(keypairId);
    this.keypair = null;
    console.log('Auth service reset - keypair deleted');
  }
}

/**
 * Create and initialize a new auth service instance
 * Convenience function for common use case
 */
export async function createAuthService(keypairId: string = 'default'): Promise<AuthService> {
  const service = new AuthService();
  await service.initialize(keypairId);
  return service;
}
