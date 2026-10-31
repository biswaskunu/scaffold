use axum::{
    extract::ws::{self, WebSocket, WebSocketMessage},
    State,
};
use std::sync::Arc;

use scaffold_graph::Graph;

pub async fn ws_handler(
    State(graph): State<Graph>,
    ws: WebSocket,
) -> Result<(), ws::Error> {
    let (mut sender, mut receiver) = ws.split();

    while let Some(msg) = receiver.recv().await {
        let msg = msg?;
        match msg {
            WebSocketMessage::Text(text) => {
                sender.send(WebSocketMessage::Text(text)).await?;
            }
            WebSocketMessage::Binary(data) => {
                sender.send(WebSocketMessage::Binary(data)).await?;
            }
            WebSocketMessage::Close(_) => {
                return Ok(());
            }
            _ => {}
        }
    }

    Ok(())
}