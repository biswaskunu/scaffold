use loro::{ExportMode, LoroDoc, ToJson, VersionVector};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, Mutex};

pub const ROOM_CHANNEL_CAPACITY: usize = 256;

pub struct Room {
    pub doc: LoroDoc,
    pub updates: broadcast::Sender<(u64, Vec<u8>)>,
    pub version: VersionVector,
}

impl Room {
    fn new() -> Self {
        let (updates, _) = broadcast::channel(ROOM_CHANNEL_CAPACITY);
        Self {
            doc: LoroDoc::new(),
            updates,
            version: VersionVector::default(),
        }
    }

    pub fn export_since(&self, version: &VersionVector) -> Result<Vec<u8>, loro::LoroEncodeError> {
        self.doc.export(ExportMode::updates(version))
    }

    pub fn updates_since_last_broadcast(
        &mut self,
    ) -> Result<Option<Vec<u8>>, loro::LoroEncodeError> {
        let current = self.doc.oplog_vv();
        if current == self.version {
            return Ok(None);
        }
        let updates = self.doc.export(ExportMode::updates(&self.version))?;
        self.version = current;
        Ok((!updates.is_empty()).then_some(updates))
    }
}

#[derive(Clone, Default)]
pub struct RoomManager {
    rooms: Arc<Mutex<HashMap<String, Arc<Mutex<Room>>>>>,
}

impl RoomManager {
    pub async fn get_or_create(&self, id: &str) -> Arc<Mutex<Room>> {
        let mut rooms = self.rooms.lock().await;
        rooms
            .entry(id.to_owned())
            .or_insert_with(|| Arc::new(Mutex::new(Room::new())))
            .clone()
    }

    pub async fn room_count(&self) -> usize {
        self.rooms.lock().await.len()
    }

    pub async fn state_json(&self, id: &str) -> Option<serde_json::Value> {
        let room = self.rooms.lock().await.get(id).cloned()?;
        let room = room.lock().await;
        Some(room.doc.get_deep_value().to_json_value())
    }

    pub async fn import_update(&self, id: &str, bytes: &[u8]) -> Result<(), String> {
        let room = self.get_or_create(id).await;
        let mut room = room.lock().await;
        let before = room.doc.oplog_vv();
        room.doc.import(bytes).map_err(|error| error.to_string())?;
        if room.doc.oplog_vv() != before {
            if let Some(updates) = room
                .updates_since_last_broadcast()
                .map_err(|error| error.to_string())?
            {
                let _ = room.updates.send((u64::MAX, updates));
            }
        }
        Ok(())
    }

    pub async fn apply_update(&self, id: &str, sender_id: u64, bytes: &[u8]) -> Result<(), String> {
        let room = self.get_or_create(id).await;
        let mut room = room.lock().await;
        let before = room.doc.oplog_vv();
        room.doc.import(bytes).map_err(|error| error.to_string())?;
        if room.doc.oplog_vv() != before {
            if let Some(delta) = room
                .updates_since_last_broadcast()
                .map_err(|error| error.to_string())?
            {
                let _ = room.updates.send((sender_id, delta));
            }
        }
        Ok(())
    }
}
