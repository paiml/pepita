//! Block multi-queue (blk-mq) interface.
//!
//! This module provides Rust abstractions for the Linux kernel's blk-mq
//! subsystem, which is the modern block device layer supporting multiple
//! hardware queues.
//!
//! ## Key Concepts
//!
//! - **Tag**: Unique identifier for in-flight requests
//! - **Hardware Queue**: Per-CPU queue for request submission
//! - **Request**: Block I/O operation (read, write, flush, etc.)
//!
//! ## ublk Integration
//!
//! The ublk driver uses blk-mq to manage requests. Each request has a tag
//! that matches the `UblkIoCmd.tag` field.

use crate::error::{KernelError, Result};

// ============================================================================
// CONSTANTS
// ============================================================================

/// Maximum queue depth (tags per queue)
pub const BLK_MQ_MAX_DEPTH: u16 = 32768;

/// Maximum number of hardware queues
pub const BLK_MQ_MAX_HW_QUEUES: u16 = 128;

// ============================================================================
// REQUEST OPERATIONS
// ============================================================================

/// Block request operation type.
///
/// Represents the type of I/O operation requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RequestOp {
    /// Read data from device
    Read = 0,
    /// Write data to device
    Write = 1,
    /// Flush device buffers
    Flush = 2,
    /// Discard sectors
    Discard = 3,
    /// Write zeroes to sectors
    WriteZeroes = 4,
    /// Secure erase
    SecureErase = 5,
    /// Zone reset
    ZoneReset = 6,
    /// Zone open
    ZoneOpen = 7,
    /// Zone close
    ZoneClose = 8,
    /// Zone finish
    ZoneFinish = 9,
}

impl RequestOp {
    /// Check if this is a read operation.
    #[must_use]
    pub const fn is_read(&self) -> bool {
        matches!(self, Self::Read)
    }

    /// Check if this is a write operation.
    #[must_use]
    pub const fn is_write(&self) -> bool {
        matches!(self, Self::Write)
    }

    /// Check if this operation transfers data.
    #[must_use]
    pub const fn has_data(&self) -> bool {
        matches!(self, Self::Read | Self::Write)
    }

    /// Check if this operation is a zone command.
    #[must_use]
    pub const fn is_zone_op(&self) -> bool {
        matches!(
            self,
            Self::ZoneReset | Self::ZoneOpen | Self::ZoneClose | Self::ZoneFinish
        )
    }

    /// Convert from u8.
    #[must_use]
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Read),
            1 => Some(Self::Write),
            2 => Some(Self::Flush),
            3 => Some(Self::Discard),
            4 => Some(Self::WriteZeroes),
            5 => Some(Self::SecureErase),
            6 => Some(Self::ZoneReset),
            7 => Some(Self::ZoneOpen),
            8 => Some(Self::ZoneClose),
            9 => Some(Self::ZoneFinish),
            _ => None,
        }
    }
}

// ============================================================================
// REQUEST FLAGS
// ============================================================================

/// Request flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RequestFlags(u32);

impl RequestFlags {
    /// No flags
    pub const NONE: Self = Self(0);

    /// Force unit access (bypass cache)
    pub const FUA: Self = Self(1 << 0);

    /// Metadata request
    pub const META: Self = Self(1 << 1);

    /// Synchronous request
    pub const SYNC: Self = Self(1 << 2);

    /// No wait (return EAGAIN if would block)
    pub const NOWAIT: Self = Self(1 << 3);

    /// Create a new flags value.
    #[must_use]
    pub const fn new() -> Self {
        Self::NONE
    }

    /// Set the FUA flag.
    #[must_use]
    pub const fn with_fua(self) -> Self {
        Self(self.0 | Self::FUA.0)
    }

    /// Set the sync flag.
    #[must_use]
    pub const fn with_sync(self) -> Self {
        Self(self.0 | Self::SYNC.0)
    }

    /// Set the nowait flag.
    #[must_use]
    pub const fn with_nowait(self) -> Self {
        Self(self.0 | Self::NOWAIT.0)
    }

    /// Check if FUA is set.
    #[must_use]
    pub const fn is_fua(&self) -> bool {
        (self.0 & Self::FUA.0) != 0
    }

    /// Check if sync is set.
    #[must_use]
    pub const fn is_sync(&self) -> bool {
        (self.0 & Self::SYNC.0) != 0
    }

    /// Check if nowait is set.
    #[must_use]
    pub const fn is_nowait(&self) -> bool {
        (self.0 & Self::NOWAIT.0) != 0
    }

    /// Get the raw value.
    #[must_use]
    pub const fn bits(&self) -> u32 {
        self.0
    }
}

// ============================================================================
// BIO VECTOR
// ============================================================================

