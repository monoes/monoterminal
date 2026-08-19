/**
 * Tests for IndexedDB keypair storage
 * Note: These tests use fake-indexeddb in the test environment
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { storeKeypair, loadKeypair, deleteKeypair, listKeypairs } from './storage';

describe('IndexedDB Storage', () => {
  beforeEach(async () => {
    // Clean up any existing keypairs
    const list = await listKeypairs();
    for (const kp of list) {
      await deleteKeypair(kp.id);
    }
  });

  it('should store and load keypair', async () => {
    const id = 'test-keypair-1';
    const publicKey = new Uint8Array(32);
    const privateKey = new Uint8Array(32);
    crypto.getRandomValues(publicKey);
    crypto.getRandomValues(privateKey);

    await storeKeypair(id, publicKey, privateKey);

    const loaded = await loadKeypair(id);

    expect(loaded).not.toBeNull();
    expect(new Uint8Array(loaded!.publicKey)).toEqual(publicKey);
    expect(new Uint8Array(loaded!.privateKey)).toEqual(privateKey);
    expect(loaded!.createdAt).toBeGreaterThan(0);
    expect(loaded!.lastUsed).toBeGreaterThan(0);
  });

  it('should return null for non-existent keypair', async () => {
    const loaded = await loadKeypair('non-existent');
    expect(loaded).toBeNull();
  });

  it('should update lastUsed timestamp on load', async () => {
    const id = 'test-keypair-2';
    const publicKey = new Uint8Array(32);
    const privateKey = new Uint8Array(32);
    crypto.getRandomValues(publicKey);
    crypto.getRandomValues(privateKey);

    await storeKeypair(id, publicKey, privateKey);

    const loaded1 = await loadKeypair(id);
    const firstLastUsed = loaded1!.lastUsed;

    // Wait a bit
    await new Promise(resolve => setTimeout(resolve, 10));

    const loaded2 = await loadKeypair(id);
    const secondLastUsed = loaded2!.lastUsed;

    expect(secondLastUsed).toBeGreaterThanOrEqual(firstLastUsed);
  });

  it('should delete keypair', async () => {
    const id = 'test-keypair-3';
    const publicKey = new Uint8Array(32);
    const privateKey = new Uint8Array(32);
    crypto.getRandomValues(publicKey);
    crypto.getRandomValues(privateKey);

    await storeKeypair(id, publicKey, privateKey);

    let loaded = await loadKeypair(id);
    expect(loaded).not.toBeNull();

    await deleteKeypair(id);

    loaded = await loadKeypair(id);
    expect(loaded).toBeNull();
  });

  it('should list all keypairs', async () => {
    const kp1 = { id: 'test-list-1', publicKey: new Uint8Array(32), privateKey: new Uint8Array(32) };
    const kp2 = { id: 'test-list-2', publicKey: new Uint8Array(32), privateKey: new Uint8Array(32) };

    crypto.getRandomValues(kp1.publicKey);
    crypto.getRandomValues(kp1.privateKey);
    crypto.getRandomValues(kp2.publicKey);
    crypto.getRandomValues(kp2.privateKey);

    await storeKeypair(kp1.id, kp1.publicKey, kp1.privateKey);
    await storeKeypair(kp2.id, kp2.publicKey, kp2.privateKey);

    const list = await listKeypairs();

    expect(list.length).toBeGreaterThanOrEqual(2);

    const ids = list.map(kp => kp.id);
    expect(ids).toContain(kp1.id);
    expect(ids).toContain(kp2.id);

    // Verify that private keys are NOT in the list
    const kp1Listed = list.find(kp => kp.id === kp1.id);
    expect(kp1Listed).toBeDefined();
    expect(new Uint8Array(kp1Listed!.publicKey)).toEqual(kp1.publicKey);
    expect('privateKey' in kp1Listed!).toBe(false);
  });

  it('should overwrite existing keypair with same id', async () => {
    const id = 'test-overwrite';
    const publicKey1 = new Uint8Array(32);
    const privateKey1 = new Uint8Array(32);
    const publicKey2 = new Uint8Array(32);
    const privateKey2 = new Uint8Array(32);

    crypto.getRandomValues(publicKey1);
    crypto.getRandomValues(privateKey1);
    crypto.getRandomValues(publicKey2);
    crypto.getRandomValues(privateKey2);

    await storeKeypair(id, publicKey1, privateKey1);
    await storeKeypair(id, publicKey2, privateKey2);

    const loaded = await loadKeypair(id);

    expect(loaded).not.toBeNull();
    expect(new Uint8Array(loaded!.publicKey)).toEqual(publicKey2);
    expect(new Uint8Array(loaded!.privateKey)).toEqual(privateKey2);
  });
});
