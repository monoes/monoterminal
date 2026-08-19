/**
 * Ed25519 key generation and management for browser
 * SRS §3.2.2: Client-side Ed25519 authentication
 *
 * Uses @noble/ed25519 for broad browser compatibility
 * (WebCrypto API doesn't support Ed25519 in all target browsers)
 */

import * as ed25519 from '@noble/ed25519';
import { storeKeypair, loadKeypair } from './storage';

export interface Ed25519Keypair {
  publicKey: Uint8Array;  // 32 bytes
  privateKey: Uint8Array; // 32 bytes
}

/**
 * Generate a new Ed25519 keypair using cryptographically secure random
 */
export async function generateKeypair(): Promise<Ed25519Keypair> {
  // Generate random 32-byte private key
  const privateKey = ed25519.utils.randomPrivateKey();

  // Derive public key from private key
  const publicKey = await ed25519.getPublicKeyAsync(privateKey);

  return {
    publicKey,
    privateKey,
  };
}

/**
 * Sign a message/challenge with Ed25519 private key
 *
 * @param message - The message to sign (typically a 32-byte challenge nonce)
 * @param privateKey - The Ed25519 private key (32 bytes)
 * @returns Ed25519 signature (64 bytes)
 */
export async function sign(message: Uint8Array, privateKey: Uint8Array): Promise<Uint8Array> {
  return await ed25519.signAsync(message, privateKey);
}

/**
 * Verify an Ed25519 signature (for testing purposes)
 *
 * @param signature - The signature to verify (64 bytes)
 * @param message - The original message
 * @param publicKey - The public key (32 bytes)
 * @returns true if signature is valid
 */
export async function verify(
  signature: Uint8Array,
  message: Uint8Array,
  publicKey: Uint8Array
): Promise<boolean> {
  return await ed25519.verifyAsync(signature, message, publicKey);
}

/**
 * Load or generate Ed25519 keypair for the default identity
 *
 * This is the main entry point for getting a keypair:
 * - Loads from IndexedDB if exists
 * - Generates new keypair if not found
 * - Stores the new keypair in IndexedDB
 *
 * @param id - Keypair identifier (default: 'default')
 * @returns Ed25519 keypair
 */
export async function loadOrGenerateKeypair(id: string = 'default'): Promise<Ed25519Keypair> {
  // Try to load existing keypair
  const stored = await loadKeypair(id);

  if (stored) {
    console.log(`Loaded existing Ed25519 keypair (id: ${id})`);
    return {
      publicKey: stored.publicKey,
      privateKey: stored.privateKey,
    };
  }

  // Generate new keypair
  console.log(`Generating new Ed25519 keypair (id: ${id})`);
  const keypair = await generateKeypair();

  // Store for future use
  await storeKeypair(id, keypair.publicKey, keypair.privateKey);

  return keypair;
}

/**
 * Get the public key fingerprint (hex-encoded)
 * Used for displaying the key identity to users
 *
 * @param publicKey - The Ed25519 public key (32 bytes)
 * @returns Hex-encoded fingerprint string
 */
export function getPublicKeyFingerprint(publicKey: Uint8Array): string {
  // Convert to hex string
  return Array.from(publicKey)
    .map(b => b.toString(16).padStart(2, '0'))
    .join('');
}

/**
 * Format public key as base64 (for transmission)
 */
export function publicKeyToBase64(publicKey: Uint8Array): string {
  return btoa(String.fromCharCode(...publicKey));
}

/**
 * Parse base64 public key
 */
export function publicKeyFromBase64(base64: string): Uint8Array {
  const binary = atob(base64);
  return Uint8Array.from(binary, c => c.charCodeAt(0));
}
