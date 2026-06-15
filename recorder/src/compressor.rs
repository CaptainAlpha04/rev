use rev_core::error::RevError;

/// Compress input bytes using LZ4 block compression with prepended size
pub fn compress(bytes: &[u8]) -> Vec<u8> {
    lz4_flex::block::compress_prepend_size(bytes)
}

/// Decompress LZ4 compressed bytes with prepended size (original_len parameter is ignored)
pub fn decompress(bytes: &[u8], _original_len: usize) -> Result<Vec<u8>, RevError> {
    lz4_flex::block::decompress_size_prepended(bytes).map_err(|e| RevError::TraceCorrupted {
        offset: 0,
        reason: format!("LZ4 decompression failed: {}", e),
    })
}
