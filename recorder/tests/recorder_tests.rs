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