/// Bio vector - describes a contiguous memory region for I/O.
///
/// This is a simplified version of the kernel's `bio_vec` structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BioVec {
    /// Base address of the buffer
    pub addr: u64,
    /// Length in bytes
    pub len: u32,
    /// Offset within page (for page-aligned operations)
    pub offset: u32,
}

impl BioVec {
    /// Create a new bio vector.
    #[must_use]
    pub const fn new(addr: u64, len: u32) -> Self {
        Self {
            addr,
            len,
            offset: 0,
        }
    }

    /// Create a new bio vector with offset.
    #[must_use]
    pub const fn with_offset(addr: u64, len: u32, offset: u32) -> Self {
        Self { addr, len, offset }
    }

    /// Get the effective address (addr + offset).
    #[must_use]
    pub const fn effective_addr(&self) -> u64 {
        self.addr + self.offset as u64
    }

    /// Check if the buffer is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Default for BioVec {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

// ============================================================================
// REQUEST STRUCTURE
// ============================================================================

/// Block I/O request.
///
/// Represents a single block I/O operation with all necessary metadata.
/// Mirrors the kernel's `struct request` for ublk integration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Request tag (unique within queue)
    tag: u16,
    /// Queue ID this request belongs to
    queue_id: u16,
    /// Operation type
    op: RequestOp,
    /// Request flags
    flags: RequestFlags,
    /// Starting sector (LBA)
    sector: u64,
    /// Number of sectors
    nr_sectors: u32,
    /// Data buffer
    bio_vec: BioVec,
}

impl Request {
    /// Create a new request.
    ///
    /// # Arguments
    ///
    /// * `tag` - Request tag
    /// * `queue_id` - Queue identifier
    /// * `op` - Operation type
    #[must_use]
    pub const fn new(tag: u16, queue_id: u16, op: RequestOp) -> Self {
        Self {
            tag,
            queue_id,
            op,
            flags: RequestFlags::NONE,
            sector: 0,
            nr_sectors: 0,
            bio_vec: BioVec::new(0, 0),
        }
    }

    /// Create a read request.
    #[must_use]
    pub const fn read(tag: u16, queue_id: u16, sector: u64, nr_sectors: u32) -> Self {
        Self {
            tag,
            queue_id,
            op: RequestOp::Read,
            flags: RequestFlags::NONE,
            sector,
            nr_sectors,
            bio_vec: BioVec::new(0, 0),
        }
    }

    /// Create a write request.
    #[must_use]
    pub const fn write(tag: u16, queue_id: u16, sector: u64, nr_sectors: u32) -> Self {
        Self {
            tag,
            queue_id,
            op: RequestOp::Write,
            flags: RequestFlags::NONE,
            sector,
            nr_sectors,
            bio_vec: BioVec::new(0, 0),
        }
    }

    /// Get the request tag.
    #[must_use]
    pub const fn tag(&self) -> u16 {
        self.tag
    }

    /// Get the queue ID.
    #[must_use]
    pub const fn queue_id(&self) -> u16 {
        self.queue_id
    }

    /// Get the operation type.
    #[must_use]
    pub const fn op(&self) -> RequestOp {
        self.op
    }

    /// Get the flags.
    #[must_use]
    pub const fn flags(&self) -> RequestFlags {
        self.flags
    }

    /// Get the starting sector.
    #[must_use]
    pub const fn sector(&self) -> u64 {
        self.sector
    }

    /// Get the number of sectors.
    #[must_use]
    pub const fn nr_sectors(&self) -> u32 {
        self.nr_sectors
    }

    /// Get the bio vector.
    #[must_use]
    pub const fn bio_vec(&self) -> &BioVec {
        &self.bio_vec
    }

    /// Set the sector range.
    pub fn set_sector_range(&mut self, sector: u64, nr_sectors: u32) {
        self.sector = sector;
        self.nr_sectors = nr_sectors;
    }

    /// Set the bio vector.
    pub fn set_bio_vec(&mut self, bio_vec: BioVec) {
        self.bio_vec = bio_vec;
    }

    /// Set flags.
    pub fn set_flags(&mut self, flags: RequestFlags) {
        self.flags = flags;
    }

    /// Calculate byte offset.
    #[must_use]
    pub const fn byte_offset(&self) -> u64 {
        self.sector * 512
    }

    /// Calculate byte length.
    #[must_use]
    pub const fn byte_len(&self) -> u64 {
        self.nr_sectors as u64 * 512
    }
}

// ============================================================================
// TAG SET CONFIGURATION
// ============================================================================

/// Tag set configuration.
///
/// Specifies the parameters for creating a blk-mq tag set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TagSetConfig {
    /// Number of hardware queues
    pub nr_hw_queues: u16,
    /// Queue depth (max concurrent requests per queue)
    pub queue_depth: u16,
    /// NUMA node for allocation (-1 for any)
    pub numa_node: i32,
    /// Configuration flags
    pub flags: u32,
}

