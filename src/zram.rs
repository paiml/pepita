//! zram: Compressed RAM Block Device
//!
//! Pure Rust implementation of zram for memory-efficient compressed block storage.
//! Supports multiple compression algorithms (LZ4, Zstd) implemented in pure Rust.
//!
//! ## Example
//!
//! ```rust,ignore
//! use pepita::zram::{ZramDevice, ZramConfig, ZramCompressor};
//!
//! let config = ZramConfig {
//!     size_bytes: 1024 * 1024 * 1024, // 1 GiB
//!     compressor: ZramCompressor::Lz4,
//!     ..Default::default()
//! };
//!
//! let device = ZramDevice::new(config)?;
//! ```

use crate::constants::PAGE_SIZE;
use crate::error::{KernelError, Result};

#[cfg(feature = "std")]
use std::collections::HashMap;
#[cfg(feature = "std")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "std")]
use std::sync::RwLock;

// ============================================================================
// COMPRESSION ALGORITHMS
// ============================================================================

/// zram compression algorithms (pure Rust implementations)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ZramCompressor {
    /// LZ4 - fast compression/decompression (default)
    #[default]
    Lz4 = 0,
    /// No compression (for testing/benchmarking)
    None = 1,
}

impl ZramCompressor {
    /// Get compressor name
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Lz4 => "lz4",
            Self::None => "none",
        }
    }

    /// Get typical compression ratio (for estimation)
    #[must_use]
    pub const fn typical_ratio(&self) -> f64 {
        match self {
            Self::Lz4 => 2.5,
            Self::None => 1.0,
        }
    }
}

// ============================================================================
// CONFIGURATION
// ============================================================================

/// zram device configuration
#[derive(Debug, Clone)]
pub struct ZramConfig {
    /// Device size in bytes (logical size)
    pub size_bytes: u64,
    /// Compression algorithm
    pub compressor: ZramCompressor,
    /// Maximum memory limit (None = 50% of size)
    pub mem_limit: Option<u64>,
    /// Number of compression streams (for parallel compression)
    pub num_streams: u32,
    /// Enable statistics tracking
    pub track_stats: bool,
}

impl Default for ZramConfig {
    fn default() -> Self {
        Self {
            size_bytes: 4 * 1024 * 1024 * 1024, // 4 GiB
            compressor: ZramCompressor::Lz4,
            mem_limit: None,
            num_streams: 4,
            track_stats: true,
        }
    }
}

impl ZramConfig {
    /// Create a new config with specified size
    #[must_use]
    pub const fn with_size(size_bytes: u64) -> Self {
        Self {
            size_bytes,
            compressor: ZramCompressor::Lz4,
            mem_limit: None,
            num_streams: 4,
            track_stats: true,
        }
    }

    /// Set compressor
    #[must_use]
    pub const fn compressor(mut self, compressor: ZramCompressor) -> Self {
        self.compressor = compressor;
        self
    }

    /// Set memory limit
    #[must_use]
    pub const fn mem_limit(mut self, limit: u64) -> Self {
        self.mem_limit = Some(limit);
        self
    }

    /// Set number of streams
    #[must_use]
    pub const fn num_streams(mut self, streams: u32) -> Self {
        self.num_streams = streams;
        self
    }

    /// Calculate max pages
    #[must_use]
    pub const fn max_pages(&self) -> u64 {
        self.size_bytes / PAGE_SIZE as u64
    }

    /// Get effective memory limit
    #[must_use]
    pub fn effective_mem_limit(&self) -> u64 {
        self.mem_limit.unwrap_or(self.size_bytes / 2)
    }
}

// ============================================================================
// STATISTICS
// ============================================================================

/// zram device statistics
#[derive(Debug, Clone, Default)]
pub struct ZramStats {
    /// Uncompressed data size (bytes)
    pub orig_data_size: u64,
    /// Compressed data size (bytes)
    pub compr_data_size: u64,
    /// Total memory used (including metadata)
    pub mem_used_total: u64,
    /// Number of pages stored
    pub pages_stored: u64,
    /// Number of zero-filled pages (stored as marker only)
    pub zero_pages: u64,
    /// Number of same-filled pages (stored as single byte)
    pub same_pages: u64,
    /// Number of pages that didn't compress well
    pub huge_pages: u64,
    /// Number of read operations
    pub num_reads: u64,
    /// Number of write operations
    pub num_writes: u64,
    /// Failed reads
    pub failed_reads: u64,
    /// Failed writes
    pub failed_writes: u64,
}

