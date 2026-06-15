use rev_core::error::RevError;
use rev_core::types::{DeltaHash, PageDiff};
use rusqlite::Connection;
use std::path::PathBuf;

pub struct MerkleDAG {
    conn: Connection,
}

impl MerkleDAG {
    pub fn new(pid: u32) -> Result<Self, RevError> {
        let db_path = get_db_path(pid);
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;

        // Initialize SQLite table for Merkle DAG nodes
        conn.execute(
            "CREATE TABLE IF NOT EXISTS deltas (
                hash BLOB PRIMARY KEY,
                parent BLOB,
                step_id INTEGER UNIQUE,
                pages BLOB
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_deltas_step_id ON deltas (step_id)",
            [],
        )?;

        Ok(Self { conn })
    }

    /// Insert a new delta node into the SQLite Merkle DAG. Returns computed hash.
    pub fn insert(
        &mut self,
        parent: DeltaHash,
        step_id: u64,
        pages: &[PageDiff],
    ) -> Result<DeltaHash, RevError> {
        let serialized_pages = bincode::serialize(pages)
            .map_err(|e| RevError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

        // Compute Blake3 hash: Blake3(parent_hash || step_id || serialized_pages)
        let mut hasher = blake3::Hasher::new();
        hasher.update(&parent);
        hasher.update(&step_id.to_le_bytes());
        hasher.update(&serialized_pages);
        let hash: DeltaHash = *hasher.finalize().as_bytes();

        self.conn.execute(
            "INSERT OR REPLACE INTO deltas (hash, parent, step_id, pages) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![&hash[..], &parent[..], step_id, serialized_pages],
        )?;

        Ok(hash)
    }

    /// Retrieve the delta details for a specific step
    pub fn get_node_by_step(
        &self,
        step_id: u64,
    ) -> Result<Option<(DeltaHash, DeltaHash, Vec<PageDiff>)>, RevError> {
        let mut stmt = self
            .conn
            .prepare("SELECT hash, parent, pages FROM deltas WHERE step_id = ?1")?;

        let mut rows = stmt.query(rusqlite::params![step_id])?;
        if let Some(row) = rows.next()? {
            let hash_vec: Vec<u8> = row.get(0)?;
            let parent_vec: Vec<u8> = row.get(1)?;
            let pages_vec: Vec<u8> = row.get(2)?;

            let mut hash = [0u8; 32];
            let mut parent = [0u8; 32];
            hash.copy_from_slice(&hash_vec[..32]);
            parent.copy_from_slice(&parent_vec[..32]);

            let pages: Vec<PageDiff> = bincode::deserialize(&pages_vec).map_err(|e| {
                RevError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            })?;

            Ok(Some((hash, parent, pages)))
        } else {
            Ok(None)
        }
    }
}

fn get_db_path(pid: u32) -> PathBuf {
    let mut path = if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
    } else if let Ok(userprofile) = std::env::var("USERPROFILE") {
        PathBuf::from(userprofile)
    } else {
        PathBuf::from(".")
    };
    path.push(".rev");
    path.push("deltas");
    path.push(format!("{}.db", pid));
    path
}
