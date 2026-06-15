use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A type alias for a Blake3 hash of a delta node
pub type DeltaHash = [u8; 32];

/// The single event type returned by every interceptor implementation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyscallEvent {
    pub id: u64,               // Monotonically increasing sequence number
    pub timestamp_ns: u64,     // Nanoseconds since process start
    pub syscall: SyscallKind,  // Enum of captured syscall categories
    pub return_bytes: Vec<u8>, // The exact bytes returned to the process
    pub fd: Option<i32>,       // File descriptor if applicable
}

/// Categories of non-deterministic syscalls captured by rev
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SyscallKind {
    TimeRead,
    RandomRead,
    NetworkRead { socket_addr: Option<String> },
    FileRead { path: Option<String> },
    EnvRead { key: String },
    ProcessId,
}

/// The state of the program at a specific execution step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramState {
    pub step: u64,
    pub timestamp_ns: u64,
    pub variables: Vec<Variable>, // Local variables in scope at this step
    pub call_stack: Vec<StackFrame>, // Call stack
    pub last_event: SyscallEvent, // What triggered this step boundary
}

/// A variable in scope during program execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Variable {
    pub name: String,
    pub type_name: String,
    pub value: serde_json::Value, // JSON representation of value
    pub is_changed: bool,         // Did this variable change at this step?
}

/// A single frame on the call stack
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StackFrame {
    pub function: String,
    pub file: String,
    pub line: u32,
}

/// A differential update of memory pages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageDiff {
    pub address: u64,    // Page-aligned memory address
    pub before: Vec<u8>, // Page content before this step (for reverse replay)
    pub after: Vec<u8>,  // Page content after this step
}

/// Representing a page in memory
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryPage {
    pub address: u64,
    pub content: Vec<u8>,
}

/// Full memory state at a given step
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryState {
    pub step_id: u64,
    pub pages: Vec<MemoryPage>,
}

/// Configuration for the trace recorder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecorderConfig {
    pub trace_path: PathBuf,
    pub chunk_size: usize,
    pub runtime_name: String,
    pub target_pid: u32,
    pub start_ts: u64,
}

/// Statistics for a recorded trace file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStats {
    pub total_events: u64,
    pub bytes_written: u64,
    pub compression_ratio: f32,
    pub duration_ms: u64,
}