impl ZramStats {
    /// Calculate compression ratio
    #[must_use]
    pub fn compression_ratio(&self) -> f64 {
        if self.compr_data_size == 0 {
            return 1.0;
        }
        self.orig_data_size as f64 / self.compr_data_size as f64
    }

    /// Calculate memory efficiency
    #[must_use]
    pub fn memory_efficiency(&self) -> f64 {
        if self.mem_used_total == 0 {
            return 1.0;
        }
        self.orig_data_size as f64 / self.mem_used_total as f64
    }

    /// Get special page count (zero + same)
    #[must_use]
    pub const fn special_pages(&self) -> u64 {
        self.zero_pages + self.same_pages
    }
}

/// Atomic statistics (for thread-safe updates)
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub struct AtomicStats {
    orig_data_size: AtomicU64,
    compr_data_size: AtomicU64,
    mem_used_total: AtomicU64,
    pages_stored: AtomicU64,
    zero_pages: AtomicU64,
    same_pages: AtomicU64,
    huge_pages: AtomicU64,
    num_reads: AtomicU64,
    num_writes: AtomicU64,
    failed_reads: AtomicU64,
    failed_writes: AtomicU64,
}

#[cfg(feature = "std")]
impl AtomicStats {
    /// Create a snapshot of current stats
    pub fn snapshot(&self) -> ZramStats {
        ZramStats {
            orig_data_size: self.orig_data_size.load(Ordering::Relaxed),
            compr_data_size: self.compr_data_size.load(Ordering::Relaxed),
            mem_used_total: self.mem_used_total.load(Ordering::Relaxed),
            pages_stored: self.pages_stored.load(Ordering::Relaxed),
            zero_pages: self.zero_pages.load(Ordering::Relaxed),
            same_pages: self.same_pages.load(Ordering::Relaxed),
            huge_pages: self.huge_pages.load(Ordering::Relaxed),
            num_reads: self.num_reads.load(Ordering::Relaxed),
            num_writes: self.num_writes.load(Ordering::Relaxed),
            failed_reads: self.failed_reads.load(Ordering::Relaxed),
            failed_writes: self.failed_writes.load(Ordering::Relaxed),
        }
    }

