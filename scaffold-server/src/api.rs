use axum::{
    routing::{get, post},
    Json,
    Router,
};
use serde_json::Value;

pub fn router() -> Router {
    Router::new()
        .route("/api/rooms", post(create_room))
        .route("/api/rooms/:id", get(get_room))
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