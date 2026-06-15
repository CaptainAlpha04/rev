#![allow(clippy::manual_is_multiple_of)]

pub mod filter;
pub mod merkle;
pub mod page_tracker;

use merkle::MerkleDAG;
use page_tracker::PageTracker;
use rev_core::error::RevError;
use rev_core::types::{DeltaHash, MemoryState};

pub struct DeltaEngine {
    merkle: MerkleDAG,
    page_tracker: PageTracker,
    current_step: u64,
    steps: Vec<u64>,
    last_hash: DeltaHash,
}

impl DeltaEngine {
    pub fn new(pid: u32) -> Result<Self, RevError> {
        let merkle = MerkleDAG::new(pid)?;
        let page_tracker = PageTracker::new(pid)?;
        Ok(Self {
            merkle,
            page_tracker,
            current_step: 0,
            steps: Vec::new(),
            last_hash: [0u8; 32], // root parent hash is zeroed
        })
    }

    /// Called at each step boundary. Computes dirty pages, creates delta node.
    pub fn commit_step(&mut self, step_id: u64) -> Result<DeltaHash, RevError> {
        // Force a full snapshot at step 0 or at snapshot intervals
        let force_full = step_id == 0 || step_id % rev_core::constants::SNAPSHOT_INTERVAL == 0;
        let diffs = self.page_tracker.get_dirty_pages(force_full)?;
        let hash = self.merkle.insert(self.last_hash, step_id, &diffs)?;

        self.last_hash = hash;
        self.steps.push(step_id);
        self.current_step = step_id;

        Ok(hash)
    }

    /// Retrieve the full memory state at any historical step.
    pub fn state_at(&self, step_id: u64) -> Result<MemoryState, RevError> {
        // Find nearest snapshot step S <= step_id
        let mut s_step = None;
        for &step in &self.steps {
            if step <= step_id {
                if step == 0 || step % rev_core::constants::SNAPSHOT_INTERVAL == 0 {
                    s_step = Some(step);
                }
            } else {
                break;
            }
        }

        let s = s_step.ok_or_else(|| RevError::ReplayFailed {
            step: step_id,
            reason: "No base snapshot found".to_string(),
        })?;

        // 1. Load base snapshot S
        let mut page_map = std::collections::HashMap::new();
        if let Some((_, _, s_pages)) = self.merkle.get_node_by_step(s)? {
            for diff in s_pages {
                page_map.insert(diff.address, diff.after);
            }
        } else {
            return Err(RevError::ReplayFailed {
                step: step_id,
                reason: format!("Base snapshot step {} not found in database", s),
            });
        }

        // 2. Apply forward deltas from S+1 up to step_id
        for &step in &self.steps {
            if step > s && step <= step_id {
                if let Some((_, _, pages)) = self.merkle.get_node_by_step(step)? {
                    for diff in pages {
                        page_map.insert(diff.address, diff.after);
                    }
                }
            }
        }

        // 3. Convert to MemoryPage list
        let pages = page_map
            .into_iter()
            .map(|(address, content)| rev_core::types::MemoryPage { address, content })
            .collect();

        Ok(MemoryState { step_id, pages })
    }

    /// Returns ordered list of all step IDs recorded so far.
    pub fn steps(&self) -> &[u64] {
        &self.steps
    }
}