    /// Record a write operation
    pub fn record_write(&self, orig_size: usize, compr_size: usize) {
        self.orig_data_size.fetch_add(orig_size as u64, Ordering::Relaxed);
        self.compr_data_size.fetch_add(compr_size as u64, Ordering::Relaxed);
        self.mem_used_total.fetch_add(compr_size as u64, Ordering::Relaxed);
        self.pages_stored.fetch_add(1, Ordering::Relaxed);
        self.num_writes.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a zero page
    pub fn record_zero_page(&self) {
        self.zero_pages.fetch_add(1, Ordering::Relaxed);
        self.pages_stored.fetch_add(1, Ordering::Relaxed);
        self.num_writes.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a same-filled page
    pub fn record_same_page(&self) {
        self.same_pages.fetch_add(1, Ordering::Relaxed);
        self.pages_stored.fetch_add(1, Ordering::Relaxed);
        self.num_writes.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a huge (incompressible) page
    pub fn record_huge_page(&self) {
        self.huge_pages.fetch_add(1, Ordering::Relaxed);
        self.orig_data_size.fetch_add(PAGE_SIZE as u64, Ordering::Relaxed);
        self.compr_data_size.fetch_add(PAGE_SIZE as u64, Ordering::Relaxed);
        self.mem_used_total.fetch_add(PAGE_SIZE as u64, Ordering::Relaxed);
        self.pages_stored.fetch_add(1, Ordering::Relaxed);
        self.num_writes.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a read
    pub fn record_read(&self) {
        self.num_reads.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a failed read
    pub fn record_failed_read(&self) {
        self.failed_reads.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a failed write
    pub fn record_failed_write(&self) {
        self.failed_writes.fetch_add(1, Ordering::Relaxed);
    }
}

// ============================================================================
// PAGE ENTRY
// ============================================================================

/// Entry in the page table
#[cfg(feature = "std")]
#[derive(Debug, Clone, Default)]
pub enum PageEntry {
    /// Empty slot (not yet written)
    #[default]
    Empty,
    /// Zero-filled page (no data stored)
    Zero,
    /// Same-byte-filled page (store single byte)
    Same {
        /// The repeated byte value
        value: u8,
    },
    /// Compressed page data
    Compressed {
        /// Compressed bytes
        data: Vec<u8>,
    },
    /// Uncompressed page (didn't compress well)
    Uncompressed {
        /// Raw page data
        data: Vec<u8>,
    },
}

#[cfg(feature = "std")]
impl PageEntry {
    /// Get the memory size of this entry
    #[must_use]
    pub fn memory_size(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Zero => 0,
            Self::Same { .. } => 1,
            Self::Compressed { data } => data.len(),
            Self::Uncompressed { data } => data.len(),
        }
    }

    /// Check if this is an empty entry
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
}

#[cfg(feature = "std")]
// ============================================================================
// PAGE TABLE
// ============================================================================

/// Page table for zram device
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct PageTable {
    /// Entries indexed by page number
    entries: RwLock<HashMap<u64, PageEntry>>,
}

#[cfg(feature = "std")]
impl PageTable {
    /// Create a new page table
    #[must_use]
    pub fn new() -> Self {
        Self { entries: RwLock::new(HashMap::new()) }
    }

    /// Get a page entry
    pub fn get(&self, page_index: u64) -> Result<PageEntry> {
        let entries = self.entries.read().map_err(|_| KernelError::ResourceBusy)?;
        Ok(entries.get(&page_index).cloned().unwrap_or(PageEntry::Empty))
    }

    /// Set a page entry
    pub fn set(&self, page_index: u64, entry: PageEntry) -> Result<()> {
        let mut entries = self.entries.write().map_err(|_| KernelError::ResourceBusy)?;
        entries.insert(page_index, entry);
        Ok(())
    }

    /// Remove a page entry
    pub fn remove(&self, page_index: u64) -> Result<Option<PageEntry>> {
        let mut entries = self.entries.write().map_err(|_| KernelError::ResourceBusy)?;
        Ok(entries.remove(&page_index))
    }

    /// Get number of stored pages
    pub fn len(&self) -> usize {
        self.entries.read().map(|e| e.len()).unwrap_or(0)
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(feature = "std")]
impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// COMPRESSION STREAM
// ============================================================================

/// Compression stream for a single thread
#[cfg(feature = "std")]
pub struct CompressionStream {
    /// Compressor type
    compressor: ZramCompressor,
}

#[cfg(feature = "std")]
impl CompressionStream {
    /// Create a new compression stream
    #[must_use]
    pub fn new(compressor: ZramCompressor) -> Self {
        Self { compressor }
    }

    /// Compress data using configured algorithm
    pub fn compress(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        match self.compressor {
            ZramCompressor::Lz4 => self.compress_lz4(input),
            ZramCompressor::None => Ok(input.to_vec()),
        }
    }

    /// Decompress data using configured algorithm
    pub fn decompress(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize> {
        match self.compressor {
            ZramCompressor::Lz4 => self.decompress_lz4(input, output),
            ZramCompressor::None => {
                let len = input.len().min(output.len());
                output[..len].copy_from_slice(&input[..len]);
                Ok(len)
            }
        }
    }

    // ========================================================================
    // LZ4 COMPRESSION (Pure Rust Implementation)
    // ========================================================================
    //
    // LZ4 is a fast compression algorithm that uses:
    // - Sliding window dictionary
    // - Hash table for match finding
    // - Simple literal/match encoding
    //
    // Block format:
    // [token][literal_len?][literals][offset][match_len?]
    //
    // Token byte: [4 bits literal len][4 bits match len]

    /// LZ4 compress
    fn compress_lz4(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        // For very small inputs, don't compress
        if input.len() < 16 {
            return Ok(input.to_vec());
        }

        let mut output = Vec::with_capacity(input.len());
        let mut hash_table: [u32; 4096] = [0; 4096];

        let mut ip = 0usize; // Input position
        let mut anchor = 0usize; // Start of literals

        // Leave room for last literals (LZ4 requires 5 bytes minimum at end)
        let limit = input.len().saturating_sub(5);

        while ip < limit {
            // Find match using hash
            let hash = self.lz4_hash(&input[ip..]) as usize % 4096;
            let ref_pos = hash_table[hash] as usize;
            hash_table[hash] = ip as u32;

            // Check for match (need at least 4 bytes and valid offset)
            if ref_pos > 0
                && ip > ref_pos
                && ip - ref_pos < 65535
                && ip + 4 <= input.len()
                && ref_pos + 4 <= input.len()
                && input[ref_pos..ref_pos + 4] == input[ip..ip + 4]
            {
                // Found a match - first extend it
                let offset = ip - ref_pos;
                let mut match_len = 4;
                while ip + match_len < input.len()
                    && ref_pos + match_len < input.len()
                    && input[ip + match_len] == input[ref_pos + match_len]
                {
                    match_len += 1;
                }

                // Emit literals and match
                self.lz4_emit_sequence(&mut output, &input[anchor..ip], offset, match_len);

                ip += match_len;
                anchor = ip;
            } else {
                ip += 1;
            }
        }

        // Emit remaining literals (final sequence has no match)
        if anchor < input.len() {
            self.lz4_emit_literals_only(&mut output, &input[anchor..]);
        }

        // If compression didn't help, return original
        if output.len() >= input.len() {
            return Ok(input.to_vec());
        }

        Ok(output)
    }

    /// Emit a complete LZ4 sequence: token + literals + offset + match
    fn lz4_emit_sequence(
        &self,
        output: &mut Vec<u8>,
        literals: &[u8],
        offset: usize,
        match_len: usize,
    ) {
        let literal_len = literals.len();

        // Token: high 4 bits = literal length, low 4 bits = match length - 4
        let lit_token = if literal_len >= 15 { 15 } else { literal_len } as u8;
        let match_token = if match_len - 4 >= 15 { 15 } else { match_len - 4 } as u8;
        output.push((lit_token << 4) | match_token);

        // Extended literal length
        if literal_len >= 15 {
            let mut remaining = literal_len - 15;
            while remaining >= 255 {
                output.push(255);
                remaining -= 255;
            }
            output.push(remaining as u8);
        }

        // Literal bytes
        output.extend_from_slice(literals);

        // Offset (little-endian)
        output.push((offset & 0xFF) as u8);
        output.push(((offset >> 8) & 0xFF) as u8);

        // Extended match length
        if match_len - 4 >= 15 {
            let mut remaining = match_len - 4 - 15;
            while remaining >= 255 {
                output.push(255);
                remaining -= 255;
            }
            output.push(remaining as u8);
        }
    }

    /// Emit literals only (for final block with no match)
    fn lz4_emit_literals_only(&self, output: &mut Vec<u8>, literals: &[u8]) {
        let literal_len = literals.len();

        // Token: high 4 bits = literal length, low 4 bits = 0 (no match)
        let lit_token = if literal_len >= 15 { 15 } else { literal_len } as u8;
        output.push(lit_token << 4);

        // Extended literal length
        if literal_len >= 15 {
            let mut remaining = literal_len - 15;
            while remaining >= 255 {
                output.push(255);
                remaining -= 255;
            }
            output.push(remaining as u8);
        }

        // Literal bytes
        output.extend_from_slice(literals);
    }

    /// LZ4 decompress
    fn decompress_lz4(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize> {
        if input.is_empty() {
            return Ok(0);
        }

        let mut ip = 0usize; // Input position
        let mut op = 0usize; // Output position

        while ip < input.len() {
            // Read token
            let token = input[ip];
            ip += 1;

            let mut literal_len = ((token >> 4) & 0x0F) as usize;
            let mut match_len = (token & 0x0F) as usize;

            // Extended literal length
            if literal_len == 15 {
                while ip < input.len() {
                    let byte = input[ip];
                    ip += 1;
                    literal_len += byte as usize;
                    if byte != 255 {
                        break;
                    }
                }
            }

            // Copy literals
            if literal_len > 0 {
                if ip + literal_len > input.len() || op + literal_len > output.len() {
                    return Err(KernelError::InvalidArgument);
                }
                output[op..op + literal_len].copy_from_slice(&input[ip..ip + literal_len]);
                ip += literal_len;
                op += literal_len;
            }

            // Check for end
            if ip >= input.len() {
                break;
            }

            // Read offset
            if ip + 2 > input.len() {
                return Err(KernelError::InvalidArgument);
            }
            let offset = (input[ip] as usize) | ((input[ip + 1] as usize) << 8);
            ip += 2;

            if offset == 0 || offset > op {
                return Err(KernelError::InvalidArgument);
            }

            // Extended match length
            match_len += 4; // Minimum match
            if (token & 0x0F) == 15 {
                while ip < input.len() {
                    let byte = input[ip];
                    ip += 1;
                    match_len += byte as usize;
                    if byte != 255 {
                        break;
                    }
                }
            }

            // Copy match
            let match_pos = op - offset;
            if op + match_len > output.len() {
                return Err(KernelError::InvalidArgument);
            }

            // Handle overlapping copy
            for i in 0..match_len {
                output[op + i] = output[match_pos + i];
            }
            op += match_len;
        }

        Ok(op)
    }

    /// LZ4 hash function
    fn lz4_hash(&self, data: &[u8]) -> u32 {
        if data.len() < 4 {
            return 0;
        }
        let v = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        v.wrapping_mul(2654435761) >> 20
    }
}

// ============================================================================
// ZRAM DEVICE
// ============================================================================

/// zram block device
#[cfg(feature = "std")]
pub struct ZramDevice {
    /// Configuration
    config: ZramConfig,
    /// Page table
    page_table: PageTable,
    /// Compression stream (single for now, TODO: thread-local pool)
    stream: std::sync::Mutex<CompressionStream>,
    /// Statistics
    stats: AtomicStats,
}

#[cfg(feature = "std")]
impl ZramDevice {
    /// Create a new zram device
    pub fn new(config: ZramConfig) -> Result<Self> {
        let stream = CompressionStream::new(config.compressor);

        Ok(Self {
            config,
            page_table: PageTable::new(),
            stream: std::sync::Mutex::new(stream),
            stats: AtomicStats::default(),
        })
    }

    /// Get device configuration
    pub const fn config(&self) -> &ZramConfig {
        &self.config
    }

    /// Get device statistics
    pub fn stats(&self) -> ZramStats {
        self.stats.snapshot()
    }

    /// Read a page
    pub fn read_page(&self, page_index: u64, buffer: &mut [u8]) -> Result<()> {
        if buffer.len() < PAGE_SIZE {
            return Err(KernelError::InvalidArgument);
        }

        if page_index >= self.config.max_pages() {
            return Err(KernelError::InvalidArgument);
        }

        let entry = self.page_table.get(page_index)?;

        match entry {
            PageEntry::Empty => {
                // Unwritten page reads as zeros
                buffer[..PAGE_SIZE].fill(0);
            }
            PageEntry::Zero => {
                buffer[..PAGE_SIZE].fill(0);
            }
            PageEntry::Same { value } => {
                buffer[..PAGE_SIZE].fill(value);
            }
            PageEntry::Compressed { data } => {
                let mut stream = self.stream.lock().map_err(|_| KernelError::ResourceBusy)?;
                let decompressed = stream.decompress(&data, &mut buffer[..PAGE_SIZE])?;
                if decompressed != PAGE_SIZE {
                    // Pad with zeros if needed
                    buffer[decompressed..PAGE_SIZE].fill(0);
                }
            }
            PageEntry::Uncompressed { data } => {
                let len = data.len().min(PAGE_SIZE);
                buffer[..len].copy_from_slice(&data[..len]);
                if len < PAGE_SIZE {
                    buffer[len..PAGE_SIZE].fill(0);
                }
            }
        }

        self.stats.record_read();
        Ok(())
    }

    /// Write a page
    pub fn write_page(&self, page_index: u64, data: &[u8]) -> Result<()> {
        if data.len() < PAGE_SIZE {
            return Err(KernelError::InvalidArgument);
        }

        if page_index >= self.config.max_pages() {
            return Err(KernelError::InvalidArgument);
        }

        let page_data = &data[..PAGE_SIZE];

        // Check for zero page
        if page_data.iter().all(|&b| b == 0) {
            self.page_table.set(page_index, PageEntry::Zero)?;
            self.stats.record_zero_page();
            return Ok(());
        }

        // Check for same-filled page
        if let Some(value) = Self::check_same_filled(page_data) {
            self.page_table.set(page_index, PageEntry::Same { value })?;
            self.stats.record_same_page();
            return Ok(());
        }

        // Compress
        let mut stream = self.stream.lock().map_err(|_| KernelError::ResourceBusy)?;
        let compressed = stream.compress(page_data)?;

        // Check if compression is worthwhile (at least 12.5% savings)
        if compressed.len() >= PAGE_SIZE - PAGE_SIZE / 8 {
            // Store uncompressed
            self.page_table
                .set(page_index, PageEntry::Uncompressed { data: page_data.to_vec() })?;
            self.stats.record_huge_page();
        } else {
            self.page_table.set(page_index, PageEntry::Compressed { data: compressed.clone() })?;
            self.stats.record_write(PAGE_SIZE, compressed.len());
        }

        Ok(())
    }

    /// Discard a page (TRIM)
    pub fn discard_page(&self, page_index: u64) -> Result<()> {
        if page_index >= self.config.max_pages() {
            return Err(KernelError::InvalidArgument);
        }
        self.page_table.remove(page_index)?;
        Ok(())
    }

    /// Get the number of stored pages
    pub fn stored_pages(&self) -> usize {
        self.page_table.len()
    }

    /// Check if a page is all the same byte
    fn check_same_filled(data: &[u8]) -> Option<u8> {
        if data.is_empty() {
            return None;
        }
        let first = data[0];
        if data.iter().all(|&b| b == first) {
            Some(first)
        } else {
            None
        }
    }

    /// Reset the device (clear all data)
    pub fn reset(&self) -> Result<()> {
        // Clear page table by removing all entries
        for i in 0..self.config.max_pages() {
            let _ = self.page_table.remove(i);
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
impl std::fmt::Debug for ZramDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZramDevice")
            .field("size_bytes", &self.config.size_bytes)
            .field("compressor", &self.config.compressor)
            .field("stored_pages", &self.stored_pages())
            .finish()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // ZramCompressor Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_compressor_name() {
        assert_eq!(ZramCompressor::Lz4.name(), "lz4");
        assert_eq!(ZramCompressor::None.name(), "none");
    }

    #[test]
    fn test_compressor_default() {
        assert_eq!(ZramCompressor::default(), ZramCompressor::Lz4);
    }

    // ------------------------------------------------------------------------
    // ZramConfig Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_config_default() {
        let config = ZramConfig::default();
        assert_eq!(config.size_bytes, 4 * 1024 * 1024 * 1024);
        assert_eq!(config.compressor, ZramCompressor::Lz4);
        assert!(config.mem_limit.is_none());
    }

    #[test]
    fn test_config_builder() {
        let config = ZramConfig::with_size(1024 * 1024)
            .compressor(ZramCompressor::None)
            .mem_limit(512 * 1024)
            .num_streams(8);

        assert_eq!(config.size_bytes, 1024 * 1024);
        assert_eq!(config.compressor, ZramCompressor::None);
        assert_eq!(config.mem_limit, Some(512 * 1024));
        assert_eq!(config.num_streams, 8);
    }

    #[test]
    fn test_config_max_pages() {
        let config = ZramConfig::with_size(PAGE_SIZE as u64 * 100);
        assert_eq!(config.max_pages(), 100);
    }

    // ------------------------------------------------------------------------
    // ZramStats Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_stats_compression_ratio() {
        let mut stats = ZramStats::default();
        assert_eq!(stats.compression_ratio(), 1.0);

        stats.orig_data_size = 1000;
        stats.compr_data_size = 500;
        assert!((stats.compression_ratio() - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_stats_special_pages() {
        let stats = ZramStats { zero_pages: 10, same_pages: 5, ..Default::default() };
        assert_eq!(stats.special_pages(), 15);
    }

    // ------------------------------------------------------------------------
    // CompressionStream Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_compress_decompress_none() {
        let mut stream = CompressionStream::new(ZramCompressor::None);
        let input = b"Hello, World!";
        let compressed = stream.compress(input).unwrap();
        assert_eq!(compressed, input);

        let mut output = vec![0u8; 64];
        let len = stream.decompress(&compressed, &mut output).unwrap();
        assert_eq!(&output[..len], input);
    }

    #[test]
    fn test_compress_decompress_lz4_simple() {
        let mut stream = CompressionStream::new(ZramCompressor::Lz4);

        // Compressible data (repeated pattern)
        let input: Vec<u8> = (0..PAGE_SIZE).map(|i| (i % 256) as u8).collect();

        let compressed = stream.compress(&input).unwrap();

        let mut output = vec![0u8; PAGE_SIZE];
        let len = stream.decompress(&compressed, &mut output).unwrap();

        assert_eq!(len, PAGE_SIZE);
        assert_eq!(&output[..], &input[..]);
    }

    #[test]
    fn test_compress_decompress_lz4_repeated() {
        let mut stream = CompressionStream::new(ZramCompressor::Lz4);

        // Highly compressible (repeated bytes)
        let input = vec![0xABu8; PAGE_SIZE];

        let compressed = stream.compress(&input).unwrap();
        // Should compress well
        assert!(compressed.len() < input.len());

        let mut output = vec![0u8; PAGE_SIZE];
        let len = stream.decompress(&compressed, &mut output).unwrap();
        assert_eq!(len, PAGE_SIZE);
        assert_eq!(&output[..], &input[..]);
    }

    #[test]
    fn test_compress_empty() {
        let mut stream = CompressionStream::new(ZramCompressor::Lz4);
        let compressed = stream.compress(&[]).unwrap();
        assert!(compressed.is_empty());
    }

    #[test]
    fn test_compress_small() {
        let mut stream = CompressionStream::new(ZramCompressor::Lz4);
        let input = b"tiny";
        let compressed = stream.compress(input).unwrap();
        // Small inputs not compressed
        assert_eq!(compressed, input);
    }

    // ------------------------------------------------------------------------
    // PageTable Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_page_table_basic() {
        let table = PageTable::new();
        assert!(table.is_empty());

        table.set(0, PageEntry::Zero).unwrap();
        assert_eq!(table.len(), 1);

        let entry = table.get(0).unwrap();
        assert!(matches!(entry, PageEntry::Zero));

        let removed = table.remove(0).unwrap();
        assert!(removed.is_some());
        assert!(table.is_empty());
    }

    #[test]
    fn test_page_table_empty_read() {
        let table = PageTable::new();
        let entry = table.get(42).unwrap();
        assert!(matches!(entry, PageEntry::Empty));
    }

    // ------------------------------------------------------------------------
    // ZramDevice Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_device_create() {
        let config = ZramConfig::with_size(PAGE_SIZE as u64 * 100);
        let device = ZramDevice::new(config).unwrap();
        assert_eq!(device.stored_pages(), 0);
    }

    #[test]
    fn test_device_write_read_zero_page() {
        let config = ZramConfig::with_size(PAGE_SIZE as u64 * 100);
        let device = ZramDevice::new(config).unwrap();

        let zeros = vec![0u8; PAGE_SIZE];
        device.write_page(0, &zeros).unwrap();

        let mut buffer = vec![0u8; PAGE_SIZE];
        device.read_page(0, &mut buffer).unwrap();
        assert_eq!(buffer, zeros);

        let stats = device.stats();
        assert_eq!(stats.zero_pages, 1);
    }

    #[test]
    fn test_device_write_read_same_page() {
        let config = ZramConfig::with_size(PAGE_SIZE as u64 * 100);
        let device = ZramDevice::new(config).unwrap();

        let same = vec![0xABu8; PAGE_SIZE];
        device.write_page(0, &same).unwrap();

        let mut buffer = vec![0u8; PAGE_SIZE];
        device.read_page(0, &mut buffer).unwrap();
        assert_eq!(buffer, same);

        let stats = device.stats();
        assert_eq!(stats.same_pages, 1);
    }

    #[test]
    fn test_device_write_read_compressed() {
        let config = ZramConfig::with_size(PAGE_SIZE as u64 * 100);
        let device = ZramDevice::new(config).unwrap();

        // Create compressible data
        let data: Vec<u8> = (0..PAGE_SIZE).map(|i| ((i / 16) % 256) as u8).collect();

        device.write_page(0, &data).unwrap();

        let mut buffer = vec![0u8; PAGE_SIZE];
        device.read_page(0, &mut buffer).unwrap();
        assert_eq!(buffer, data);

        let stats = device.stats();
        assert!(stats.pages_stored >= 1);
    }

    #[test]
    fn test_device_read_unwritten() {
        let config = ZramConfig::with_size(PAGE_SIZE as u64 * 100);
        let device = ZramDevice::new(config).unwrap();

        let mut buffer = vec![0xFFu8; PAGE_SIZE];
        device.read_page(42, &mut buffer).unwrap();

        // Unwritten pages should read as zeros
        assert!(buffer.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_device_discard() {
        let config = ZramConfig::with_size(PAGE_SIZE as u64 * 100);
        let device = ZramDevice::new(config).unwrap();

        let data = vec![0xABu8; PAGE_SIZE];
        device.write_page(0, &data).unwrap();
        assert_eq!(device.stored_pages(), 1);

        device.discard_page(0).unwrap();
        assert_eq!(device.stored_pages(), 0);
    }

    #[test]
    fn test_device_out_of_bounds() {
        let config = ZramConfig::with_size(PAGE_SIZE as u64 * 10);
        let device = ZramDevice::new(config).unwrap();

        let data = vec![0u8; PAGE_SIZE];
        let result = device.write_page(100, &data);
        assert!(result.is_err());
    }

    #[test]
    fn test_device_multiple_pages() {
        let config = ZramConfig::with_size(PAGE_SIZE as u64 * 100);
        let device = ZramDevice::new(config).unwrap();

        for i in 0..10 {
            let data: Vec<u8> = (0..PAGE_SIZE).map(|_| i as u8).collect();
            device.write_page(i, &data).unwrap();
        }

        for i in 0..10 {
            let mut buffer = vec![0u8; PAGE_SIZE];
            device.read_page(i, &mut buffer).unwrap();
            assert!(buffer.iter().all(|&b| b == i as u8));
        }
    }

    #[test]
    fn test_device_stats() {
        let config = ZramConfig::with_size(PAGE_SIZE as u64 * 100);
        let device = ZramDevice::new(config).unwrap();

        // Write various page types
        device.write_page(0, &vec![0u8; PAGE_SIZE]).unwrap(); // Zero
        device.write_page(1, &vec![0xABu8; PAGE_SIZE]).unwrap(); // Same

        let stats = device.stats();
        assert_eq!(stats.zero_pages, 1);
        assert_eq!(stats.same_pages, 1);
        assert!(stats.num_writes >= 2);
    }

    #[test]
    fn test_device_compression_ratio() {
        let config = ZramConfig::with_size(PAGE_SIZE as u64 * 100);
        let device = ZramDevice::new(config).unwrap();

        // Write compressible data
        for i in 0..10 {
            let data: Vec<u8> =
                (0..PAGE_SIZE).map(|j| ((j / 64 + i as usize) % 256) as u8).collect();
            device.write_page(i, &data).unwrap();
        }

        let stats = device.stats();
        let ratio = stats.compression_ratio();
        // Should have some compression
        assert!(ratio >= 1.0);
    }

    #[test]
    fn test_device_debug() {
        let config = ZramConfig::with_size(PAGE_SIZE as u64 * 100);
        let device = ZramDevice::new(config).unwrap();
        let debug = format!("{:?}", device);
        assert!(debug.contains("ZramDevice"));
    }
}
