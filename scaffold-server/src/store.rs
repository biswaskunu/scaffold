use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::str::FromStr;

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePool::connect_with(options).await?;
        
        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| sqlx::Error::Configuration(e.to_string().into()))?;

        Ok(Self { pool })
    }

    pub async fn ensure_room(&self, room_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO rooms (id) VALUES (?) ON CONFLICT(id) DO NOTHING"
        )
        .bind(room_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn append_update(&self, room_id: &str, update_data: &[u8], sequence: i64) -> Result<(), sqlx::Error> {
        self.ensure_room(room_id).await?;
        sqlx::query(
            "INSERT INTO room_updates (room_id, update_data, sequence) VALUES (?, ?, ?)"
        )
        .bind(room_id)
        .bind(update_data)
        .bind(sequence)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_room_data(&self, room_id: &str) -> Result<Option<(Vec<u8>, i64, Vec<Vec<u8>>)>, sqlx::Error> {
        // Returns Option<(snapshot_data, last_update_id, subsequent_updates)>
        let snapshot_row: Option<(Vec<u8>, i64)> = sqlx::query_as(
            "SELECT snapshot_data, last_update_id FROM room_snapshots WHERE room_id = ? AND kind = 'compacted'"
        )
        .bind(room_id)
        .fetch_optional(&self.pool)
        .await?;

        let (snapshot_data, last_update_id) = match snapshot_row {
            Some((data, id)) => (data, id),
            None => {
                // If no snapshot, load all updates starting from sequence 0 (or 1)
                (Vec::new(), -1)
            }
        };

        let updates: Vec<(Vec<u8>,)> = sqlx::query_as(
            "SELECT update_data FROM room_updates WHERE room_id = ? AND sequence > ? ORDER BY id ASC"
        )
        .bind(room_id)
        .bind(last_update_id)
        .fetch_all(&self.pool)
        .await?;

        let update_bytes = updates.into_iter().map(|u| u.0).collect();

        if snapshot_data.is_empty() && update_bytes.is_empty() {
            Ok(None)
        } else {
            Ok(Some((snapshot_data, last_update_id, update_bytes)))
        }
    }

    pub async fn compact(
        &self,
        room_id: &str,
        snapshot_data: &[u8],
        covered_seq: i64,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO room_snapshots (room_id, snapshot_data, last_update_id, kind) VALUES (?, ?, ?, 'compacted') \
             ON CONFLICT(room_id) DO UPDATE SET snapshot_data = excluded.snapshot_data, last_update_id = excluded.last_update_id"
        )
        .bind(room_id)
        .bind(snapshot_data)
        .bind(covered_seq)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "DELETE FROM room_updates WHERE room_id = ? AND sequence <= ?"
        )
        .bind(room_id)
        .bind(covered_seq)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
}
