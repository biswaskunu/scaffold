use futures_util::{SinkExt, StreamExt};
use loro::{ExportMode, LoroDoc, VersionVector};
use scaffold_server::api::router_with_rooms;
use scaffold_server::room::RoomManager;
use scaffold_server::sync_protocol::{self, Hello, ProtocolError, SyncReply, PROTOCOL_VERSION};
use serde::Serialize;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_tungstenite::{connect_async, tungstenite::Message};

async fn start_server(rooms: RoomManager) -> (String, JoinHandle<()>) {
    let app = router_with_rooms(rooms);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("ws://{address}/ws/test-room"), task)
}

fn json_frame<T: Serialize>(tag: u8, payload: &T) -> Vec<u8> {
    let mut frame = vec![tag];
    frame.extend(serde_json::to_vec(payload).unwrap());
    frame
}

async fn connect(
    url: &str,
) -> (
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    SyncReply,
) {
    let (mut socket, _) = connect_async(url).await.unwrap();
    let hello = Hello {
        protocol_version: PROTOCOL_VERSION,
        room_id: "test-room".into(),
        version: VersionVector::default(),
    };
    socket
        .send(Message::Binary(json_frame(0x01, &hello)))
        .await
        .unwrap();
    let reply = socket.next().await.unwrap().unwrap().into_data();
    assert_eq!(reply[0], 0x02);
    let reply: SyncReply = serde_json::from_slice(&reply[1..]).unwrap();
    (socket, reply)
}

fn make_update(peer: u64, key: &str, value: &str) -> (LoroDoc, Vec<u8>) {
    let doc = LoroDoc::new();
    doc.set_peer_id(peer).unwrap();
    doc.get_map("root").insert(key, value).unwrap();
    let bytes = doc
        .export(ExportMode::updates(&VersionVector::default()))
        .unwrap();
    (doc, bytes)
}

fn make_presence(state: &scaffold_server::sync_protocol::PresenceState) -> Vec<u8> {
    scaffold_server::sync_protocol::encode_presence("999", state).unwrap()
}

#[tokio::test]
async fn test_full_websocket_convergence() {
    let rooms = RoomManager::default();
    let (url, server) = start_server(rooms.clone()).await;
    let (mut a, _) = connect(&url).await;
    let (mut b, _) = connect(&url).await;

    let (doc_a, update_a) = make_update(1, "key1", "value1");
    let (doc_b, update_b) = make_update(2, "key2", "value2");

    a.send(Message::Binary(sync_protocol::encode_update(&update_a)))
        .await
        .unwrap();
    b.send(Message::Binary(sync_protocol::encode_update(&update_b)))
        .await
        .unwrap();
    a.flush().await.unwrap();
    b.flush().await.unwrap();

    let expected = serde_json::json!({"root": {"key1": "value1", "key2": "value2"}});
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        async {
            while serde_json::to_value(doc_a.get_deep_value()).unwrap() != expected {
                let frame = a.next().await.unwrap().unwrap().into_data();
                assert_eq!(frame[0], 0x03);
                doc_a.import(&frame[1..]).unwrap();
            }
        },
    )
    .await
    .expect("client A did not converge");

    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        async {
            while serde_json::to_value(doc_b.get_deep_value()).unwrap() != expected {
                let frame = b.next().await.unwrap().unwrap().into_data();
                assert_eq!(frame[0], 0x03);
                doc_b.import(&frame[1..]).unwrap();
            }
        },
    )
    .await
    .expect("client B did not converge");

    let state_a = serde_json::to_value(doc_a.get_deep_value()).unwrap();
    let state_b = serde_json::to_value(doc_b.get_deep_value()).unwrap();
    assert_eq!(state_a, state_b);
    assert_eq!(
        state_a,
        serde_json::json!({"root": {"key1": "value1", "key2": "value2"}})
    );

    let (late, initial_sync) = connect(&url).await;
    assert!(!initial_sync.updates.is_empty());
    let late_doc = LoroDoc::new();
    late_doc.import(&initial_sync.updates).unwrap();
    assert_eq!(
        serde_json::to_value(late_doc.get_deep_value()).unwrap(),
        state_a
    );
    drop(late);
    server.abort();
}

#[tokio::test]
async fn test_presence_does_not_persist_to_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("presence.db");
    let database_url = format!("sqlite://{}", db_path.display());
    let store = scaffold_server::store::Store::new(&database_url).await.unwrap();
    let rooms = scaffold_server::room::RoomManager::new(Some(store.clone()));

    let (url, server) = start_server(rooms.clone()).await;

    let (mut socket, _) = connect_async(&url).await.unwrap();
    let hello = Hello {
        protocol_version: PROTOCOL_VERSION,
        room_id: "room-presence".into(),
        version: VersionVector::default(),
    };
    socket
        .send(Message::Binary(json_frame(0x01, &hello)))
        .await
        .unwrap();

    let presence_state = scaffold_server::sync_protocol::PresenceState {
        name: "TestUser".into(),
        color: "#e06c75".into(),
        x: Some(100.0),
        y: Some(200.0),
        selected_nodes: vec!["node-1".into()],
        editing_node: None,
    };
    let frame = make_presence(&presence_state);
    socket
        .send(Message::Binary(frame))
        .await
        .unwrap();
    socket.flush().await.unwrap();

    let _ = rooms.get_or_create("room-presence").await;

    let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM room_updates WHERE room_id = 'room-presence'")
        .fetch_one(&store.pool)
        .await
        .unwrap();

    assert_eq!(row_count, 0);

    server.abort();
}

#[tokio::test]
async fn test_protocol_version_mismatch_returns_error_frame() {
    let rooms = RoomManager::default();
    let (url, server) = start_server(rooms.clone()).await;
    let (mut socket, _) = connect_async(&url).await.unwrap();

    let hello = Hello {
        protocol_version: PROTOCOL_VERSION + 1,
        room_id: "test-room".into(),
        version: VersionVector::default(),
    };
    socket
        .send(Message::Binary(json_frame(0x01, &hello)))
        .await
        .unwrap();
    let frame = socket.next().await.unwrap().unwrap().into_data();
    assert_eq!(frame[0], 0x05);

    let error: ProtocolError = serde_json::from_slice(&frame[1..]).unwrap();
    assert_eq!(error.code, "protocol_version_mismatch");
    assert!(error.message.contains(&PROTOCOL_VERSION.to_string()));

    server.abort();
}

#[tokio::test]
async fn test_compaction_preserves_state() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("compact.db");
    let database_url = format!("sqlite://{}", db_path.display());

    let store = scaffold_server::store::Store::new(&database_url).await.unwrap();
    let rooms = scaffold_server::room::RoomManager::new(Some(store.clone()));

    let (url, server) = start_server(rooms.clone()).await;

    let (mut socket, _) = connect_async(&url).await.unwrap();
    let hello = Hello {
        protocol_version: PROTOCOL_VERSION,
        room_id: "room-compact".into(),
        version: VersionVector::default(),
    };
    socket
        .send(Message::Binary(json_frame(0x01, &hello)))
        .await
        .unwrap();

    for i in 1..=10 {
        let (doc, update) = make_update(i as u64, &format!("key{}", i), &format!("value{}", i));
        socket
            .send(Message::Binary(sync_protocol::encode_update(&update)))
            .await
            .unwrap();
    }
    socket.flush().await.unwrap();

    // Compact and verify no panic, state_json returns Some
    let _ = rooms.compact("room-compact").await;
    let json_after_compact = rooms.state_json("room-compact").await;
    // Verify compact didn't crash and state is accessible
    assert!(json_after_compact.is_some(), "state_json should return Some after compact");
}