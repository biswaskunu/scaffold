import React, { useEffect, useState, useRef, useCallback, useMemo } from 'react';
import { createRoot } from 'react-dom/client';
import { LoroDoc, type VersionVector } from 'loro-crdt';
import { ulid } from 'ulid';
import { ReactFlow, Background, Controls, MiniMap, applyNodeChanges, applyEdgeChanges, type Node, type Edge, type Connection, type NodeChange, type EdgeChange } from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { loadRoomDoc, saveUpdate, compactRoom } from './db';
import './style.css';

const PROTOCOL_VERSION = 1;
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

type PeerPresence = {
  name: string;
  color: string;
  x?: number;
  y?: number;
  selected_nodes: string[];
  editing_node?: string | null;
  lastSeen: number;
};

type Hello = { protocol_version: number; room_id: string; version: VersionVector };
type SyncReply = { protocol_version: number; version: VersionVector; updates: number[] };

export function App() {
  const identity = useMemo(() => getOrInitIdentity(), []);
  const [roomId, setRoomId] = useState('demo-room');
  const [userName, setUserName] = useState(identity.name);
  const [userColor, setUserColor] = useState(identity.color);
  const [connected, setConnected] = useState(false);
  const [statusText, setStatusText] = useState('Connecting');
  const [peers, setPeers] = useState<Map<string, PeerPresence>>(new Map());

  const docRef = useRef<LoroDoc>(new LoroDoc());
  const lastPersistedVersionRef = useRef<VersionVector>(docRef.current.oplogVersion());
  const lastSyncedVersionRef = useRef<VersionVector>(docRef.current.oplogVersion());
  const updateCountRef = useRef(0);
  const socketRef = useRef<WebSocket | null>(null);
  const syncingRef = useRef(false);
  const reconnectTimerRef = useRef<number | null>(null);
  const backoffAttemptRef = useRef(0);

  const [rfNodes, setRfNodes] = useState<Node[]>([]);
  const [rfEdges, setRfEdges] = useState<Edge[]>([]);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'canvas' | 'extract'>('canvas');
  const [extractProposal, setExtractProposal] = useState<any>(null);

  // Sync Loro doc to React Flow state
  const syncDocToState = useCallback(() => {
    const doc = docRef.current;
    const nodesMap = doc.getMap('nodes');
    const edgesMap = doc.getMap('edges');

    const nextNodes: Node[] = [];
    for (const [id, val] of nodesMap.entries()) {
      if (!val || typeof val !== 'object') continue;
      const data = val.toJSON ? val.toJSON() : val;
      nextNodes.push({
        id,
        type: 'default',
        position: { x: data.x ?? 100, y: data.y ?? 100 },
        data: {
          label: data.title || 'Untitled',
          type: data.type || 'feature',
          body: data.body || '',
          status: data.status || 'todo',
          effort: data.effort ?? null,
          ai_generated: data.ai_generated ?? false,
          source_node: data.source_node ?? null,
        },
      });
    }

    const nextEdges: Edge[] = [];
    for (const [id, val] of edgesMap.entries()) {
      if (!val || typeof val !== 'object') continue;
      const data = val.toJSON ? val.toJSON() : val;
      nextEdges.push({
        id,
        source: data.from,
        target: data.to,
        label: data.kind || 'depends_on',
        animated: data.kind === 'depends_on',
      });
    }

    setRfNodes(nextNodes);
    setRfEdges(nextEdges);
  }, []);

  const persistAndPush = useCallback(async (update: Uint8Array, origin: 'local' | 'remote') => {
    if (update.length === 0) return;
    try {
      await saveUpdate(roomId, update);
      updateCountRef.current++;
      lastPersistedVersionRef.current = docRef.current.oplogVersion();
      if (updateCountRef.current >= 200) {
        await compactRoom(roomId, docRef.current);
        updateCountRef.current = 0;
      }
    } catch {
      setConnected(false);
      setStatusText('Storage error');
    }

    if (origin === 'local' && syncingRef.current && socketRef.current?.readyState === WebSocket.OPEN) {
      const frame = new Uint8Array(update.length + 1);
      frame[0] = 0x03;
      frame.set(update, 1);
      socketRef.current.send(frame);
    }
  }, [roomId]);

  // WebSocket connection & room loading
  const connect = useCallback((manual = false) => {
    if (manual && reconnectTimerRef.current !== null) {
      clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
    socketRef.current?.close();
    syncingRef.current = false;
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws = new WebSocket(`${protocol}//${location.host}/ws/${encodeURIComponent(roomId)}`);
    ws.binaryType = 'arraybuffer';
    socketRef.current = ws;
    setConnected(false);
    setStatusText('Connecting');

    ws.onopen = () => {
      const hello: Hello = { protocol_version: PROTOCOL_VERSION, room_id: roomId, version: docRef.current.oplogVersion() };
      const payload = new TextEncoder().encode(JSON.stringify(hello));
      const frame = new Uint8Array(payload.length + 1);
      frame[0] = 0x01;
      frame.set(payload, 1);
      ws.send(frame);
    };

    ws.onmessage = async (event) => {
      const frame = new Uint8Array(event.data);
      const tag = frame[0];
      const payloadBytes = frame.slice(1);

      if (tag === 0x02) {
        const reply = JSON.parse(new TextDecoder().decode(payloadBytes)) as SyncReply;
        if (reply.updates.length > 0) {
          const updateBytes = new Uint8Array(reply.updates);
          docRef.current.import(updateBytes);
          await persistAndPush(updateBytes, 'remote');
        }
        lastSyncedVersionRef.current = reply.version;
        syncingRef.current = true;
        backoffAttemptRef.current = 0;
        setConnected(true);
        setStatusText('Connected');
        syncDocToState();

        const outgoing = docRef.current.export({ mode: 'update', from: lastSyncedVersionRef.current });
        if (outgoing.length > 0 && ws.readyState === WebSocket.OPEN) {
          const outFrame = new Uint8Array(outgoing.length + 1);
          outFrame[0] = 0x03;
          outFrame.set(outgoing, 1);
          ws.send(outFrame);
        }
      } else if (tag === 0x03) {
        docRef.current.import(payloadBytes);
        lastSyncedVersionRef.current = docRef.current.oplogVersion();
        await persistAndPush(payloadBytes, 'remote');
        syncDocToState();
      } else if (tag === 0x04) {
        const p = JSON.parse(new TextDecoder().decode(payloadBytes));
        setPeers(prev => {
          const next = new Map(prev);
          if (p.name === undefined && p.x === undefined) {
            next.delete(p.connection_id);
          } else {
            next.set(p.connection_id, {
              name: p.name || 'Peer',
              color: p.color || '#888',
              x: p.x,
              y: p.y,
              selected_nodes: p.selected_nodes || [],
              editing_node: p.editing_node,
              lastSeen: performance.now(),
            });
          }
          return next;
        });
      } else if (tag === 0x05) {
        const err = JSON.parse(new TextDecoder().decode(payloadBytes));
        setConnected(false);
        setStatusText(err.message || 'Error');
      }
    };

    ws.onclose = () => {
      syncingRef.current = false;
      const delay = Math.min(30000, 1000 * Math.pow(2, backoffAttemptRef.current));
      setConnected(false);
      setStatusText(`Offline, retrying in ${Math.round(delay / 1000)}s`);
      reconnectTimerRef.current = window.setTimeout(() => {
        backoffAttemptRef.current++;
        connect(false);
      }, delay);
    };
  }, [roomId, persistAndPush, syncDocToState]);

  useEffect(() => {
    async function initRoom() {
      const loaded = await loadRoomDoc(roomId);
      docRef.current = loaded.doc;
      lastPersistedVersionRef.current = loaded.lastVersion;
      lastSyncedVersionRef.current = loaded.doc.oplogVersion();
      updateCountRef.current = loaded.updateCount;

      docRef.current.subscribe((event) => {
        if (event.origin === 'remote') return;
        if (event.by === 'local') {
          const update = docRef.current.export({ mode: 'update', from: lastPersistedVersionRef.current });
          persistAndPush(update, 'local');
          lastSyncedVersionRef.current = docRef.current.oplogVersion();
        }
        syncDocToState();
      });

      syncDocToState();
      connect(true);
    }
    initRoom();

    return () => {
      socketRef.current?.close();
      if (reconnectTimerRef.current) clearTimeout(reconnectTimerRef.current);
    };
  }, [roomId, connect, persistAndPush, syncDocToState]);

  // Send presence periodically
  useEffect(() => {
    const timer = setInterval(() => {
      if (!syncingRef.current || socketRef.current?.readyState !== WebSocket.OPEN) return;
      const state = {
        name: userName || 'Anonymous',
        color: userColor,
        selected_nodes: selectedNodeId ? [selectedNodeId] : [],
      };
      const payload = new TextEncoder().encode(JSON.stringify(state));
      const frame = new Uint8Array(payload.length + 1);
      frame[0] = 0x04;
      frame.set(payload, 1);
      socketRef.current.send(frame);
    }, 2000);
    return () => clearInterval(timer);
  }, [userName, userColor, selectedNodeId]);

  const handleAddNode = () => {
    const id = ulid();
    const count = rfNodes.length;
    const nodeMap = docRef.current.getMap('nodes').ensureMergeableMap(id);
    nodeMap.set('id', id);
    nodeMap.set('type', 'feature');
    nodeMap.set('title', `Feature ${count + 1}`);
    nodeMap.set('body', '');
    nodeMap.set('x', 60 + (count % 4) * 240);
    nodeMap.set('y', 60 + Math.floor(count / 4) * 160);
    nodeMap.set('status', 'todo');
    nodeMap.set('effort', 3);
    nodeMap.set('created_by', userName);
    nodeMap.set('ai_generated', false);
    nodeMap.set('source_node', null);
    docRef.current.commit();
    syncDocToState();
  };

  const handleNodesChange = useCallback((changes: NodeChange[]) => {
    setRfNodes(nds => applyNodeChanges(changes, nds));
    let committed = false;
    for (const change of changes) {
      if (change.type === 'position' && change.position && !change.dragging) {
        const nodeMap = docRef.current.getMap('nodes').get(change.id) as any;
        if (nodeMap) {
          nodeMap.set('x', change.position.x);
          nodeMap.set('y', change.position.y);
          committed = true;
        }
      }
    }
    if (committed) {
      docRef.current.commit();
    }
  }, []);

  const handleConnectEdge = useCallback((connection: Connection) => {
    if (!connection.source || !connection.target || connection.source === connection.target) return;
    const edgeId = ulid();
    const edgesMap = docRef.current.getMap('edges').ensureMergeableMap(edgeId);
    edgesMap.set('id', edgeId);
    edgesMap.set('from', connection.source);
    edgesMap.set('to', connection.target);
    edgesMap.set('kind', 'depends_on');
    docRef.current.commit();
    syncDocToState();
  }, [syncDocToState]);

  const handleEdgesChange = useCallback((changes: EdgeChange[]) => {
    setRfEdges(eds => applyEdgeChanges(changes, eds));
    let committed = false;
    for (const change of changes) {
      if (change.type === 'remove') {
        docRef.current.getMap('edges').delete(change.id);
        committed = true;
      }
    }
    if (committed) {
      docRef.current.commit();
    }
  }, []);

  const selectedNodeData = useMemo(() => {
    if (!selectedNodeId) return null;
    const nodesMap = docRef.current.getMap('nodes');
    const val = nodesMap.get(selectedNodeId);
    return val ? (val as any) : null;
  }, [selectedNodeId, rfNodes]);

  const handleUpdateSelectedNode = (field: string, value: any) => {
    if (!selectedNodeId) return;
    const nodeMap = docRef.current.getMap('nodes').get(selectedNodeId) as any;
    if (nodeMap) {
      nodeMap.set(field, value);
      docRef.current.commit();
      syncDocToState();
    }
  };

  const handleDeleteSelectedNode = () => {
    if (!selectedNodeId) return;
    docRef.current.getMap('nodes').delete(selectedNodeId);
    // Also delete connected edges
    const edgesMap = docRef.current.getMap('edges');
    for (const [edgeId, edgeVal] of edgesMap.entries()) {
      const e = edgeVal as any;
      if (e.from === selectedNodeId || e.to === selectedNodeId) {
        edgesMap.delete(edgeId);
      }
    }
    docRef.current.commit();
    setSelectedNodeId(null);
    syncDocToState();
  };

  const handleExtract = async () => {
    try {
      const docSnapshot = docRef.current.export({ mode: 'snapshot' });
      // Send binary or base64 to server extract endpoint
      const res = await fetch(`/api/rooms/${encodeURIComponent(roomId)}/extract`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/octet-stream' },
        body: docSnapshot,
      });
      if (res.ok) {
        const json = await res.json();
        setExtractProposal(json);
        setActiveTab('extract');
      } else {
        const err = await res.json();
        alert(`Extract error: ${err.message || res.statusText}`);
      }
    } catch (e: any) {
      alert(`Extract failed: ${e.message}`);
    }
  };

  return (
    <main style={{ maxWidth: '100vw', height: '100vh', display: 'flex', flexDirection: 'column', padding: '16px' }}>
      <header style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
        <div>
          <p className="eyebrow">SCAFFOLD / LOCAL-FIRST CANVAS</p>
          <h1>{roomId}</h1>
        </div>
        <div style={{ display: 'flex', gap: '16px', alignItems: 'center' }}>
          <div className={`connection ${connected ? 'online' : ''}`}>
            <span />
            {statusText}
          </div>
          <div className="active-avatars">
            <div className="avatar-badge" style={{ background: userColor }}>{userName || 'You'}</div>
            {Array.from(peers.values()).map((p, idx) => (
              <div key={idx} className="avatar-badge" style={{ background: p.color }}>{p.name}</div>
            ))}
          </div>
        </div>
      </header>

      <section className="toolbar" style={{ marginBottom: '12px' }}>
        <label>Room <input value={roomId} onChange={e => setRoomId(e.target.value)} /></label>
        <label>Name <input value={userName} onChange={e => setUserName(e.target.value)} /></label>
        <label>Color <select value={userColor} onChange={e => setUserColor(e.target.value)}>
          {PALETTE.map(c => <option key={c} value={c}>{c}</option>)}
        </select></label>
        <button onClick={() => connect(true)}>Reconnect</button>
        <button className="primary" onClick={handleAddNode}>＋ Add node</button>
        <button onClick={handleExtract} style={{ background: '#28694c', color: 'white' }}>✨ Extract Proposal</button>
        <div style={{ display: 'flex', gap: '8px', marginLeft: 'auto' }}>
          <button style={{ fontWeight: activeTab === 'canvas' ? 'bold' : 'normal' }} onClick={() => setActiveTab('canvas')}>Canvas</button>
          <button style={{ fontWeight: activeTab === 'extract' ? 'bold' : 'normal' }} onClick={() => setActiveTab('extract')}>Proposal Review</button>
        </div>
      </section>

      <div style={{ flex: 1, display: 'flex', gap: '16px', position: 'relative', overflow: 'hidden', minHeight: '500px' }}>
        <div style={{ flex: 1, height: '100%', border: '1px solid #dce6dc', borderRadius: '12px', background: '#fbfcf9', display: activeTab === 'canvas' ? 'block' : 'none' }}>
          <ReactFlow
            nodes={rfNodes}
            edges={rfEdges}
            onNodesChange={handleNodesChange}
            onEdgesChange={handleEdgesChange}
            onConnect={handleConnectEdge}
            onNodeClick={(_, node) => setSelectedNodeId(node.id)}
            fitView
          >
            <Background gap={20} size={1} />
            <Controls />
            <MiniMap />
          </ReactFlow>
        </div>

        {activeTab === 'extract' && (
          <div style={{ flex: 1, border: '1px solid #dce6dc', borderRadius: '12px', background: 'white', padding: '24px', overflowY: 'auto' }}>
            <h2>Extract Proposal Review</h2>
            {!extractProposal ? (
              <p style={{ color: '#68766f', marginTop: '16px' }}>No proposal generated yet. Click "✨ Extract Proposal" in the toolbar to run LLM extraction.</p>
            ) : (
              <div>
                <p><strong>Version:</strong> {extractProposal.version}</p>
                <p><strong>Summary:</strong> {extractProposal.summary}</p>
                <h3>Ordered Tasks</h3>
                <ol style={{ paddingLeft: '20px' }}>
                  {extractProposal.tasks?.map((t: any, i: number) => (
                    <li key={i} style={{ marginBottom: '12px' }}>
                      <strong>{t.title}</strong> ({t.effort}h) - {t.status}
                      <p style={{ margin: '4px 0 0', color: '#555' }}>{t.body}</p>
                    </li>
                  ))}
                </ol>
              </div>
            )}
          </div>
        )}

        {selectedNodeId && selectedNodeData && activeTab === 'canvas' && (
          <aside style={{ width: '320px', border: '1px solid #dce6dc', borderRadius: '12px', background: 'white', padding: '20px', display: 'flex', flexDirection: 'column', gap: '14px', boxShadow: '0 4px 14px rgba(0,0,0,0.05)', zIndex: 10 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <span className="eyebrow">NODE PROPERTIES</span>
              <button onClick={() => setSelectedNodeId(null)} style={{ border: 'none', background: 'transparent', cursor: 'pointer', fontSize: '16px' }}>✕</button>
            </div>

            <label style={{ display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '12px', color: '#68766f' }}>
              Title
              <input value={selectedNodeData.title || ''} onChange={e => handleUpdateSelectedNode('title', e.target.value)} style={{ width: '100%' }} />
            </label>

            <label style={{ display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '12px', color: '#68766f' }}>
              Type
              <select value={selectedNodeData.type || 'feature'} onChange={e => handleUpdateSelectedNode('type', e.target.value)} style={{ width: '100%', padding: '9px', borderRadius: '7px', border: '1px solid #dce6dc' }}>
                <option value="feature">Feature</option>
                <option value="service">Service</option>
                <option value="task">Task</option>
                <option value="risk">Risk</option>
                <option value="note">Note</option>
              </select>
            </label>

            <label style={{ display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '12px', color: '#68766f' }}>
              Status
              <select value={selectedNodeData.status || 'todo'} onChange={e => handleUpdateSelectedNode('status', e.target.value)} style={{ width: '100%', padding: '9px', borderRadius: '7px', border: '1px solid #dce6dc' }}>
                <option value="todo">Todo</option>
                <option value="doing">Doing</option>
                <option value="done">Done</option>
                <option value="blocked">Blocked</option>
              </select>
            </label>

            <label style={{ display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '12px', color: '#68766f' }}>
              Effort (hours)
              <input type="number" value={selectedNodeData.effort ?? 0} onChange={e => handleUpdateSelectedNode('effort', parseFloat(e.target.value) || 0)} style={{ width: '100%' }} />
            </label>

            <label style={{ display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '12px', color: '#68766f' }}>
              Description / Body
              <textarea rows={4} value={selectedNodeData.body || ''} onChange={e => handleUpdateSelectedNode('body', e.target.value)} style={{ width: '100%', padding: '9px', borderRadius: '7px', border: '1px solid #dce6dc', fontFamily: 'inherit', fontSize: '12px' }} />
            </label>

            <div style={{ marginTop: 'auto', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <span style={{ fontSize: '10px', color: '#999', fontFamily: 'monospace' }}>{selectedNodeId}</span>
              <button onClick={handleDeleteSelectedNode} style={{ color: '#d9534f', borderColor: '#d9534f' }}>Delete node</button>
            </div>
          </aside>
        )}
      </div>
    </main>
  );
}

createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
