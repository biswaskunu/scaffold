import { LoroDoc, type VersionVector } from 'loro-crdt';
import { loadRoomDoc, saveUpdate, compactRoom } from './db';
import './style.css';

const PROTOCOL_VERSION = 1;
const roomInput = document.querySelector<HTMLInputElement>('#room')!;
const canvas = document.querySelector<HTMLElement>('#canvas')!;
const connection = document.querySelector<HTMLElement>('#connection')!;

let doc = new LoroDoc();
let nodes = doc.getMap('nodes');
let socket: WebSocket | undefined;
let syncing = false;
let lastSyncedVersion: VersionVector = doc.oplogVersion();
let lastPersistedVersion: VersionVector = doc.oplogVersion();
let updateCount = 0;
let currentRoomId = roomInput.value;

// Backoff state
let reconnectTimer: number | null = null;
let backoffAttempt = 0;
const BASE_DELAY_MS = 1000;
const MAX_DELAY_MS = 30000;

type NodeValue = { id: string; title: string; x: number; y: number };
type Hello = { protocol_version: number; room_id: string; version: VersionVector };
type SyncReply = { protocol_version: number; version: VersionVector; updates: number[] };

function binaryFrame(kind: number, bytes: Uint8Array): Uint8Array {
  const frame = new Uint8Array(bytes.length + 1);
  frame[0] = kind;
  frame.set(bytes, 1);
  return frame;
}

function jsonFrame(kind: number, value: unknown): Uint8Array {
  return binaryFrame(kind, new TextEncoder().encode(JSON.stringify(value)));
}

function sendUpdate(update: Uint8Array): void {
  if (syncing && socket?.readyState === WebSocket.OPEN && update.length > 0) {
    socket.send(binaryFrame(0x03, update));
  }
}

async function persistAndPush(update: Uint8Array, origin: 'local' | 'remote') {
  if (update.length === 0) return;
  try {
    await saveUpdate(currentRoomId, update);
    updateCount++;
    lastPersistedVersion = doc.oplogVersion();
    if (updateCount >= 200) {
      await compactRoom(currentRoomId, doc);
      updateCount = 0;
    }
  } catch (err) {
    setConnected(false, 'Storage quota / Private mode error');
  }

  if (origin === 'local') {
    sendUpdate(update);
  }
}

function render(): void {
  canvas.replaceChildren();
  for (const [id, value] of nodes.entries()) {
    if (!value || typeof value !== 'object') continue;
    const node = value as NodeValue;
    const card = document.createElement('article');
    card.className = 'node';
    card.dataset.id = id;
    card.style.left = `${node.x ?? 60}px`;
    card.style.top = `${node.y ?? 60}px`;
    card.innerHTML = `<span class="node-type">FEATURE</span><strong></strong><small></small>`;
    card.querySelector('strong')!.textContent = node.title ?? 'Untitled';
    card.querySelector('small')!.textContent = id.slice(0, 8);
    card.addEventListener('pointerdown', (event) => beginDrag(event, card, id));
    canvas.append(card);
  }
}

function beginDrag(event: PointerEvent, card: HTMLElement, id: string): void {
  if ((event.target as HTMLElement).closest('button')) return;
  const startX = event.clientX;
  const startY = event.clientY;
  const initialX = Number.parseFloat(card.style.left);
  const initialY = Number.parseFloat(card.style.top);
  card.setPointerCapture(event.pointerId);
  const move = (moveEvent: PointerEvent) => {
    const x = Math.max(8, initialX + moveEvent.clientX - startX);
    const y = Math.max(8, initialY + moveEvent.clientY - startY);
    card.style.left = `${x}px`;
    card.style.top = `${y}px`;
    doc.getMap(`nodes/${id}`).set('x', x);
    doc.getMap(`nodes/${id}`).set('y', y);
  };
  const finish = () => {
    card.removeEventListener('pointermove', move);
    card.removeEventListener('pointerup', finish);
    card.removeEventListener('pointercancel', finish);
    doc.commit();
  };
  card.addEventListener('pointermove', move);
  card.addEventListener('pointerup', finish);
  card.addEventListener('pointercancel', finish);
}

doc.subscribe((event) => {
  if (event.origin === 'remote') return;
  if (event.by === 'local') {
    const update = doc.export({ mode: 'update', from: lastPersistedVersion });
    persistAndPush(update, 'local');
    lastSyncedVersion = doc.oplogVersion();
  }
  render();
});

function setConnected(connected: boolean, label: string): void {
  connection.classList.toggle('online', connected);
  connection.lastChild!.textContent = label;
}

function getBackoffDelay(): number {
  const exp = Math.min(MAX_DELAY_MS, BASE_DELAY_MS * Math.pow(2, backoffAttempt));
  const jitter = exp * (Math.random() * 0.5 - 0.25); // ±25% jitter
  return Math.round(Math.max(BASE_DELAY_MS, exp + jitter));
}

