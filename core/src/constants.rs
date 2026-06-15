/// Magic bytes for the trace file header
pub const TRACE_MAGIC: &[u8; 8] = b"REVTRACE";

/// Schema version of the trace file format
pub const SCHEMA_VERSION: u16 = 1;

/// Default number of events per compressed chunk in a trace file
pub const DEFAULT_CHUNK_SIZE: usize = 256;

/// Number of steps between full memory snapshots
pub const SNAPSHOT_INTERVAL: u64 = 100;

/// Memory page size in bytes on target architecture (x86_64)
pub const PAGE_SIZE: usize = 4096;

/// Character marker used in the TUI to indicate a changed variable
pub const CHANGED_MARKER: char = '●';
