use crate::filter::{parse_maps, MemoryRegion};
use rev_core::error::RevError;
use rev_core::types::PageDiff;
use std::collections::HashMap;

#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::io::{Read, Seek, SeekFrom, Write};

pub struct PageTracker {
    _pid: u32,
    whitelist: Vec<MemoryRegion>,
    page_cache: HashMap<u64, Vec<u8>>,
}

#[cfg(target_os = "linux")]
impl PageTracker {
    pub fn new(pid: u32) -> Result<Self, RevError> {
        let whitelist = parse_maps(pid)?;
        let mut tracker = Self {
            _pid: pid,
            whitelist,
            page_cache: HashMap::new(),
        };
        // Reset soft-dirty bits
        tracker.clear_soft_dirty()?;
        // Populate initial contents cache
        tracker.populate_initial_cache()?;
        Ok(tracker)
    }

    fn clear_soft_dirty(&mut self) -> Result<(), RevError> {
        let path = format!("/proc/{}/clear_refs", self._pid);
        let mut file = File::create(&path)?;
        file.write_all(b"4\n")?; // Write 4 to clear soft-dirty bits
        Ok(())
    }

    fn populate_initial_cache(&mut self) -> Result<(), RevError> {
        let mem_path = format!("/proc/{}/mem", self._pid);
        let mut mem_file = File::open(&mem_path)?;

        for region in &self.whitelist {
            let mut addr = region.start;
            while addr < region.end {
                if let Ok(content) = read_page(&mut mem_file, addr) {
                    self.page_cache.insert(addr, content);
                }
                addr += 4096;
            }
        }
        Ok(())
    }

    pub fn get_dirty_pages(&mut self) -> Result<Vec<PageDiff>, RevError> {
        let pagemap_path = format!("/proc/{}/pagemap", self._pid);
        let mut pagemap_file = File::open(&pagemap_path)?;

        let mem_path = format!("/proc/{}/mem", self._pid);
        let mut mem_file = File::open(&mem_path)?;

        let mut diffs = Vec::new();

        for region in &self.whitelist {
            let mut addr = region.start;
            while addr < region.end {
                let pagemap_offset = (addr / 4096) * 8;
                if pagemap_file.seek(SeekFrom::Start(pagemap_offset)).is_ok() {
                    let mut entry_bytes = [0u8; 8];
                    if pagemap_file.read_exact(&mut entry_bytes).is_ok() {
                        let entry = u64::from_le_bytes(entry_bytes);
                        let is_soft_dirty = (entry & (1 << 55)) != 0;

                        if is_soft_dirty {
                            if let Ok(after) = read_page(&mut mem_file, addr) {
                                let before = self
                                    .page_cache
                                    .get(&addr)
                                    .cloned()
                                    .unwrap_or_else(|| vec![0; 4096]);

                                if before != after {
                                    diffs.push(PageDiff {
                                        address: addr,
                                        before: before.clone(),
                                        after: after.clone(),
                                    });
                                    self.page_cache.insert(addr, after);
                                }
                            }
                        }
                    }
                }
                addr += 4096;
            }
        }

        self.clear_soft_dirty()?;
        Ok(diffs)
    }
}

#[cfg(target_os = "linux")]
fn read_page(file: &mut File, address: u64) -> Result<Vec<u8>, std::io::Error> {
    file.seek(SeekFrom::Start(address))?;
    let mut page = vec![0; 4096];
    file.read_exact(&mut page)?;
    Ok(page)
}

#[cfg(not(target_os = "linux"))]
impl PageTracker {
    pub fn new(pid: u32) -> Result<Self, RevError> {
        let whitelist = parse_maps(pid)?;
        Ok(Self {
            _pid: pid,
            whitelist,
            page_cache: HashMap::new(),
        })
    }

    pub fn get_dirty_pages(&mut self) -> Result<Vec<PageDiff>, RevError> {
        // Mock dirty page detection on Windows:
        // Pretend a single page is edited at each checkpoint.
        let mut diffs = Vec::new();
        if let Some(region) = self.whitelist.first() {
            let addr = region.start;
            let before = self
                .page_cache
                .get(&addr)
                .cloned()
                .unwrap_or_else(|| vec![0u8; 4096]);
            let mut after = before.clone();
            after[0] = after[0].wrapping_add(1);

            diffs.push(PageDiff {
                address: addr,
                before,
                after: after.clone(),
            });
            self.page_cache.insert(addr, after);
        }
        Ok(diffs)
    }
}
