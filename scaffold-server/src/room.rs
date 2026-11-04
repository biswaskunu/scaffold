use crate::store::Store;
use loro::{ExportMode, LoroDoc, ToJson, VersionVector};
use std::time::Instant;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, Mutex};

pub const ROOM_CHANNEL_CAPACITY: usize = 256;

pub struct Room {
    pub doc: LoroDoc,
    pub updates: broadcast::Sender<(u64, Vec<u8>)>,
    pub presence_updates: broadcast::Sender<(u64, Vec<u8>)>,
    pub version: VersionVector,
    pub updates_since_compaction: usize,
    pub last_activity: Instant,
}

impl Room {
    fn new() -> Self {
        let (updates, _) = broadcast::channel(ROOM_CHANNEL_CAPACITY);
        let (presence_updates, _) = broadcast::channel(ROOM_CHANNEL_CAPACITY);
        Self {
            doc: LoroDoc::new(),
            updates,
            presence_updates,
            version: VersionVector::default(),
            updates_since_compaction: 0,
            last_activity: Instant::now(),
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

#[derive(Clone)]
pub struct RoomManager {
    rooms: Arc<Mutex<HashMap<String, Arc<Mutex<Room>>>>>,
    store: Option<Store>,
    compact_after_updates: usize,
    compact_idle_secs: u64,
}

impl Default for RoomManager {
    fn default() -> Self {
        Self::new(None)
    }
}

impl RoomManager {
    pub fn new(store: Option<Store>) -> Self {
        let compact_after_updates = std::env::var("COMPACT_AFTER_UPDATES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(500);

        let compact_idle_secs = std::env::var("COMPACT_IDLE_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        let manager = Self {
            rooms: Arc::new(Mutex::new(HashMap::new())),
            store,
            compact_after_updates,
            compact_idle_secs,
        };

        manager.spawn_compaction_timer();
        manager
    }

    pub async fn get_or_create(&self, id: &str) -> Arc<Mutex<Room>> {
        let mut rooms = self.rooms.lock().await;
        if let Some(room) = rooms.get(id) {
            return room.clone();
        }

        let mut room = Room::new();

        if let Some(ref store) = self.store {
            if let Ok(Some((snapshot_data, _last_update_id, subsequent_updates))) =
                store.load_room_data(id).await
            {
                if !snapshot_data.is_empty() {
                    let _ = room.doc.import(&snapshot_data);
                }
                for update in subsequent_updates {
                    let _ = room.doc.import(&update);
                }
                room.version = room.doc.oplog_vv();
            }
        }

        let room_arc = Arc::new(Mutex::new(room));
        rooms.insert(id.to_owned(), room_arc.clone());
        room_arc
    }

    pub async fn room_count(&self) -> usize {
        self.rooms.lock().await.len()
    }

    pub async fn state_json(&self, id: &str) -> Option<serde_json::Value> {
        let room = self.get_or_create(id).await;
        let room = room.lock().await;
        Some(room.doc.get_deep_value().to_json_value())
    }

    pub async fn import_update(&self, id: &str, bytes: &[u8]) -> Result<(), String> {
        self.apply_update(id, u64::MAX, bytes).await
    }

    pub async fn apply_update(&self, id: &str, sender_id: u64, bytes: &[u8]) -> Result<(), String> {
        let room = self.get_or_create(id).await;
        let mut room = room.lock().await;
        let before = room.doc.oplog_vv();
        room.doc.import(bytes).map_err(|error| error.to_string())?;

        if room.doc.oplog_vv() != before {
            room.updates_since_compaction += 1;
            room.last_activity = Instant::now();
            let seq = room.updates_since_compaction as i64;

            if let Some(ref store) = self.store {
                let _ = store.append_update(id, bytes, seq).await;
            }

            if room.updates_since_compaction >= self.compact_after_updates {
                self.compact_room_internal(id, &mut room).await?;
            }

            if let Some(delta) = room
                .updates_since_last_broadcast()
                .map_err(|error| error.to_string())?
            {
                let _ = room.updates.send((sender_id, delta));
            }
        }
        Ok(())
    }

    pub async fn compact(&self, id: &str) -> Result<(), String> {
        let room = self.get_or_create(id).await;
        let mut room = room.lock().await;
        self.compact_room_internal(id, &mut room).await
    }

    async fn compact_room_internal(&self, id: &str, room: &mut Room) -> Result<(), String> {
        let Some(ref store) = self.store else {
            return Ok(());
        };

        let covered_seq = room.updates_since_compaction as i64;
        let snapshot_data = room
            .doc
            .export(ExportMode::Snapshot)
            .map_err(|e| e.to_string())?;

        store
            .compact(id, &snapshot_data, covered_seq)
            .await
            .map_err(|e| e.to_string())?;

        room.updates_since_compaction = 0;
        Ok(())
    }

    fn spawn_compaction_timer(&self) {
        let manager = self.clone();
        tokio::spawn(async move {
            let interval = std::time::Duration::from_secs(10);
            loop {
                tokio::time::sleep(interval).await;
                let now = Instant::now();
                let rooms_map = manager.rooms.lock().await;
                for (id, room_arc) in rooms_map.iter() {
                    let mut room = room_arc.lock().await;
                    if room.updates_since_compaction > 0
                        && now.duration_since(room.last_activity).as_secs()
                            >= manager.compact_idle_secs
                    {
                        let _ = manager.compact_room_internal(id, &mut room).await;
                    }
                }
            }
        });
    }
}
