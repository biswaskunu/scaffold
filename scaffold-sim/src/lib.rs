use loro::{LoroDoc, VersionVector};
use std::collections::{HashMap, HashSet, VecDeque};

pub struct SimulatedNetwork {
    pub drop_rate: f64,
    pub delay_ms_range: std::ops::Range<u64>,
    pub messages: VecDeque<SimMessage>,
    pub partitions: HashMap<String, HashSet<String>>,
}

#[derive(Clone)]
pub struct SimMessage {
    pub from: String,
    pub to: String,
    pub payload: Vec<u8>,
    pub deliver_at: u64,
}

impl SimulatedNetwork {
    pub fn new() -> Self {
        Self {
            drop_rate: 0.0,
            delay_ms_range: 0..10,
            messages: VecDeque::new(),
            partitions: HashMap::new(),
        }
    }

    pub fn send(&mut self, from: &str, to: &str, payload: Vec<u8>, current_time: u64) {
        if self.is_partitioned(from, to) {
            return;
        }
        self.messages.push_back(SimMessage {
            from: from.to_string(),
            to: to.to_string(),
            payload,
            deliver_at: current_time + 5,
        });
    }

    pub fn partition(&mut self, client_a: &str, client_b: &str) {
        self.partitions
            .entry(client_a.to_string())
            .or_default()
            .insert(client_b.to_string());
        self.partitions
            .entry(client_b.to_string())
            .or_default()
            .insert(client_a.to_string());
    }

    pub fn heal(&mut self, client_a: &str, client_b: &str) {
        if let Some(set) = self.partitions.get_mut(client_a) {
            set.remove(client_b);
        }
        if let Some(set) = self.partitions.get_mut(client_b) {
            set.remove(client_a);
        }
    }

    fn is_partitioned(&self, a: &str, b: &str) -> bool {
        if let Some(set) = self.partitions.get(a) {
            if set.contains(b) {
                return true;
            }
        }
        if let Some(set) = self.partitions.get(b) {
            if set.contains(a) {
                return true;
            }
        }
        false
    }

    pub fn drain_ready(&mut self, current_time: u64) -> Vec<SimMessage> {
        let mut ready = Vec::new();
        let mut remaining = VecDeque::new();
        while let Some(msg) = self.messages.pop_front() {
            if msg.deliver_at <= current_time {
                ready.push(msg);
            } else {
                remaining.push_back(msg);
            }
        }
        self.messages = remaining;
        ready
    }
}

impl Default for SimulatedNetwork {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SimClient {
    pub id: String,
    pub doc: LoroDoc,
    pub last_vv: VersionVector,
}

impl SimClient {
    pub fn new(id: &str) -> Self {
        let doc = LoroDoc::new();
        // Use seeded random u64 for peer id instead of first 8 ULID bytes
        // (which are mostly timestamp and can cause collisions)
        let val: u64 = rand::random();
        doc.set_peer_id(val).unwrap();
        let last_vv = doc.oplog_vv();
        Self {
            id: id.to_string(),
            doc,
            last_vv,
        }
    }

    pub fn export_update(&self) -> Vec<u8> {
        self.doc
            .export(loro::ExportMode::updates(&self.last_vv))
            .unwrap()
    }

    pub fn import_update(&mut self, bytes: &[u8]) {
        self.doc.import(bytes).unwrap();
        self.last_vv = self.doc.oplog_vv();
    }
}
