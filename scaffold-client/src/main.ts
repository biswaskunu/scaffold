import { LoroDoc, type VersionVector } from 'loro-crdt';
import { loadRoomDoc, saveUpdate, compactRoom } from './db';
import './style.css';

const PROTOCOL_VERSION = 1;
const roomInput = document.querySelector<HTMLInputElement>('#room')!;
const userNameInput = document.querySelector<HTMLInputElement>('#user-name')!;
const userColorSelect = document.querySelector<HTMLSelectElement>('#user-color')!;
const activeAvatarsContainer = document.querySelector<HTMLElement>('#active-avatars')!;
const canvas = document.querySelector<HTMLElement>('#canvas')!;
const connection = document.querySelector<HTMLElement>('#connection')!;

// Identity management
const PALETTE = ['#e06c75', '#d19a66', '#e5c07b', '#98c379', '#56b6c2', '#61afef', '#c678dd', '#abb2bf'];
const ANIMALS = ['Fox', 'Owl', 'Bear', 'Wolf', 'Deer', 'Hawk', 'Otter', 'Lynx', 'Badger', 'Panda'];

function getOrInitIdentity() {
  let name = localStorage.getItem('scaffold_user_name');
  let color = localStorage.getItem('scaffold_user_color');
  if (!name) {
    const randomAnimal = ANIMALS[Math.floor(Math.random() * ANIMALS.length)];
    const randomNum = Math.floor(Math.random() * 900) + 100;
    name = `${randomAnimal} ${randomNum}`;
    localStorage.setItem('scaffold_user_name', name);
  }
  if (!color) {
    color = PALETTE[Math.floor(Math.random() * PALETTE.length)];
    localStorage.setItem('scaffold_user_color', color);
  }
  return { name, color };
}

const myIdentity = getOrInitIdentity();
userNameInput.value = myIdentity.name;
userColorSelect.value = myIdentity.color;

userNameInput.addEventListener('input', () => {
  localStorage.setItem('scaffold_user_name', userNameInput.value);
});
userColorSelect.addEventListener('change', () => {
  localStorage.setItem('scaffold_user_color', userColorSelect.value);
});

function sendUpdate(update: Uint8Array): void {
  if (syncing && socket?.readyState === WebSocket.OPEN && update.length > 0) {
    socket.send(binaryFrame(0x03, update));
  }
}
type PeerPresence = {
  name: string;
  color: string;
  x?: number;
  y?: number;
  selected_nodes: string[];
  editing_node?: string | null;
  lastSeen: number;
};
const peers = new Map<string, PeerPresence>();
let mouseX: number | undefined = undefined;
let mouseY: number | undefined = undefined;
let selectedNodes = new Set<string>();
let editingNode: string | null = null;
let lastPresenceSend = 0;


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

