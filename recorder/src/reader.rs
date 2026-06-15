use crate::compressor::decompress;
use crate::writer::crc32;
use rev_core::constants::{SCHEMA_VERSION, TRACE_MAGIC};
use rev_core::error::RevError;
use rev_core::types::SyscallEvent;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

pub struct TraceHeader {
    pub version: u16,
    pub runtime_name: String,
    pub pid: u32,
    pub start_ts: u64,
}

pub struct TraceReader {
    reader: BufReader<File>,
    pub header: TraceHeader,
    current_offset: u64,
}

impl TraceReader {
    pub fn new(trace_path: &Path) -> Result<Self, RevError> {
        let file = File::open(trace_path)?;
        let mut reader = BufReader::new(file);

        let mut header_buf = [0u8; 64];
        reader.read_exact(&mut header_buf)?;

        // Verify magic number
        if &header_buf[0..8] != TRACE_MAGIC {
            return Err(RevError::TraceCorrupted {
                offset: 0,
                reason: "Invalid magic number".to_string(),
            });
        }

        // Verify version
        let version = u16::from_le_bytes([header_buf[8], header_buf[9]]);
        if version > SCHEMA_VERSION {
            return Err(RevError::SchemaMismatch {
                found: version,
                supported: SCHEMA_VERSION,
            });
        }

        // Runtime name (16 bytes, null-padded)
        let runtime_end = header_buf[10..26]
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(16);
        let runtime_name = String::from_utf8_lossy(&header_buf[10..10 + runtime_end]).into_owned();

        // Target PID
        let pid = u32::from_le_bytes([
            header_buf[26],
            header_buf[27],
            header_buf[28],
            header_buf[29],
        ]);

        // Start timestamp
        let mut start_ts_bytes = [0u8; 8];
        start_ts_bytes.copy_from_slice(&header_buf[30..38]);
        let start_ts = u64::from_le_bytes(start_ts_bytes);

        Ok(Self {
            reader,
            header: TraceHeader {
                version,
                runtime_name,
                pid,
                start_ts,
            },
            current_offset: 64,
        })
    }

    /// Read all events from the trace file
    pub fn read_all_events(&mut self) -> Result<Vec<SyscallEvent>, RevError> {
        let mut events = Vec::new();
        self.reader.seek(SeekFrom::Start(64))?;
        self.current_offset = 64;

        loop {
            let mut chunk_header = [0u8; 10];
            let bytes_read = self.reader.read(&mut chunk_header)?;
            if bytes_read == 0 {
                break; // EOF
            }
            if bytes_read < 10 {
                return Err(RevError::TraceCorrupted {
                    offset: self.current_offset,
                    reason: "Incomplete chunk header".to_string(),
                });
            }

            let chunk_len = u32::from_le_bytes([
                chunk_header[0],
                chunk_header[1],
                chunk_header[2],
                chunk_header[3],
            ]) as usize;

            let _event_count = u16::from_le_bytes([chunk_header[4], chunk_header[5]]) as usize;

            let checksum = u32::from_le_bytes([
                chunk_header[6],
                chunk_header[7],
                chunk_header[8],
                chunk_header[9],
            ]);

            self.current_offset += 10;

            let mut compressed_data = vec![0u8; chunk_len];
            self.reader.read_exact(&mut compressed_data)?;

            // Verify checksum
            let computed_checksum = crc32(&compressed_data);
            if computed_checksum != checksum {
                return Err(RevError::TraceCorrupted {
                    offset: self.current_offset,
                    reason: format!(
                        "CRC32 mismatch. Expected {:08x}, computed {:08x}",
                        checksum, computed_checksum
                    ),
                });
            }

            self.current_offset += chunk_len as u64;

            // Decompress
            let decompressed_data = decompress(&compressed_data, 0)?;

            // Deserialize sequence of SyscallEvents
            let chunk_events: Vec<SyscallEvent> = bincode::deserialize(&decompressed_data)
                .map_err(|e| {
                    RevError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                })?;

            events.extend(chunk_events);
        }

        Ok(events)
    }
}
