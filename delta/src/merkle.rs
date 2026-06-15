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

    /// Retrieve delta details for a range of steps (inclusive)
    pub fn get_nodes_in_range(
        &self,
        start_step: u64,
        end_step: u64,
    ) -> Result<Vec<(u64, Vec<PageDiff>)>, RevError> {
        let mut stmt = self
            .conn
            .prepare("SELECT step_id, pages FROM deltas WHERE step_id >= ?1 AND step_id <= ?2 ORDER BY step_id ASC")?;

        let mut rows = stmt.query(rusqlite::params![start_step, end_step])?;
        let mut results = Vec::new();

        while let Some(row) = rows.next()? {
            let step_id: u64 = row.get(0)?;
            let pages_vec: Vec<u8> = row.get(1)?;
            let pages: Vec<PageDiff> = bincode::deserialize(&pages_vec).map_err(|e| {
                RevError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            })?;
            results.push((step_id, pages));
        }

        Ok(results)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_dag_range_query() {
        let pid = 99999;
        let mut merkle = MerkleDAG::new(pid).unwrap();

        let diffs1 = vec![
            PageDiff {
                address: 0x1000,
                before: vec![0; 4],
                after: vec![1; 4],
            }
        ];
        let diffs2 = vec![
            PageDiff {
                address: 0x2000,
                before: vec![0; 4],
                after: vec![2; 4],
            }
        ];

        let hash0 = [0u8; 32];
        let hash1 = merkle.insert(hash0, 0, &diffs1).unwrap();
        let _hash2 = merkle.insert(hash1, 1, &diffs2).unwrap();

        let nodes = merkle.get_nodes_in_range(0, 1).unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].0, 0);
        assert_eq!(nodes[0].1[0].address, 0x1000);
        assert_eq!(nodes[1].0, 1);
        assert_eq!(nodes[1].1[0].address, 0x2000);

        // Cleanup DB file
        let db_path = get_db_path(pid);
        drop(merkle);
        let _ = std::fs::remove_file(db_path);
    }
}
