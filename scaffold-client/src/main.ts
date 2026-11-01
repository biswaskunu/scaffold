import { LoroDoc, type VersionVector } from 'loro-crdt';
import './style.css';

const PROTOCOL_VERSION = 1;
const doc = new LoroDoc();
const nodes = doc.getMap('nodes');
const canvas = document.querySelector<HTMLElement>('#canvas')!;
const connection = document.querySelector<HTMLElement>('#connection')!;
const roomInput = document.querySelector<HTMLInputElement>('#room')!;
let socket: WebSocket | undefined;
let syncing = false;
let lastSyncedVersion: VersionVector = doc.oplogVersion();

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
    sendUpdate(doc.export({ mode: 'update', from: lastSyncedVersion }));
    lastSyncedVersion = doc.oplogVersion();
  }
  render();
});

function setConnected(connected: boolean, label: string): void {
  connection.classList.toggle('online', connected);
  connection.lastChild!.textContent = label;
}

function connect(): void {
  socket?.close();
  syncing = false;
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  socket = new WebSocket(`${protocol}//${location.host}/ws/${encodeURIComponent(roomInput.value)}`);
  socket.binaryType = 'arraybuffer';
  setConnected(false, 'Connecting');
  socket.addEventListener('open', () => {
    const hello: Hello = { protocol_version: PROTOCOL_VERSION, room_id: roomInput.value, version: doc.oplogVersion() };
    socket?.send(jsonFrame(0x01, hello));
  });
  socket.addEventListener('message', (event: MessageEvent<ArrayBuffer>) => {
    const frame = new Uint8Array(event.data);
    if (frame[0] === 0x02) {
      const reply = JSON.parse(new TextDecoder().decode(frame.slice(1))) as SyncReply;
      if (reply.updates.length) doc.import(new Uint8Array(reply.updates));
      lastSyncedVersion = reply.version;
      syncing = true;
      setConnected(true, 'Connected');
    } else if (frame[0] === 0x03) {
      doc.import(frame.slice(1));
      lastSyncedVersion = doc.oplogVersion();
    } else if (frame[0] === 0x05) {
      const error = JSON.parse(new TextDecoder().decode(frame.slice(1))) as { message: string };
      setConnected(false, error.message);
    }
    render();
  });
  socket.addEventListener('close', () => { syncing = false; setConnected(false, 'Offline'); });
  socket.addEventListener('error', () => setConnected(false, 'Connection error'));
}

document.querySelector('#connect')!.addEventListener('click', connect);
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

render();
connect();
