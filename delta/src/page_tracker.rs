#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals, dead_code)]

#[cfg(target_os = "linux")]
use crate::filter::parse_maps;
use crate::filter::MemoryRegion;
use rev_core::error::RevError;
use rev_core::types::PageDiff;
use std::collections::HashMap;

#[allow(dead_code)]
pub struct PageTracker {
    _pid: u32,
    whitelist: Vec<MemoryRegion>,
    page_cache: HashMap<u64, Vec<u8>>,
}

#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::io::{Read, Seek, SeekFrom, Write};

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

    pub fn get_dirty_pages(&mut self, force_full: bool) -> Result<Vec<PageDiff>, RevError> {
        let pagemap_path = format!("/proc/{}/pagemap", self._pid);
        let mut pagemap_file = File::open(&pagemap_path)?;

        let mem_path = format!("/proc/{}/mem", self._pid);
        let mut mem_file = File::open(&mem_path)?;

        let mut diffs = Vec::new();

        for region in &self.whitelist {
            let mut addr = region.start;
            while addr < region.end {
                let mut should_record = force_full;

                if !should_record {
                    let pagemap_offset = (addr / 4096) * 8;
                    if pagemap_file.seek(SeekFrom::Start(pagemap_offset)).is_ok() {
                        let mut entry_bytes = [0u8; 8];
                        if pagemap_file.read_exact(&mut entry_bytes).is_ok() {
                            let entry = u64::from_le_bytes(entry_bytes);
                            should_record = (entry & (1 << 55)) != 0;
                        }
                    }
                }

                if should_record {
                    if let Ok(after) = read_page(&mut mem_file, addr) {
                        let before = self
                            .page_cache
                            .get(&addr)
                            .cloned()
                            .unwrap_or_else(|| vec![0; 4096]);

                        if force_full || before != after {
                            diffs.push(PageDiff {
                                address: addr,
                                before: before.clone(),
                                after: after.clone(),
                            });
                            self.page_cache.insert(addr, after);
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

#[cfg(target_os = "windows")]
use std::ffi::c_void;

#[cfg(target_os = "windows")]
extern "system" {
    fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> *mut c_void;
    fn CloseHandle(hObject: *mut c_void) -> i32;
    fn ReadProcessMemory(
        hProcess: *mut c_void,
        lpBaseAddress: *const c_void,
        lpBuffer: *mut c_void,
        nSize: usize,
        lpNumberOfBytesRead: *mut usize,
    ) -> i32;
    fn VirtualQueryEx(
        hProcess: *mut c_void,
        lpAddress: *const c_void,
        lpBuffer: *mut MEMORY_BASIC_INFORMATION,
        dwLength: usize,
    ) -> usize;
}

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Copy, Clone)]
pub struct MEMORY_BASIC_INFORMATION {
    pub BaseAddress: *mut c_void,
    pub AllocationBase: *mut c_void,
    pub AllocationProtect: u32,
    pub PartitionId: u16,
    pub RegionSize: usize,
    pub State: u32,
    pub Protect: u32,
    pub Type: u32,
}

#[cfg(target_os = "windows")]
const PROCESS_VM_READ: u32 = 0x0010;
#[cfg(target_os = "windows")]
const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
#[cfg(target_os = "windows")]
const MEM_COMMIT: u32 = 0x1000;
#[cfg(target_os = "windows")]
const PAGE_READWRITE: u32 = 0x04;
#[cfg(target_os = "windows")]
const PAGE_EXECUTE_READWRITE: u32 = 0x40;

#[cfg(target_os = "windows")]
impl PageTracker {
    pub fn new(pid: u32) -> Result<Self, RevError> {
        let mut tracker = Self {
            _pid: pid,
            whitelist: Vec::new(),
            page_cache: HashMap::new(),
        };
        tracker.populate_whitelist()?;
        tracker.populate_initial_cache()?;
        Ok(tracker)
    }

    fn populate_whitelist(&mut self) -> Result<(), RevError> {
        let h_process = unsafe {
            OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, self._pid)
        };
        if h_process.is_null() {
            return Err(RevError::Io(std::io::Error::last_os_error()));
        }

        let mut addr = std::ptr::null();
        let mut mbi = unsafe { std::mem::zeroed::<MEMORY_BASIC_INFORMATION>() };

        while unsafe {
            VirtualQueryEx(
                h_process,
                addr,
                &mut mbi,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        } > 0 {
            let is_commit = mbi.State == MEM_COMMIT;
            let is_writable = (mbi.Protect & (PAGE_READWRITE | PAGE_EXECUTE_READWRITE)) != 0;
            let is_guard = (mbi.Protect & 0x100) != 0; // PAGE_GUARD is 0x100
            
            if is_commit && is_writable && !is_guard {
                self.whitelist.push(MemoryRegion {
                    start: mbi.BaseAddress as u64,
                    end: (mbi.BaseAddress as u64) + mbi.RegionSize as u64,
                    path: None,
                });
            }

            let next_addr = (mbi.BaseAddress as usize).saturating_add(mbi.RegionSize);
            if next_addr <= (mbi.BaseAddress as usize) {
                break;
            }
            addr = next_addr as *const c_void;
        }

        unsafe { CloseHandle(h_process); }
        Ok(())
    }

    fn populate_initial_cache(&mut self) -> Result<(), RevError> {
        let h_process = unsafe {
            OpenProcess(PROCESS_VM_READ, 0, self._pid)
        };
        if h_process.is_null() {
            return Err(RevError::Io(std::io::Error::last_os_error()));
        }

        for region in &self.whitelist {
            let mut addr = region.start;
            while addr < region.end {
                let mut page_buf = vec![0u8; 4096];
                let mut bytes_read = 0;
                let res = unsafe {
                    ReadProcessMemory(
                        h_process,
                        addr as *const c_void,
                        page_buf.as_mut_ptr() as *mut c_void,
                        4096,
                        &mut bytes_read,
                    )
                };
                if res != 0 && bytes_read == 4096 {
                    self.page_cache.insert(addr, page_buf);
                }
                addr += 4096;
            }
        }

        unsafe { CloseHandle(h_process); }
        Ok(())
    }

    pub fn get_dirty_pages(&mut self, force_full: bool) -> Result<Vec<PageDiff>, RevError> {
        let h_process = unsafe {
            OpenProcess(PROCESS_VM_READ, 0, self._pid)
        };
        if h_process.is_null() {
            return Err(RevError::Io(std::io::Error::last_os_error()));
        }

        let mut diffs = Vec::new();

        for region in &self.whitelist {
            let mut addr = region.start;
            while addr < region.end {
                let mut page_buf = vec![0u8; 4096];
                let mut bytes_read = 0;
                let res = unsafe {
                    ReadProcessMemory(
                        h_process,
                        addr as *const c_void,
                        page_buf.as_mut_ptr() as *mut c_void,
                        4096,
                        &mut bytes_read,
                    )
                };
                if res != 0 && bytes_read == 4096 {
                    let before = self
                        .page_cache
                        .get(&addr)
                        .cloned()
                        .unwrap_or_else(|| vec![0; 4096]);

                    if force_full || before != page_buf {
                        diffs.push(PageDiff {
                            address: addr,
                            before: before.clone(),
                            after: page_buf.clone(),
                        });
                        self.page_cache.insert(addr, page_buf);
                    }
                }
                addr += 4096;
            }
        }

        unsafe { CloseHandle(h_process); }
        Ok(diffs)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
impl PageTracker {
    pub fn new(_pid: u32) -> Result<Self, RevError> {
        Err(RevError::UnsupportedPlatform(
            "PageTracker is only supported on Linux and Windows".to_string(),
        ))
    }

    pub fn get_dirty_pages(&mut self, _force_full: bool) -> Result<Vec<PageDiff>, RevError> {
        Err(RevError::UnsupportedPlatform(
            "PageTracker is only supported on Linux and Windows".to_string(),
        ))
    }
}
