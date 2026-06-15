use rev_core::error::RevError;
use rev_core::types::{RecorderConfig, SyscallEvent, TraceStats};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;

pub struct Recorder {
    writer: BufWriter<File>,
    chunk_buffer: Vec<SyscallEvent>,
    chunk_size: usize,
    bytes_written: u64,
    total_events: u64,
    uncompressed_bytes: u64,
    start_time: Instant,
}

impl Recorder {
    pub fn new(trace_path: &Path, config: &RecorderConfig) -> Result<Self, RevError> {
        // Ensure parent directories exist
        if let Some(parent) = trace_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = File::create(trace_path)?;
        let mut writer = BufWriter::new(file);

        // Write the header immediately
        let bytes_written = write_header(
            &mut writer,
            &config.runtime_name,
            config.target_pid,
            config.start_ts,
        )?;

        Ok(Self {
            writer,
            chunk_buffer: Vec::with_capacity(config.chunk_size),
            chunk_size: config.chunk_size,
            bytes_written,
            total_events: 0,
            uncompressed_bytes: 0,
            start_time: Instant::now(),
        })
    }

    /// Buffer an event. Flushes chunk to disk when chunk_size is reached.
    pub fn record(&mut self, event: SyscallEvent) -> Result<(), RevError> {
        self.chunk_buffer.push(event);
        if self.chunk_buffer.len() >= self.chunk_size {
            self.flush_chunk()?;
        }
        Ok(())
    }

    /// Force-flush remaining buffered events and write EOF marker.
    pub fn finalize(&mut self) -> Result<TraceStats, RevError> {
        self.flush_chunk()?;
        self.writer.flush()?;

        let duration_ms = self.start_time.elapsed().as_millis() as u64;

        // Compression ratio = uncompressed bytes / compressed bytes written to trace blocks
        let compression_ratio = if self.bytes_written > 64 {
            let trace_data_written = (self.bytes_written - 64) as f32;
            if trace_data_written > 0.0 {
                self.uncompressed_bytes as f32 / trace_data_written
            } else {
                1.0
            }
        } else {
            1.0
        };

        Ok(TraceStats {
            total_events: self.total_events,
            bytes_written: self.bytes_written,
            compression_ratio,
            duration_ms,
        })
    }

    /// Flush the current chunk buffer to disk
    fn flush_chunk(&mut self) -> Result<(), RevError> {
        if self.chunk_buffer.is_empty() {
            return Ok(());
        }

        let event_count = self.chunk_buffer.len() as u16;
        self.total_events += event_count as u64;

        // Serialize sequence of SyscallEvents
        let serialized_data = bincode::serialize(&self.chunk_buffer)
            .map_err(|e| RevError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

        self.uncompressed_bytes += serialized_data.len() as u64;

        // Compress
        let compressed_data = crate::compressor::compress(&serialized_data);

        // Checksum
        let checksum = crc32(&compressed_data);

        // Chunk layout:
        //   chunk_len:   4 bytes (u32)
        //   event_count: 2 bytes (u16)
        //   checksum:    4 bytes (u32)
        //   data:        N bytes
        let chunk_len = compressed_data.len() as u32;

        self.writer.write_all(&chunk_len.to_le_bytes())?;
        self.writer.write_all(&event_count.to_le_bytes())?;
        self.writer.write_all(&checksum.to_le_bytes())?;
        self.writer.write_all(&compressed_data)?;

        self.bytes_written += 10 + chunk_len as u64;
        self.chunk_buffer.clear();

        Ok(())
    }
}

/// Helper function to write the 64-byte trace file header
fn write_header(
    writer: &mut BufWriter<File>,
    runtime_name: &str,
    pid: u32,
    start_ts: u64,
) -> Result<u64, RevError> {
    let mut header = [0u8; 64];

    // Magic number (8 bytes)
    header[0..8].copy_from_slice(rev_core::constants::TRACE_MAGIC);

    // Schema version (2 bytes)
    let version_bytes = rev_core::constants::SCHEMA_VERSION.to_le_bytes();
    header[8..10].copy_from_slice(&version_bytes);

    // Runtime name (16 bytes, null-padded)
    let runtime_bytes = runtime_name.as_bytes();
    let runtime_len = runtime_bytes.len().min(16);
    header[10..10 + runtime_len].copy_from_slice(&runtime_bytes[..runtime_len]);

    // Target PID (4 bytes)
    let pid_bytes = pid.to_le_bytes();
    header[26..30].copy_from_slice(&pid_bytes);

    // Start Unix timestamp in ns (8 bytes)
    let start_ts_bytes = start_ts.to_le_bytes();
    header[30..38].copy_from_slice(&start_ts_bytes);

    // Reserved (26 bytes) remain 0

    writer.write_all(&header)?;
    Ok(64)
}

/// Standard IEEE CRC32 checksum helper (polynomial 0xedb88320)
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffffffffu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            if c & 1 != 0 {
                c = (c >> 1) ^ 0xedb88320u32;
            } else {
                c >>= 1;
            }
        }
    }
    !c
}
