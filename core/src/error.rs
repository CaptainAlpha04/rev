/// Unified error types for the rev project
#[derive(Debug, thiserror::Error)]
pub enum RevError {
    #[error("No command provided. Usage: rev <runtime> [args...]")]
    NoCommand,

    #[error("Unsupported runtime: '{0}'. Supported: python, node, ruby")]
    UnsupportedRuntime(String),

    #[error("Failed to attach to process {pid}: {reason}")]
    AttachFailed { pid: u32, reason: String },

    #[error("Trace file corrupted at offset {offset}: {reason}")]
    TraceCorrupted { offset: u64, reason: String },

    #[error("Trace schema version {found} is newer than supported {supported}")]
    SchemaMismatch { found: u16, supported: u16 },

    #[error("Replay failed at step {step}: {reason}")]
    ReplayFailed { step: u64, reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
}
