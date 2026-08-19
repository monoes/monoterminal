/**
 * IndexedDB storage for Ed25519 keypairs
 * SRS §3.2.2: Secure client-side key storage
 */

const DB_NAME = 'monoterminal-auth';
const DB_VERSION = 1;
const STORE_NAME = 'keypairs';

export interface StoredKeypair {
  publicKey: Uint8Array;
  privateKey: Uint8Array;
  createdAt: number;
  lastUsed: number;
}

/**
 * Initialize IndexedDB database
 */
function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);

    request.onerror = () => {
      reject(new Error(`Failed to open database: ${request.error?.message}`));
    };

    request.onsuccess = () => {
      resolve(request.result);
    };

    request.onupgradeneeded = (event) => {
      const db = (event.target as IDBOpenDBRequest).result;

      // Create keypairs object store if it doesn't exist
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME, { keyPath: 'id' });
      }
    };
  });
}

/**
 * Store Ed25519 keypair in IndexedDB
 */
export async function storeKeypair(
  id: string,
  publicKey: Uint8Array,
  privateKey: Uint8Array
): Promise<void> {
  const db = await openDatabase();

  return new Promise((resolve, reject) => {
    const transaction = db.transaction(STORE_NAME, 'readwrite');
    const store = transaction.objectStore(STORE_NAME);

    const keypair: StoredKeypair & { id: string } = {
      id,
      publicKey,
      privateKey,
      createdAt: Date.now(),
      lastUsed: Date.now(),
    };

    const request = store.put(keypair);

    request.onerror = () => {
      reject(new Error(`Failed to store keypair: ${request.error?.message}`));
    };

    request.onsuccess = () => {
      resolve();
    };

    transaction.oncomplete = () => {
      db.close();
    };
  });
}

/**
 * Load Ed25519 keypair from IndexedDB
 */
export async function loadKeypair(id: string): Promise<StoredKeypair | null> {
  const db = await openDatabase();

  return new Promise((resolve, reject) => {
    const transaction = db.transaction(STORE_NAME, 'readwrite');
    const store = transaction.objectStore(STORE_NAME);

    const request = store.get(id);

    request.onerror = () => {
      reject(new Error(`Failed to load keypair: ${request.error?.message}`));
    };

    request.onsuccess = () => {
      const result = request.result as (StoredKeypair & { id: string }) | undefined;

      if (result) {
        // Update lastUsed timestamp
        result.lastUsed = Date.now();
        store.put(result);
        resolve(result);
      } else {
        resolve(null);
      }
    };

    transaction.oncomplete = () => {
      db.close();
    };
  });
}

/**
 * Delete keypair from IndexedDB
 */
export async function deleteKeypair(id: string): Promise<void> {
  const db = await openDatabase();

  return new Promise((resolve, reject) => {
    const transaction = db.transaction(STORE_NAME, 'readwrite');
    const store = transaction.objectStore(STORE_NAME);

    const request = store.delete(id);

    request.onerror = () => {
      reject(new Error(`Failed to delete keypair: ${request.error?.message}`));
    };

    request.onsuccess = () => {
      resolve();
    };

    transaction.oncomplete = () => {
      db.close();
    };
  });
}

/**
 * List all stored keypairs (metadata only, no private keys)
 */
export async function listKeypairs(): Promise<Array<{ id: string; publicKey: Uint8Array; createdAt: number; lastUsed: number }>> {
  const db = await openDatabase();

  return new Promise((resolve, reject) => {
    const transaction = db.transaction(STORE_NAME, 'readonly');
    const store = transaction.objectStore(STORE_NAME);

    const request = store.getAll();

    request.onerror = () => {
      reject(new Error(`Failed to list keypairs: ${request.error?.message}`));
    };

    request.onsuccess = () => {
      const results = (request.result || []).map((kp: StoredKeypair & { id: string }) => ({
        id: kp.id,
        publicKey: kp.publicKey,
        createdAt: kp.createdAt,
        lastUsed: kp.lastUsed,
      }));
      resolve(results);
    };

    transaction.oncomplete = () => {
      db.close();
    };
  });
}
