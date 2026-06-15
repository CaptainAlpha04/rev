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
        let diffs = self.page_tracker.get_dirty_pages()?;
        let hash = self.merkle.insert(self.last_hash, step_id, &diffs)?;

        self.last_hash = hash;
        self.steps.push(step_id);
        self.current_step = step_id;

        Ok(hash)
    }

    /// Retrieve the full memory state at any historical step.
    /// (Reconstruction algorithm is implemented in Phase 2).
    pub fn state_at(&self, step_id: u64) -> Result<MemoryState, RevError> {
        Ok(MemoryState {
            step_id,
            pages: Vec::new(),
        })
    }

    /// Returns ordered list of all step IDs recorded so far.
    pub fn steps(&self) -> &[u64] {
        &self.steps
    }
}
