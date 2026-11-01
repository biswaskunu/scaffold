use crate::{
    room::RoomManager,
    sync_protocol::{self, ClientMessage, ProtocolError, SyncReply, PROTOCOL_VERSION},
};
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::broadcast;

static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);

pub async fn ws_handler(mut socket: WebSocket, rooms: RoomManager) {
    let first = match socket.recv().await {
        Some(Ok(Message::Binary(frame))) => frame,
        _ => return,
    };
    let hello = match sync_protocol::decode_client(&first) {
        Ok(ClientMessage::Hello(hello)) => hello,
        Ok(_) => {
            send_error(&mut socket, "expected_hello", "first frame must be Hello").await;
            return;
        }
        Err(error) => {
            send_error(&mut socket, "invalid_frame", &error.to_string()).await;
            return;
        }
    };
    if hello.protocol_version != PROTOCOL_VERSION {
        send_error(
            &mut socket,
            "protocol_version_mismatch",
            &format!("server supports protocol version {PROTOCOL_VERSION}"),
        )
        .await;
        return;
    }
    let room = rooms.get_or_create(&hello.room_id).await;
    let connection_id = NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed);
    let (mut updates_rx, initial_frame) = {
        let room = room.lock().await;
        let updates_rx = room.updates.subscribe();
        let delta = match room.export_since(&hello.version) {
            Ok(delta) => delta,
            Err(error) => {
                send_error(&mut socket, "sync_failed", &error.to_string()).await;
                return;
            }
        };
        let reply = SyncReply {
            protocol_version: PROTOCOL_VERSION,
            version: room.doc.oplog_vv(),
            updates: delta,
        };
        let frame = match sync_protocol::encode_sync_reply(&reply) {
            Ok(frame) => frame,
            Err(error) => {
                send_error(&mut socket, "sync_failed", &error.to_string()).await;
                return;
            }
        };
        (updates_rx, frame)
    };
    if socket.send(Message::Binary(initial_frame)).await.is_err() {
        return;
    }

    let (mut sender, mut receiver) = socket.split();
    let writer = tokio::spawn(async move {
        loop {
            match updates_rx.recv().await {
                Ok((source, update)) => {
                    if source != connection_id
                        && sender
                            .send(Message::Binary(sync_protocol::encode_update(&update)))
                            .await
                            .is_err()
                    {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    send_error(
                        &mut sender,
                        "resync_required",
                        "client fell behind; reconnect to resync",
                    )
                    .await;
                    break;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    loop {
        match receiver.next().await {
            Some(Ok(Message::Binary(frame))) => {
                let message = match sync_protocol::decode_client(&frame) {
                    Ok(message) => message,
                    Err(_error) => {
                        writer.abort();
                        return;
                    }
                };
                match message {
                    ClientMessage::Hello(_) => {
                        writer.abort();
                        return;
                    }
                    ClientMessage::Update(bytes) => {
                        if bytes.is_empty() || bytes.len() + 1 > sync_protocol::MAX_FRAME_BYTES {
                            writer.abort();
                            return;
                        }
                        match rooms
                            .apply_update(&hello.room_id, connection_id, &bytes)
                            .await
                        {
                            Ok(()) => {}
                            Err(_error) => {
                                writer.abort();
                                return;
                            }
                        }
                    }
                }
            }
            Some(Ok(Message::Close(_))) | None | Some(Err(_)) => {
                writer.abort();
                return;
            }
            Some(Ok(_)) => {
                writer.abort();
                return;
            }
        }
        if writer.is_finished() {
            return;
        }
    }
}

async fn send_error<S>(socket: &mut S, code: &str, message: &str)
where
    S: futures_util::Sink<Message> + Unpin,
    S::Error: std::fmt::Debug,
{
    let error = ProtocolError {
        code: code.to_owned(),
        message: message.to_owned(),
    };
    if let Ok(frame) = sync_protocol::encode_error(&error) {
        let _ = socket.send(Message::Binary(frame)).await;
    }
}