function sendPresence(force = false) {
  if (!syncing || socket?.readyState !== WebSocket.OPEN) return;
  const now = performance.now();
  if (!force && now - lastPresenceSend < 40) return; // ~25fps throttle
  lastPresenceSend = now;

  const state = {
    name: userNameInput.value || 'Anonymous',
    color: userColorSelect.value,
    x: mouseX,
    y: mouseY,
    selected_nodes: Array.from(selectedNodes),
    editing_node: editingNode,
  };
  socket.send(jsonFrame(0x04, state));
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
  // Save current scroll position
  const scrollLeft = canvas.scrollLeft;
  const scrollTop = canvas.scrollTop;

  canvas.replaceChildren();

  // Render remote cursors
  for (const [, peer] of peers.entries()) {
    if (peer.x !== undefined && peer.y !== undefined && now() - peer.lastSeen < 10000) {
      const cursorEl = document.createElement('div');
      cursorEl.className = 'remote-cursor';
      const isIdle = now() - peer.lastSeen > 5000;
      cursorEl.style.opacity = isIdle ? '0.4' : '1.0';
      cursorEl.style.transform = `translate(${peer.x}px, ${peer.y}px)`;
      cursorEl.innerHTML = `
        <svg viewBox="0 0 24 24" fill="${peer.color}" stroke="white" stroke-width="2"><path d="M5.5 3.21V20.8l4.47-4.47h8.82L5.5 3.21z"/></svg>
        <span style="background: ${peer.color}">${escapeHtml(peer.name)}</span>
      `;
      canvas.append(cursorEl);
    }
  }

  for (const [id, value] of nodes.entries()) {
    if (!value || typeof value !== 'object') continue;
    const node = value as NodeValue;
    const card = document.createElement('article');
    card.className = 'node';
    if (selectedNodes.has(id)) {
      card.classList.add('selected');
    }
    card.dataset.id = id;
    card.style.left = `${node.x ?? 60}px`;
    card.style.top = `${node.y ?? 60}px`;

    // Check if any peer is editing this node
    let editingPeer: PeerPresence | undefined;
    for (const peer of peers.values()) {
      if (peer.editing_node === id && now() - peer.lastSeen < 10000) {
        editingPeer = peer;
        break;
      }
    }

    let innerHtml = `<span class="node-type">FEATURE</span><strong></strong><small></small>`;
    if (editingPeer) {
      innerHtml += `<div class="editing-badge" style="--peer-color: ${editingPeer.color}">${escapeHtml(editingPeer.name)} editing...</div>`;
      card.style.setProperty('--peer-color', editingPeer.color);
    } else if (selectedNodes.has(id)) {
      card.style.setProperty('--peer-color', userColorSelect.value);
    }

    card.innerHTML = innerHtml;
    card.querySelector('strong')!.textContent = node.title ?? 'Untitled';
    card.querySelector('small')!.textContent = id.slice(0, 8);

    card.addEventListener('pointerdown', (event) => {
      if (!event.shiftKey) {
        selectedNodes.clear();
      }
      selectedNodes.add(id);
      render();
      sendPresence(true);
      beginDrag(event, card, id);
    });

    canvas.append(card);
  }

  // Restore scroll
  canvas.scrollLeft = scrollLeft;
  canvas.scrollTop = scrollTop;

  renderAvatars();
}

function now() {
  return performance.now();
}

function escapeHtml(str: string): string {
  return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function renderAvatars() {
  activeAvatarsContainer.replaceChildren();
  // Self avatar
  const selfDiv = document.createElement('div');
  selfDiv.className = 'avatar-badge';
  selfDiv.style.background = userColorSelect.value;
  selfDiv.textContent = userNameInput.value || 'You';
  activeAvatarsContainer.append(selfDiv);

  for (const peer of peers.values()) {
    if (now() - peer.lastSeen < 10000) {
      const div = document.createElement('div');
      div.className = 'avatar-badge';
      div.style.background = peer.color;
      div.textContent = peer.name;
      activeAvatarsContainer.append(div);
    }
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
    } else if (frame[0] === 0x04) {
      const presencePayload = JSON.parse(new TextDecoder().decode(frame.slice(1))) as {
        connection_id: string;
        name: string;
        color: string;
        x?: number;
        y?: number;
        selected_nodes: string[];
        editing_node?: string | null;
      };
      if (presencePayload.name === undefined && presencePayload.color === undefined && presencePayload.x === undefined) {
        // Peer left frame
        peers.delete(presencePayload.connection_id);
      } else {
        peers.set(presencePayload.connection_id, {
          name: presencePayload.name,
          color: presencePayload.color,
          x: presencePayload.x,
          y: presencePayload.y,
          selected_nodes: presencePayload.selected_nodes || [],
          editing_node: presencePayload.editing_node,
          lastSeen: now(),
        });
      }
      render();
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

canvas.addEventListener('pointermove', (event) => {
  const rect = canvas.getBoundingClientRect();
  mouseX = event.clientX - rect.left + canvas.scrollLeft;
  mouseY = event.clientY - rect.top + canvas.scrollTop;
  sendPresence(false);
});

canvas.addEventListener('pointerleave', () => {
  mouseX = undefined;
  mouseY = undefined;
  sendPresence(true);
});

setInterval(() => {
  sendPresence(true);
  let changed = false;
  const current = now();
  for (const [id, peer] of peers.entries()) {
    if (current - peer.lastSeen > 10000) {
      peers.delete(id);
      changed = true;
    }
  }
  if (changed) {
    render();
  }
}, 3000);

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
