/**
 * Tests for Ed25519 key generation and signing
 */

import { describe, it, expect } from 'vitest';
import {
  generateKeypair,
  sign,
  verify,
  getPublicKeyFingerprint,
  publicKeyToBase64,
  publicKeyFromBase64,
} from './keys';

describe('Ed25519 Keys', () => {
  it('should generate valid Ed25519 keypair', async () => {
    const keypair = await generateKeypair();

    expect(keypair.publicKey).toBeInstanceOf(Uint8Array);
    expect(keypair.privateKey).toBeInstanceOf(Uint8Array);
    expect(keypair.publicKey.length).toBe(32);
    expect(keypair.privateKey.length).toBe(32);
  });

  it('should generate different keypairs each time', async () => {
    const keypair1 = await generateKeypair();
    const keypair2 = await generateKeypair();

    expect(keypair1.publicKey).not.toEqual(keypair2.publicKey);
    expect(keypair1.privateKey).not.toEqual(keypair2.privateKey);
  });

  it('should sign and verify messages correctly', async () => {
    const keypair = await generateKeypair();
    const message = new Uint8Array(32); // 32-byte challenge
    crypto.getRandomValues(message);

    const signature = await sign(message, keypair.privateKey);

    expect(signature).toBeInstanceOf(Uint8Array);
    expect(signature.length).toBe(64);

    const isValid = await verify(signature, message, keypair.publicKey);
    expect(isValid).toBe(true);
  });

  it('should fail verification with wrong public key', async () => {
    const keypair1 = await generateKeypair();
    const keypair2 = await generateKeypair();
    const message = new Uint8Array(32);
    crypto.getRandomValues(message);

    const signature = await sign(message, keypair1.privateKey);

    // Try to verify with different public key
    const isValid = await verify(signature, message, keypair2.publicKey);
    expect(isValid).toBe(false);
  });

  it('should fail verification with tampered message', async () => {
    const keypair = await generateKeypair();
    const message = new Uint8Array(32);
    crypto.getRandomValues(message);

    const signature = await sign(message, keypair.privateKey);

    // Tamper with message
    const tamperedMessage = new Uint8Array(message);
    tamperedMessage[0] ^= 0xFF;

    const isValid = await verify(signature, tamperedMessage, keypair.publicKey);
    expect(isValid).toBe(false);
  });

  it('should generate valid fingerprint', async () => {
    const keypair = await generateKeypair();
    const fingerprint = getPublicKeyFingerprint(keypair.publicKey);

    expect(typeof fingerprint).toBe('string');
    expect(fingerprint.length).toBe(64); // 32 bytes = 64 hex chars
    expect(/^[0-9a-f]+$/.test(fingerprint)).toBe(true);
  });

  it('should generate same fingerprint for same public key', async () => {
    const keypair = await generateKeypair();
    const fp1 = getPublicKeyFingerprint(keypair.publicKey);
    const fp2 = getPublicKeyFingerprint(keypair.publicKey);

    expect(fp1).toBe(fp2);
  });

  it('should convert public key to/from base64', async () => {
    const keypair = await generateKeypair();
    const base64 = publicKeyToBase64(keypair.publicKey);

    expect(typeof base64).toBe('string');
    expect(base64.length).toBeGreaterThan(0);

    const decoded = publicKeyFromBase64(base64);
    expect(decoded).toEqual(keypair.publicKey);
  });
});
