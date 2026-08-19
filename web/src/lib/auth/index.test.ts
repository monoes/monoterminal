/**
 * Tests for main AuthService
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { AuthService, createAuthService } from './index';
import { deleteKeypair } from './storage';

describe('AuthService', () => {
  beforeEach(async () => {
    // Clean up test keypairs
    try {
      await deleteKeypair('test-auth');
    } catch {
      // Ignore if doesn't exist
    }
  });

  it('should initialize with keypair', async () => {
    const service = new AuthService();
    await service.initialize('test-auth');

    const publicKey = service.getPublicKey();
    expect(publicKey).toBeInstanceOf(Uint8Array);
    expect(publicKey.length).toBe(32);
  });

  it('should throw error if not initialized', () => {
    const service = new AuthService();

    expect(() => service.getPublicKey()).toThrow('Auth service not initialized');
  });

  it('should get public key fingerprint', async () => {
    const service = new AuthService();
    await service.initialize('test-auth');

    const fingerprint = service.getPublicKeyFingerprint();
    expect(typeof fingerprint).toBe('string');
    expect(fingerprint.length).toBe(64);
  });

  it('should get public key as base64', async () => {
    const service = new AuthService();
    await service.initialize('test-auth');

    const base64 = service.getPublicKeyBase64();
    expect(typeof base64).toBe('string');
    expect(base64.length).toBeGreaterThan(0);
  });

  it('should sign challenge', async () => {
    const service = new AuthService();
    await service.initialize('test-auth');

    const nonce = new Uint8Array(32);
    crypto.getRandomValues(nonce);
    const base64Nonce = btoa(String.fromCharCode(...nonce));

    const challengeData = {
      nonce: base64Nonce,
      expiresAt: Date.now() + 30000,
    };

    const response = await service.signChallenge(challengeData);

    expect(typeof response.signature).toBe('string');
    expect(typeof response.publicKey).toBe('string');
  });

  it('should store and retrieve JWT', async () => {
    const service = new AuthService();
    await service.initialize('test-auth');

    expect(service.isAuthenticated()).toBe(false);
    expect(service.getJWT()).toBeNull();

    const testJWT = 'eyJhbGciOiJFZERTQSIsInR5cCI6IkpXVCJ9.test.signature';
    service.setJWT(testJWT, 900);

    expect(service.isAuthenticated()).toBe(true);
    expect(service.getJWT()).toBe(testJWT);

    const timeRemaining = service.getJWTTimeRemaining();
    expect(timeRemaining).toBeGreaterThan(0);
    expect(timeRemaining).toBeLessThanOrEqual(900);
  });

  it('should detect expired JWT', async () => {
    const service = new AuthService();
    await service.initialize('test-auth');

    const testJWT = 'expired.jwt.token';
    service.setJWT(testJWT, -1); // Already expired

    expect(service.isAuthenticated()).toBe(false);
    expect(service.getJWT()).toBeNull();
  });

  it('should clear JWT', async () => {
    const service = new AuthService();
    await service.initialize('test-auth');

    const testJWT = 'test.jwt.token';
    service.setJWT(testJWT, 900);

    expect(service.isAuthenticated()).toBe(true);

    service.clearJWT();

    expect(service.isAuthenticated()).toBe(false);
    expect(service.getJWT()).toBeNull();
    expect(service.getJWTTimeRemaining()).toBeNull();
  });

  it('should reset service and delete keypair', async () => {
    const service = new AuthService();
    await service.initialize('test-auth');

    const testJWT = 'test.jwt.token';
    service.setJWT(testJWT, 900);

    expect(service.isAuthenticated()).toBe(true);

    await service.reset('test-auth');

    expect(service.isAuthenticated()).toBe(false);
    expect(() => service.getPublicKey()).toThrow('Auth service not initialized');
  });

  it('should create and initialize service with helper', async () => {
    const service = await createAuthService('test-auth-helper');

    const publicKey = service.getPublicKey();
    expect(publicKey).toBeInstanceOf(Uint8Array);
    expect(publicKey.length).toBe(32);

    // Clean up
    await service.reset('test-auth-helper');
  });

  it('should reuse same keypair across instances', async () => {
    const service1 = await createAuthService('test-auth-reuse');
    const fingerprint1 = service1.getPublicKeyFingerprint();

    const service2 = await createAuthService('test-auth-reuse');
    const fingerprint2 = service2.getPublicKeyFingerprint();

    expect(fingerprint1).toBe(fingerprint2);

    // Clean up
    await service1.reset('test-auth-reuse');
  });
});
