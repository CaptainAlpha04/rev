use rev_core::types::{RecorderConfig, SyscallEvent, SyscallKind};
use rev_recorder::compressor::{compress, decompress};
use rev_recorder::writer::crc32;
use rev_recorder::Recorder;
use std::fs::File;
use std::io::Read;

#[test]
fn test_compressor_roundtrip() {
    let data = b"Hello, time-traveling world! This is a test of the LZ4 compression wrapper.";
    let compressed = compress(data);
    assert!(!compressed.is_empty());

    let decompressed = decompress(&compressed, data.len()).unwrap();
    assert_eq!(data.to_vec(), decompressed);
}

#[test]
fn test_crc32_matches_expectations() {
    let data = b"123456789";
    // Standard CRC-32 IEEE value for "123456789" is 0xCBF43926
    assert_eq!(crc32(data), 0xCBF43926);
}

#[test]
fn test_recorder_writes_valid_trace() {
    let temp_dir = std::env::temp_dir();
    let trace_path = temp_dir.join(format!("test_trace_{}.rev-trace", std::process::id()));

    // Clean up old test file if exists
    if trace_path.exists() {
        let _ = std::fs::remove_file(&trace_path);
    }

    let config = RecorderConfig {
        trace_path: trace_path.clone(),
        chunk_size: 2, // Flush every 2 events
        runtime_name: "test_runtime".to_string(),
        target_pid: 9999,
        start_ts: 123456789,
    };

    let mut recorder = Recorder::new(&trace_path, &config).unwrap();

    // Write 3 events (1 chunk flush + 1 remaining event)
    for i in 0..3 {
        let event = SyscallEvent {
            id: i,
            timestamp_ns: 1000 + i * 100,
            syscall: SyscallKind::TimeRead,
            return_bytes: vec![1, 2, 3],
            fd: None,
        };
        recorder.record(event).unwrap();
    }

    let stats = recorder.finalize().unwrap();
    assert_eq!(stats.total_events, 3);
    assert!(stats.bytes_written > 64); // Header + at least one chunk + remaining flush

    // Read the file and verify the magic header
    let mut file = File::open(&trace_path).unwrap();
    let mut header = [0u8; 64];
    file.read_exact(&mut header).unwrap();

    assert_eq!(&header[0..8], b"REVTRACE");
    assert_eq!(u16::from_le_bytes([header[8], header[9]]), 1); // schema version
    assert_eq!(&header[10..22], b"test_runtime"); // runtime name
    assert_eq!(
        u32::from_le_bytes([header[26], header[27], header[28], header[29]]),
        9999
    );

    // Clean up
    let _ = std::fs::remove_file(&trace_path);
}

#[test]
fn test_trace_reader_roundtrip() {
    let temp_dir = std::env::temp_dir();
    let trace_path = temp_dir.join(format!(
        "test_reader_roundtrip_{}.rev-trace",
        std::process::id()
    ));

    if trace_path.exists() {
        let _ = std::fs::remove_file(&trace_path);
    }

    let config = RecorderConfig {
        trace_path: trace_path.clone(),
        chunk_size: 3,
        runtime_name: "python3".to_string(),
        target_pid: 4567,
        start_ts: 987654321,
    };

    let mut recorder = Recorder::new(&trace_path, &config).unwrap();

    let mut written_events = Vec::new();
    for i in 0..5 {
        let event = SyscallEvent {
            id: i,
            timestamp_ns: 2000 + i * 200,
            syscall: SyscallKind::RandomRead,
            return_bytes: vec![i as u8, (i + 1) as u8],
            fd: None,
        };
        recorder.record(event.clone()).unwrap();
        written_events.push(event);
    }

    recorder.finalize().unwrap();

    // Now read back with TraceReader
    let mut reader = rev_recorder::TraceReader::new(&trace_path).unwrap();
    assert_eq!(reader.header.version, rev_core::constants::SCHEMA_VERSION);
    assert_eq!(reader.header.runtime_name, "python3");
    assert_eq!(reader.header.pid, 4567);
    assert_eq!(reader.header.start_ts, 987654321);

    let read_events = reader.read_all_events().unwrap();
    assert_eq!(read_events.len(), 5);
    assert_eq!(read_events, written_events);

    let _ = std::fs::remove_file(&trace_path);
}
