import { LoroDoc, type VersionVector } from 'loro-crdt';

export interface StorageRecord {
  id?: number;
  roomId: string;
  bytes: Uint8Array;
  isSnapshot?: boolean;
}

const DB_NAME = 'scaffold';
const STORE_NAME = 'updates';
const DB_VERSION = 1;

export function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        const store = db.createObjectStore(STORE_NAME, { keyPath: 'id', autoIncrement: true });
        store.createIndex('roomId', 'roomId', { unique: false });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

export async function loadRoomDoc(roomId: string): Promise<{ doc: LoroDoc; lastVersion: VersionVector; updateCount: number }> {
  const doc = new LoroDoc();
  let updateCount = 0;
  try {
    const db = await openDb();
    await new Promise<void>((resolve, reject) => {
      const transaction = db.transaction(STORE_NAME, 'readonly');
      const store = transaction.objectStore(STORE_NAME);
      const index = store.index('roomId');
      const request = index.openCursor(IDBKeyRange.only(roomId));

      request.onsuccess = (event) => {
        const cursor = (event.target as IDBRequest<IDBCursorWithValue>).result;
        if (cursor) {
          const record = cursor.value as StorageRecord;
          if (record.bytes && record.bytes.length > 0) {
            doc.import(record.bytes);
            updateCount++;
          }
          cursor.continue();
        } else {
          resolve();
        }
      };
      request.onerror = () => reject(request.error);
    });
    db.close();
  } catch (err) {
    console.warn('Failed to load room from IndexedDB:', err);
  }
  return { doc, lastVersion: doc.oplogVersion(), updateCount };
}

export async function saveUpdate(roomId: string, bytes: Uint8Array, isSnapshot = false): Promise<void> {
  if (bytes.length === 0) return;
  try {
    const db = await openDb();
    await new Promise<void>((resolve, reject) => {
      const transaction = db.transaction(STORE_NAME, 'readwrite');
      const store = transaction.objectStore(STORE_NAME);
      const record: StorageRecord = { roomId, bytes, isSnapshot };
      const request = store.add(record);
      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
    db.close();
  } catch (err) {
    console.warn('Failed to save update to IndexedDB (quota or private mode?):', err);
    throw err;
  }
}

export async function compactRoom(roomId: string, doc: LoroDoc): Promise<void> {
  try {
    const snapshot = doc.export({ mode: 'snapshot' });
    const db = await openDb();
    await new Promise<void>((resolve, reject) => {
      const transaction = db.transaction(STORE_NAME, 'readwrite');
      const store = transaction.objectStore(STORE_NAME);
      const index = store.index('roomId');
      const request = index.openCursor(IDBKeyRange.only(roomId));

      request.onsuccess = (event) => {
        const cursor = (event.target as IDBRequest<IDBCursorWithValue>).result;
        if (cursor) {
          store.delete(cursor.primaryKey);
          cursor.continue();
        } else {
          // Add single snapshot record
          const record: StorageRecord = { roomId, bytes: snapshot, isSnapshot: true };
          store.add(record);
          resolve();
        }
      };
      request.onerror = () => reject(request.error);
    });
    db.close();
  } catch (err) {
    console.warn('Failed to compact room in IndexedDB:', err);
  }
}
