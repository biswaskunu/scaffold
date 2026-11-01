//! Binary WebSocket framing for the room synchronization protocol.
use loro::VersionVector;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

const HELLO: u8 = 0x01;
const SYNC_REPLY: u8 = 0x02;
const UPDATE: u8 = 0x03;
const ERROR: u8 = 0x05;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hello {
    pub protocol_version: u16,
    pub room_id: String,
    pub version: VersionVector,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReply {
    pub protocol_version: u16,
    pub version: VersionVector,
    pub updates: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolError {
    pub code: String,
    pub message: String,
}

#[derive(Debug)]
pub enum ClientMessage {
    Hello(Hello),
    Update(Vec<u8>),
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame is empty")]
    Empty,
    #[error("frame exceeds maximum size")]
    TooLarge,
    #[error("unknown frame type {0:#x}")]
    UnknownType(u8),
    #[error("invalid frame payload: {0}")]
    InvalidPayload(#[from] serde_json::Error),
    #[error("message type is not allowed from a client")]
    WrongDirection,
}

pub fn decode_client(bytes: &[u8]) -> Result<ClientMessage, FrameError> {
    if bytes.is_empty() {
        return Err(FrameError::Empty);
    }
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge);
    }
    match bytes[0] {
        HELLO => Ok(ClientMessage::Hello(serde_json::from_slice(&bytes[1..])?)),
        UPDATE => Ok(ClientMessage::Update(bytes[1..].to_vec())),
        SYNC_REPLY | ERROR => Err(FrameError::WrongDirection),
        other => Err(FrameError::UnknownType(other)),
    }
}

pub fn encode_sync_reply(reply: &SyncReply) -> Result<Vec<u8>, serde_json::Error> {
    encode_json(SYNC_REPLY, reply)
}

pub fn encode_error(error: &ProtocolError) -> Result<Vec<u8>, serde_json::Error> {
    encode_json(ERROR, error)
}

pub fn encode_update(update: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(update.len() + 1);
    frame.push(UPDATE);
    frame.extend_from_slice(update);
    frame
}

fn encode_json<T: Serialize>(kind: u8, value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let mut frame = vec![kind];
    frame.extend(serde_json::to_vec(value)?);
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_frame_round_trips() {
        let hello = Hello {
            protocol_version: PROTOCOL_VERSION,
            room_id: "room-a".into(),
            version: VersionVector::default(),
        };
        let mut frame = vec![HELLO];
        frame.extend(serde_json::to_vec(&hello).unwrap());
        match decode_client(&frame).unwrap() {
            ClientMessage::Hello(decoded) => {
                assert_eq!(decoded.room_id, "room-a");
                assert_eq!(decoded.protocol_version, PROTOCOL_VERSION);
            }
            ClientMessage::Update(_) => panic!("expected hello"),
        }
    }

    #[test]
    fn rejects_empty_unknown_wrong_direction_and_oversized_frames() {
        assert!(matches!(decode_client(&[]), Err(FrameError::Empty)));
        assert!(matches!(
            decode_client(&[0xff]),
            Err(FrameError::UnknownType(0xff))
        ));
        assert!(matches!(
            decode_client(&[ERROR]),
            Err(FrameError::WrongDirection)
        ));
        assert!(matches!(
            decode_client(&vec![UPDATE; MAX_FRAME_BYTES + 1]),
            Err(FrameError::TooLarge)
        ));
    }

    #[test]
    fn mismatch_error_is_structured() {
        let encoded = encode_error(&ProtocolError {
            code: "protocol_version_mismatch".into(),
            message: "server supports protocol version 1".into(),
        })
        .unwrap();
        assert_eq!(encoded[0], ERROR);
        let decoded: ProtocolError = serde_json::from_slice(&encoded[1..]).unwrap();
        assert_eq!(decoded.code, "protocol_version_mismatch");
    }
}