impl TagSetConfig {
    /// Create a new tag set configuration.
    ///
    /// # Arguments
    ///
    /// * `nr_hw_queues` - Number of hardware queues
    /// * `queue_depth` - Maximum concurrent requests per queue
    #[must_use]
    pub const fn new(nr_hw_queues: u16, queue_depth: u16) -> Self {
        Self {
            nr_hw_queues,
            queue_depth,
            numa_node: -1,
            flags: 0,
        }
    }

    /// Set the NUMA node.
    #[must_use]
    pub const fn with_numa_node(mut self, node: i32) -> Self {
        self.numa_node = node;
        self
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<()> {
        if self.nr_hw_queues == 0 {
            return Err(KernelError::InvalidArgument);
        }
        if self.nr_hw_queues > BLK_MQ_MAX_HW_QUEUES {
            return Err(KernelError::InvalidArgument);
        }
        if self.queue_depth == 0 {
            return Err(KernelError::InvalidArgument);
        }
        if self.queue_depth > BLK_MQ_MAX_DEPTH {
            return Err(KernelError::InvalidArgument);
        }
        Ok(())
    }

    /// Calculate total number of tags.
    #[must_use]
    pub const fn total_tags(&self) -> u32 {
        self.nr_hw_queues as u32 * self.queue_depth as u32
    }
}

impl Default for TagSetConfig {
    fn default() -> Self {
        Self::new(1, 128)
    }
}

// ============================================================================
// BLOCK OPERATIONS TRAIT
// ============================================================================

/// Block device operations trait.
///
/// Implement this trait to create a block device driver.
pub trait BlockOps: Send + Sync {
    /// Queue-specific data type.
    type QueueData: Send + Sync;

    /// Queue a request for processing.
    ///
    /// # Arguments
    ///
    /// * `queue_data` - Queue-specific data
    /// * `request` - The request to process
    /// * `is_last` - True if this is the last request in a batch
    ///
    /// # Returns
    ///
    /// Ok(()) if the request was queued, or an error.
    fn queue_rq(
        queue_data: &Self::QueueData,
        request: &Request,
        is_last: bool,
    ) -> Result<()>;

    /// Commit outstanding requests.
    ///
    /// Called after all requests in a batch have been queued.
    fn commit_rqs(queue_data: &Self::QueueData);

