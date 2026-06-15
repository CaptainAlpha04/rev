use rev_core::error::RevError;

#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub start: u64,
    pub end: u64,
    pub path: Option<String>,
}

#[cfg(target_os = "linux")]
pub fn parse_maps(pid: u32) -> Result<Vec<MemoryRegion>, RevError> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let maps_path = format!("/proc/{}/maps", pid);
    let file = File::open(&maps_path)?;
    let reader = BufReader::new(file);
    let mut regions = Vec::new();

    for line_result in reader.lines() {
        let line = line_result?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }

        // Parse address range (start-end)
        let addr_part = parts[0];
        let mut addrs = addr_part.split('-');
        let start_str = addrs.next().ok_or_else(|| RevError::TraceCorrupted {
            offset: 0,
            reason: "invalid address in maps".to_string(),
        })?;
        let end_str = addrs.next().ok_or_else(|| RevError::TraceCorrupted {
            offset: 0,
            reason: "invalid address in maps".to_string(),
        })?;

        let start = u64::from_str_radix(start_str, 16)
            .map_err(|e| RevError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
        let end = u64::from_str_radix(end_str, 16)
            .map_err(|e| RevError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

        // Parse permissions: only track private read-write segments
        let perms = parts[1];
        if perms != "rw-p" {
            continue;
        }

        // Get path if it exists
        let path = if parts.len() >= 6 {
            let p = parts[5..].join(" ");
            Some(p)
        } else {
            None
        };

        // Whitelist user-heap and anonymous allocations
        let is_whitelisted = match &path {
            None => true,
            Some(p) => p == "[heap]",
        };

        if is_whitelisted {
            regions.push(MemoryRegion { start, end, path });
        }
    }

    Ok(regions)
}

#[cfg(not(target_os = "linux"))]
pub fn parse_maps(_pid: u32) -> Result<Vec<MemoryRegion>, RevError> {
    // Return a mock whitelisted memory layout for development on Windows
    Ok(vec![
        MemoryRegion {
            start: 0x1000,
            end: 0x5000,
            path: Some("[heap]".to_string()),
        },
        MemoryRegion {
            start: 0x8000,
            end: 0xa000,
            path: None, // anonymous allocation
        },
    ])
}