function scheduleReconnect(): void {
  if (reconnectTimer !== null) return;
  const delay = getBackoffDelay();
  setConnected(false, `Offline, retrying in ${Math.round(delay / 1000)}s`);
  reconnectTimer = window.setTimeout(() => {
    reconnectTimer = null;
    backoffAttempt++;
    connect(false);
  }, delay);
}

function connect(manual = false): void {
  if (manual) {
    if (reconnectTimer !== null) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
  }
  socket?.close();
  syncing = false;
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  socket = new WebSocket(`${protocol}//${location.host}/ws/${encodeURIComponent(currentRoomId)}`);
  socket.binaryType = 'arraybuffer';
  setConnected(false, 'Connecting');

  socket.addEventListener('open', () => {
    const hello: Hello = { protocol_version: PROTOCOL_VERSION, room_id: currentRoomId, version: doc.oplogVersion() };
    socket?.send(jsonFrame(0x01, hello));
  });

  socket.addEventListener('message', async (event: MessageEvent<ArrayBuffer>) => {
    const frame = new Uint8Array(event.data);
    if (frame[0] === 0x02) {
      const reply = JSON.parse(new TextDecoder().decode(frame.slice(1))) as SyncReply;
      if (reply.updates.length) {
        const updateBytes = new Uint8Array(reply.updates);
        doc.import(updateBytes);
        await persistAndPush(updateBytes, 'remote');
      }
      lastSyncedVersion = reply.version;
      syncing = true;
      backoffAttempt = 0; // Reset backoff on successful sync
      setConnected(true, 'Connected');

      // Push any updates the server lacks (client oplog version vs server reply version)
      const outgoingUpdate = doc.export({ mode: 'update', from: lastSyncedVersion });
      if (outgoingUpdate.length > 0) {
        sendUpdate(outgoingUpdate);
      }
    } else if (frame[0] === 0x03) {
      const updateBytes = frame.slice(1);
      doc.import(updateBytes);
      lastSyncedVersion = doc.oplogVersion();
      await persistAndPush(updateBytes, 'remote');
    } else if (frame[0] === 0x05) {
      const error = JSON.parse(new TextDecoder().decode(frame.slice(1))) as { message: string };
      if (error.message.includes('protocol_version_mismatch')) {
        setConnected(false, error.message);
        return; // Do not retry on version mismatch
      }
      if (error.message.includes('resync_required')) {
        setConnected(false, 'Resync required, reconnecting');
        connect(true);
        return;
      }
      setConnected(false, error.message);
    }
    render();
  });

  socket.addEventListener('close', () => {
    syncing = false;
    scheduleReconnect();
  });
  socket.addEventListener('error', () => {
    socket?.close();
  });
}

async function switchRoom(newRoomId: string) {
  currentRoomId = newRoomId;
  const loaded = await loadRoomDoc(currentRoomId);
  doc = loaded.doc;
  nodes = doc.getMap('nodes');
  lastPersistedVersion = loaded.lastVersion;
  lastSyncedVersion = doc.oplogVersion();
  updateCount = loaded.updateCount;

  doc.subscribe((event) => {
    if (event.origin === 'remote') return;
    if (event.by === 'local') {
      const update = doc.export({ mode: 'update', from: lastPersistedVersion });
      persistAndPush(update, 'local');
      lastSyncedVersion = doc.oplogVersion();
    }
    render();
  });

  render();
  connect(true);
}

document.querySelector('#connect')!.addEventListener('click', () => connect(true));
roomInput.addEventListener('change', () => {
  if (roomInput.value && roomInput.value !== currentRoomId) {
    switchRoom(roomInput.value);
  }
});

document.querySelector('#add')!.addEventListener('click', () => {
  const id = crypto.randomUUID();
  const count = nodes.size;
  const node = nodes.ensureMergeableMap(id);
  node.set('id', id);
  node.set('title', `New feature ${count + 1}`);
  node.set('x', 48 + (count % 4) * 250);
  node.set('y', 48 + Math.floor(count / 4) * 170);
  doc.commit();
  render();
});

window.addEventListener('online', () => {
  connect(true);
});

window.addEventListener('visibilitychange', async () => {
  if (document.visibilityState === 'hidden') {
    await compactRoom(currentRoomId, doc);
  }
});

// Initial load & connect
async function init() {
  const loaded = await loadRoomDoc(currentRoomId);
  doc = loaded.doc;
  nodes = doc.getMap('nodes');
  lastPersistedVersion = loaded.lastVersion;
  lastSyncedVersion = doc.oplogVersion();
  updateCount = loaded.updateCount;

  doc.subscribe((event) => {
    if (event.origin === 'remote') return;
    if (event.by === 'local') {
      const update = doc.export({ mode: 'update', from: lastPersistedVersion });
      persistAndPush(update, 'local');
      lastSyncedVersion = doc.oplogVersion();
    }
    render();
  });

  render();
  connect(true);
}

init();