    /// Complete a request.
    ///
    /// Called when a request has finished processing.
    fn complete(request: &Request, result: i32);
}

// ============================================================================
// TESTS (EXTREME TDD)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // RequestOp Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_request_op_is_read() {
        assert!(RequestOp::Read.is_read());
        assert!(!RequestOp::Write.is_read());
        assert!(!RequestOp::Flush.is_read());
    }

    #[test]
    fn test_request_op_is_write() {
        assert!(RequestOp::Write.is_write());
        assert!(!RequestOp::Read.is_write());
        assert!(!RequestOp::Discard.is_write());
    }

    #[test]
    fn test_request_op_has_data() {
        assert!(RequestOp::Read.has_data());
        assert!(RequestOp::Write.has_data());
        assert!(!RequestOp::Flush.has_data());
        assert!(!RequestOp::Discard.has_data());
    }

    #[test]
    fn test_request_op_is_zone_op() {
        assert!(RequestOp::ZoneReset.is_zone_op());
        assert!(RequestOp::ZoneOpen.is_zone_op());
        assert!(!RequestOp::Read.is_zone_op());
        assert!(!RequestOp::Write.is_zone_op());
    }

    #[test]
    fn test_request_op_from_u8() {
        assert_eq!(RequestOp::from_u8(0), Some(RequestOp::Read));
        assert_eq!(RequestOp::from_u8(1), Some(RequestOp::Write));
        assert_eq!(RequestOp::from_u8(2), Some(RequestOp::Flush));
        assert_eq!(RequestOp::from_u8(100), None);
    }

    #[test]
    fn test_request_op_repr() {
        assert_eq!(RequestOp::Read as u8, 0);
        assert_eq!(RequestOp::Write as u8, 1);
        assert_eq!(RequestOp::Flush as u8, 2);
    }

    // ------------------------------------------------------------------------
    // RequestFlags Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_request_flags_default() {
        let flags = RequestFlags::default();
        assert!(!flags.is_fua());
        assert!(!flags.is_sync());
        assert!(!flags.is_nowait());
    }

    #[test]
    fn test_request_flags_builders() {
        let flags = RequestFlags::new().with_fua().with_sync();
        assert!(flags.is_fua());
        assert!(flags.is_sync());
        assert!(!flags.is_nowait());
    }

    #[test]
    fn test_request_flags_bits() {
        assert_eq!(RequestFlags::NONE.bits(), 0);
        assert_ne!(RequestFlags::FUA.bits(), 0);
        assert_ne!(RequestFlags::SYNC.bits(), 0);
    }

    // ------------------------------------------------------------------------
    // BioVec Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_bio_vec_new() {
        let bv = BioVec::new(0x1000, 4096);
        assert_eq!(bv.addr, 0x1000);
        assert_eq!(bv.len, 4096);
        assert_eq!(bv.offset, 0);
    }

    #[test]
    fn test_bio_vec_with_offset() {
        let bv = BioVec::with_offset(0x1000, 4096, 512);
        assert_eq!(bv.effective_addr(), 0x1000 + 512);
    }

    #[test]
    fn test_bio_vec_is_empty() {
        assert!(BioVec::new(0, 0).is_empty());
        assert!(!BioVec::new(0x1000, 1).is_empty());
    }

    // ------------------------------------------------------------------------
    // Request Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_request_new() {
        let req = Request::new(10, 0, RequestOp::Read);
        assert_eq!(req.tag(), 10);
        assert_eq!(req.queue_id(), 0);
        assert_eq!(req.op(), RequestOp::Read);
    }

    #[test]
    fn test_request_read() {
        let req = Request::read(5, 1, 1000, 8);
        assert_eq!(req.tag(), 5);
        assert_eq!(req.queue_id(), 1);
        assert_eq!(req.op(), RequestOp::Read);
        assert_eq!(req.sector(), 1000);
        assert_eq!(req.nr_sectors(), 8);
    }

    #[test]
    fn test_request_write() {
        let req = Request::write(6, 2, 2000, 16);
        assert_eq!(req.op(), RequestOp::Write);
        assert_eq!(req.sector(), 2000);
        assert_eq!(req.nr_sectors(), 16);
    }

    #[test]
    fn test_request_byte_calculations() {
        let req = Request::read(0, 0, 100, 8);
        assert_eq!(req.byte_offset(), 100 * 512);
        assert_eq!(req.byte_len(), 8 * 512);
    }

    #[test]
    fn test_request_setters() {
        let mut req = Request::new(0, 0, RequestOp::Read);
        req.set_sector_range(500, 32);
        assert_eq!(req.sector(), 500);
        assert_eq!(req.nr_sectors(), 32);

        req.set_bio_vec(BioVec::new(0x2000, 16384));
        assert_eq!(req.bio_vec().addr, 0x2000);

        req.set_flags(RequestFlags::FUA);
        assert!(req.flags().is_fua());
    }

    // ------------------------------------------------------------------------
    // TagSetConfig Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_tag_set_config_new() {
        let config = TagSetConfig::new(4, 128);
        assert_eq!(config.nr_hw_queues, 4);
        assert_eq!(config.queue_depth, 128);
        assert_eq!(config.numa_node, -1);
    }

    #[test]
    fn test_tag_set_config_with_numa() {
        let config = TagSetConfig::new(2, 64).with_numa_node(0);
        assert_eq!(config.numa_node, 0);
    }

    #[test]
    fn test_tag_set_config_validate() {
        // Valid configs
        assert!(TagSetConfig::new(1, 1).validate().is_ok());
        assert!(TagSetConfig::new(128, 32768).validate().is_ok());

        // Invalid configs
        assert!(TagSetConfig::new(0, 128).validate().is_err());
        assert!(TagSetConfig::new(129, 128).validate().is_err()); // > MAX_HW_QUEUES
        assert!(TagSetConfig::new(1, 0).validate().is_err());
        assert!(TagSetConfig::new(1, 32769).validate().is_err()); // > MAX_DEPTH
    }

    #[test]
    fn test_tag_set_config_total_tags() {
        let config = TagSetConfig::new(4, 128);
        assert_eq!(config.total_tags(), 4 * 128);
    }

    #[test]
    fn test_tag_set_config_default() {
        let config = TagSetConfig::default();
        assert_eq!(config.nr_hw_queues, 1);
        assert_eq!(config.queue_depth, 128);
        assert!(config.validate().is_ok());
    }

    // ------------------------------------------------------------------------
    // Constants Tests (Falsification Checklist Points 16-18)
    // ------------------------------------------------------------------------

    #[test]
    fn abi_max_queue_depth() {
        assert_eq!(BLK_MQ_MAX_DEPTH, 32768, "max depth must be 32768");
    }

    #[test]
    fn abi_tag_width() {
        // Tags are u16
        let max_tag: u16 = u16::MAX;
        assert!(max_tag >= BLK_MQ_MAX_DEPTH);
    }

    #[test]
    fn abi_queue_id_width() {
        // Queue IDs are u16
        let max_queue: u16 = u16::MAX;
        assert!(max_queue >= BLK_MQ_MAX_HW_QUEUES);
    }
}
