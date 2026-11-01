use crate::{room::RoomManager, ws_handler::ws_handler};
use axum::{
    extract::{ws::WebSocketUpgrade, Path, State},
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;

pub fn router() -> Router {
    router_with_rooms(RoomManager::default())
}

pub fn router_with_rooms(rooms: RoomManager) -> Router {
    Router::new()
        .route("/ws/:room_id", get(websocket))
        .route("/api/rooms", post(create_room))
        .route("/api/rooms/:id", get(get_room))
        .with_state(rooms)
}

async fn websocket(
    State(rooms): State<RoomManager>,
    Path(_room_id): Path<String>,
    upgrade: WebSocketUpgrade,
) -> axum::response::Response {
    upgrade.on_upgrade(move |socket| ws_handler(socket, rooms))
}

async fn create_room() -> Json<Value> {
    Json(serde_json::json!({
        "id": "test-room-123",
        "name": "untitled-room",
        "created_at": chrono::Utc::now().to_rfc3339()
    }))
}

async fn get_room() -> Json<Value> {
    Json(serde_json::json!({}))
}
