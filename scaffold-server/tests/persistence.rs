use loro::{ExportMode, LoroDoc, VersionVector};
use scaffold_server::room::RoomManager;
use scaffold_server::store::Store;
use scaffold_server::sync_protocol::{self, PresenceState};
use tempfile::tempdir;

#[tokio::test]
async fn test_persistence_restart_and_reload() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let database_url = format!("sqlite://{}", db_path.display());

    // Phase 1: Start server store, make edits, and shutdown
    let store = Store::new(&database_url).await.unwrap();
    let rooms = RoomManager::new(Some(store));

    let room = rooms.get_or_create("room-1").await;
    let _ = room.lock().await;
    let doc = LoroDoc::new();
    doc.set_peer_id(1).unwrap();
    doc.get_map("root").insert("key1", "value1").unwrap();
    let bytes = doc
        .export(ExportMode::updates(&VersionVector::default()))
        .unwrap();
    rooms.apply_update("room-1", 1, &bytes).await.unwrap();

    let json_before = rooms.state_json("room-1").await.unwrap();

    // Phase 2: Restart store and room manager with same database
    drop(rooms);

    let store2 = Store::new(&database_url).await.unwrap();
    let rooms2 = RoomManager::new(Some(store2));

    let json_after = rooms2.state_json("room-1").await.unwrap();

    assert_eq!(json_before, json_after);
    assert_eq!(json_after, serde_json::json!({"root": {"key1": "value1"}}));
}

#[tokio::test]
async fn test_presence_does_not_touch_sqlite() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("presence.db");
    let database_url = format!("sqlite://{}", db_path.display());

    let store = Store::new(&database_url).await.unwrap();
    let rooms = RoomManager::new(Some(store.clone()));

    let room = rooms.get_or_create("room-presence").await;
    let room_guard = room.lock().await;

    // Simulate presence traffic
    let state = PresenceState {
        name: "TestUser".into(),
        color: "#e06c75".into(),
        x: Some(100.0),
        y: Some(200.0),
        selected_nodes: vec!["node-1".into()],
        editing_node: None,
    };
    let frame = sync_protocol::encode_presence("999", &state).unwrap();
    let _ = room_guard.presence_updates.send((999, frame));
    drop(room_guard);

    // Verify room_updates table in SQLite is completely empty
    let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM room_updates WHERE room_id = 'room-presence'")
        .fetch_one(&store.pool)
        .await
        .unwrap();

    assert_eq!(row_count, 0);
}

#[tokio::test]
async fn test_compaction_and_reload() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("compact.db");
    let database_url = format!("sqlite://{}", db_path.display());

    let store = Store::new(&database_url).await.unwrap();
    let rooms = RoomManager::new(Some(store));

    let room = rooms.get_or_create("room-2").await;
    let _ = room.lock().await;
    let doc = LoroDoc::new();
    doc.set_peer_id(1).unwrap();
    doc.get_map("root").insert("item", "100").unwrap();
    let bytes = doc
        .export(ExportMode::updates(&VersionVector::default()))
        .unwrap();
    rooms.apply_update("room-2", 1, &bytes).await.unwrap();

    // Explicitly compact
    rooms.compact("room-2").await.unwrap();

    let json_before = rooms.state_json("room-2").await.unwrap();

    // Restart and verify state survives compaction
    drop(rooms);
    let store2 = Store::new(&database_url).await.unwrap();
    let rooms2 = RoomManager::new(Some(store2));

    let json_after = rooms2.state_json("room-2").await.unwrap();
    assert_eq!(json_before, json_after);
    assert_eq!(json_after, serde_json::json!({"root": {"item": "100"}}));
}
