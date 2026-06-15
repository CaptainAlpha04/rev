use rev_core::error::RevError;

/// Compress input bytes using LZ4 block compression
pub fn compress(bytes: &[u8]) -> Vec<u8> {
    lz4_flex::block::compress(bytes)
}

/// Decompress LZ4 compressed bytes given the expected original length
pub fn decompress(bytes: &[u8], original_len: usize) -> Result<Vec<u8>, RevError> {
    lz4_flex::block::decompress(bytes, original_len).map_err(|e| RevError::TraceCorrupted {
        offset: 0,
        reason: format!("LZ4 decompression failed: {}", e),
    })
}
