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

#[tokio::test]
async fn concurrent_updates_converge_and_late_joiner_receives_state() {
    let rooms = RoomManager::default();
    let (url, server) = start_server(rooms.clone()).await;
    let (doc_a, update_a) = make_update(11, "a", "one");
    let (doc_b, update_b) = make_update(22, "b", "two");
    let (mut a, _) = connect(&url).await;
    let (mut b, _) = connect(&url).await;
    a.send(Message::Binary(sync_protocol::encode_update(&update_a)))
        .await
        .unwrap();
    b.send(Message::Binary(sync_protocol::encode_update(&update_b)))
        .await
        .unwrap();
    a.flush().await.unwrap();
    b.flush().await.unwrap();
    let merged_a = doc_a;
    let merged_b = doc_b;
    let expected = serde_json::json!({"root": {"a": "one", "b": "two"}});
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while serde_json::to_value(merged_a.get_deep_value()).unwrap() != expected {
            let frame = a.next().await.unwrap().unwrap().into_data();
            assert_eq!(frame[0], 0x03);
            merged_a.import(&frame[1..]).unwrap();
        }
    })
    .await
    .expect("client A did not converge");
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while serde_json::to_value(merged_b.get_deep_value()).unwrap() != expected {
            let frame = b.next().await.unwrap().unwrap().into_data();
            assert_eq!(frame[0], 0x03);
            merged_b.import(&frame[1..]).unwrap();
        }
    })
    .await
    .expect("client B did not converge");
    let state_a = serde_json::to_value(merged_a.get_deep_value()).unwrap();
    let state_b = serde_json::to_value(merged_b.get_deep_value()).unwrap();
    assert_eq!(state_a, state_b);
    assert_eq!(
        state_a,
        serde_json::json!({"root": {"a": "one", "b": "two"}})
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
async fn version_mismatch_returns_clear_error_frame() {
    let rooms = RoomManager::default();
    let (url, server) = start_server(rooms).await;
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
async fn room_manager_keeps_one_shared_document_per_id() {
    let rooms = RoomManager::default();
    let first = rooms.get_or_create("same").await;
    let second = rooms.get_or_create("same").await;
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    assert_eq!(rooms.room_count().await, 1);
}
