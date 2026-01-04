//! Virtio: Virtual I/O Devices
//!
//! Pure Rust virtio device implementations for MicroVM communication.
//! Supports virtio-vsock (socket), virtio-blk (block device).
//!
//! ## Example
//!
//! ```rust,ignore
//! use pepita::virtio::{VirtQueue, VirtioVsock, VsockAddr};
//!
//! // Create vsock device
//! let vsock = VirtioVsock::new(3); // CID 3
//!
//! // Connect to host
//! let addr = VsockAddr::new(2, 1234); // Host CID 2, port 1234
//! vsock.connect(addr)?;
//! ```

use crate::error::{KernelError, Result};

#[cfg(feature = "std")]
use std::collections::HashMap;
#[cfg(feature = "std")]
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// ============================================================================
// VIRTIO CONSTANTS
// ============================================================================

/// Virtio device types
pub mod device_type {
    /// Network device
    pub const NET: u32 = 1;
    /// Block device
    pub const BLK: u32 = 2;
    /// Console
    pub const CONSOLE: u32 = 3;
    /// Entropy source
    pub const RNG: u32 = 4;
    /// Memory balloon
    pub const BALLOON: u32 = 5;
    /// SCSI host
    pub const SCSI: u32 = 8;
    /// 9P transport
    pub const P9: u32 = 9;
    /// GPU
    pub const GPU: u32 = 16;
    /// Input device
    pub const INPUT: u32 = 18;
    /// Vsock (socket)
    pub const VSOCK: u32 = 19;
    /// Crypto
    pub const CRYPTO: u32 = 20;
    /// File system
    pub const FS: u32 = 26;
}

/// Virtio device status bits
pub mod status {
    /// Driver acknowledged device
    pub const ACKNOWLEDGE: u8 = 1;
    /// Driver knows how to drive device
    pub const DRIVER: u8 = 2;
    /// Driver is ready
    pub const DRIVER_OK: u8 = 4;
    /// Feature negotiation complete
    pub const FEATURES_OK: u8 = 8;
    /// Device needs reset
    pub const DEVICE_NEEDS_RESET: u8 = 64;
    /// Something went wrong
    pub const FAILED: u8 = 128;
}

/// Virtio ring flags
pub mod ring_flags {
    /// No interrupt on used buffer
    pub const AVAIL_NO_INTERRUPT: u16 = 1;
    /// No notification on available buffer
    pub const USED_NO_NOTIFY: u16 = 1;
}

/// Default queue size
pub const DEFAULT_QUEUE_SIZE: u16 = 256;

/// Maximum queue size
pub const MAX_QUEUE_SIZE: u16 = 32768;

// ============================================================================
// VIRTQUEUE DESCRIPTOR
// ============================================================================

/// Virtqueue descriptor
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct VirtqDesc {
    /// Guest physical address
    pub addr: u64,
    /// Length of buffer
    pub len: u32,
    /// Flags
    pub flags: u16,
    /// Next descriptor index (if NEXT flag set)
    pub next: u16,
}

impl VirtqDesc {
    /// Descriptor flags: next descriptor in chain
    pub const F_NEXT: u16 = 1;
    /// Descriptor flags: device write-only
    pub const F_WRITE: u16 = 2;
    /// Descriptor flags: indirect descriptor
    pub const F_INDIRECT: u16 = 4;

    /// Create a new descriptor
    #[must_use]
    pub const fn new(addr: u64, len: u32, flags: u16, next: u16) -> Self {
        Self {
            addr,
            len,
            flags,
            next,
        }
    }

    /// Check if descriptor has next
    #[must_use]
    pub const fn has_next(&self) -> bool {
        self.flags & Self::F_NEXT != 0
    }

    /// Check if descriptor is write-only
    #[must_use]
    pub const fn is_write_only(&self) -> bool {
        self.flags & Self::F_WRITE != 0
    }

    /// Check if descriptor is indirect
    #[must_use]
    pub const fn is_indirect(&self) -> bool {
        self.flags & Self::F_INDIRECT != 0
    }
}

// ============================================================================
// VIRTQUEUE AVAILABLE RING
// ============================================================================

/// Available ring element
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct VirtqAvail {
    /// Flags
    pub flags: u16,
    /// Index of next available descriptor
    pub idx: u16,
    /// Ring entries (descriptor indices)
    pub ring: [u16; 0], // Variable size
}

/// Used ring element
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct VirtqUsedElem {
    /// Index of descriptor chain head
    pub id: u32,
    /// Number of bytes written
    pub len: u32,
}

/// Used ring
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct VirtqUsed {
    /// Flags
    pub flags: u16,
    /// Index of next used element
    pub idx: u16,
    /// Ring entries
    pub ring: [VirtqUsedElem; 0], // Variable size
}

// ============================================================================
// VIRTQUEUE
// ============================================================================

/// Virtqueue configuration
#[derive(Debug, Clone, Copy)]
pub struct VirtQueueConfig {
    /// Queue size (number of descriptors)
    pub size: u16,
    /// Descriptor table address
    pub desc_addr: u64,
    /// Available ring address
    pub avail_addr: u64,
    /// Used ring address
    pub used_addr: u64,
}

impl Default for VirtQueueConfig {
    fn default() -> Self {
        Self {
            size: DEFAULT_QUEUE_SIZE,
            desc_addr: 0,
            avail_addr: 0,
            used_addr: 0,
        }
    }
}

/// Virtqueue state
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct VirtQueue {
    /// Configuration
    config: VirtQueueConfig,
    /// Queue ready
    ready: AtomicBool,
    /// Last seen available index
    last_avail_idx: AtomicU64,
    /// Last used index
    last_used_idx: AtomicU64,
    /// Pending descriptors
    pending: std::sync::Mutex<Vec<u16>>,
}

#[cfg(feature = "std")]
impl VirtQueue {
    /// Create a new virtqueue
    #[must_use]
    pub fn new(config: VirtQueueConfig) -> Self {
        Self {
            config,
            ready: AtomicBool::new(false),
            last_avail_idx: AtomicU64::new(0),
            last_used_idx: AtomicU64::new(0),
            pending: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Create with default config and size
    #[must_use]
    pub fn with_size(size: u16) -> Self {
        Self::new(VirtQueueConfig {
            size,
            ..Default::default()
        })
    }

    /// Check if queue is ready
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    /// Set queue ready
    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::Release);
    }

    /// Get queue size
    #[must_use]
    pub const fn size(&self) -> u16 {
        self.config.size
    }

    /// Get last available index
    #[must_use]
    pub fn avail_idx(&self) -> u64 {
        self.last_avail_idx.load(Ordering::Acquire)
    }

    /// Get last used index
    #[must_use]
    pub fn used_idx(&self) -> u64 {
        self.last_used_idx.load(Ordering::Acquire)
    }

    /// Add descriptor index to pending
    pub fn add_pending(&self, desc_idx: u16) -> Result<()> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| KernelError::ResourceBusy)?;
        pending.push(desc_idx);
        Ok(())
    }

    /// Pop pending descriptor
    pub fn pop_pending(&self) -> Result<Option<u16>> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| KernelError::ResourceBusy)?;
        Ok(pending.pop())
    }

    /// Get number of pending descriptors
    pub fn pending_count(&self) -> usize {
        self.pending.lock().map(|p| p.len()).unwrap_or(0)
    }

    /// Notify queue (signal device)
    pub fn notify(&self) -> Result<()> {
        if !self.is_ready() {
            return Err(KernelError::DeviceNotReady);
        }
        // In real implementation, would write to notification register
        Ok(())
    }

    /// Mark descriptor as used
    pub fn mark_used(&self, _desc_idx: u16, _len: u32) -> Result<()> {
        self.last_used_idx.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}

// ============================================================================
// VSOCK ADDRESS
// ============================================================================

/// Vsock address (CID + port)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VsockAddr {
    /// Context ID (CID)
    pub cid: u64,
    /// Port number
    pub port: u32,
}

impl VsockAddr {
    /// Host CID
    pub const HOST_CID: u64 = 2;
    /// Any CID (for binding)
    pub const ANY_CID: u64 = u64::MAX;
    /// Any port (for binding)
    pub const ANY_PORT: u32 = u32::MAX;

    /// Create a new vsock address
    #[must_use]
    pub const fn new(cid: u64, port: u32) -> Self {
        Self { cid, port }
    }

    /// Create host address
    #[must_use]
    pub const fn host(port: u32) -> Self {
        Self::new(Self::HOST_CID, port)
    }

    /// Check if this is host address
    #[must_use]
    pub const fn is_host(&self) -> bool {
        self.cid == Self::HOST_CID
    }
}

impl std::fmt::Display for VsockAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.cid, self.port)
    }
}

// ============================================================================
// VSOCK PACKET
// ============================================================================

/// Vsock packet type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum VsockPacketType {
    /// Invalid
    Invalid = 0,
    /// Request connection
    Request = 1,
    /// Response to request
    Response = 2,
    /// Reset connection
    Rst = 3,
    /// Shutdown connection
    Shutdown = 4,
    /// Read/write data
    Rw = 5,
    /// Credit update
    CreditUpdate = 6,
    /// Credit request
    CreditRequest = 7,
}

impl From<u16> for VsockPacketType {
    fn from(value: u16) -> Self {
        match value {
            1 => Self::Request,
            2 => Self::Response,
            3 => Self::Rst,
            4 => Self::Shutdown,
            5 => Self::Rw,
            6 => Self::CreditUpdate,
            7 => Self::CreditRequest,
            _ => Self::Invalid,
        }
    }
}

/// Vsock packet header
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct VsockPacketHeader {
    /// Source CID
    pub src_cid: u64,
    /// Destination CID
    pub dst_cid: u64,
    /// Source port
    pub src_port: u32,
    /// Destination port
    pub dst_port: u32,
    /// Payload length
    pub len: u32,
    /// Packet type
    pub packet_type: u16,
    /// Operation
    pub op: u16,
    /// Flags
    pub flags: u32,
    /// Buffer allocation
    pub buf_alloc: u32,
    /// Forward count
    pub fwd_cnt: u32,
}

impl VsockPacketHeader {
    /// Header size in bytes
    pub const SIZE: usize = 44;

    /// Create a new packet header
    #[must_use]
    pub const fn new(src: VsockAddr, dst: VsockAddr, packet_type: VsockPacketType) -> Self {
        Self {
            src_cid: src.cid,
            dst_cid: dst.cid,
            src_port: src.port,
            dst_port: dst.port,
            len: 0,
            packet_type: packet_type as u16,
            op: 0,
            flags: 0,
            buf_alloc: 0,
            fwd_cnt: 0,
        }
    }

    /// Get packet type
    #[must_use]
    pub fn get_type(&self) -> VsockPacketType {
        VsockPacketType::from(self.packet_type)
    }

    /// Get source address
    #[must_use]
    pub const fn src(&self) -> VsockAddr {
        VsockAddr::new(self.src_cid, self.src_port)
    }

    /// Get destination address
    #[must_use]
    pub const fn dst(&self) -> VsockAddr {
        VsockAddr::new(self.dst_cid, self.dst_port)
    }
}

// ============================================================================
// VIRTIO VSOCK
// ============================================================================

/// Vsock connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum VsockConnState {
    /// Not connected
    #[default]
    Disconnected = 0,
    /// Connection requested
    Connecting = 1,
    /// Connected
    Connected = 2,
    /// Disconnecting
    Disconnecting = 3,
}

/// Vsock connection
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct VsockConnection {
    /// Local address
    local: VsockAddr,
    /// Remote address
    remote: VsockAddr,
    /// Connection state
    state: VsockConnState,
    /// Receive buffer
    rx_buffer: Vec<u8>,
    /// Transmit buffer
    tx_buffer: Vec<u8>,
    /// Buffer allocation (credit)
    buf_alloc: u32,
    /// Forward count
    fwd_cnt: u32,
}

#[cfg(feature = "std")]
impl VsockConnection {
    /// Create a new connection
    #[must_use]
    pub fn new(local: VsockAddr, remote: VsockAddr) -> Self {
        Self {
            local,
            remote,
            state: VsockConnState::Disconnected,
            rx_buffer: Vec::with_capacity(65536),
            tx_buffer: Vec::with_capacity(65536),
            buf_alloc: 65536,
            fwd_cnt: 0,
        }
    }

    /// Get connection state
    #[must_use]
    pub const fn state(&self) -> VsockConnState {
        self.state
    }

    /// Check if connected
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        matches!(self.state, VsockConnState::Connected)
    }

    /// Get local address
    #[must_use]
    pub const fn local(&self) -> VsockAddr {
        self.local
    }

    /// Get remote address
    #[must_use]
    pub const fn remote(&self) -> VsockAddr {
        self.remote
    }

    /// Get available receive buffer space
    #[must_use]
    pub fn available_credit(&self) -> u32 {
        self.buf_alloc.saturating_sub(self.rx_buffer.len() as u32)
    }

    /// Get pending transmit data length
    #[must_use]
    pub fn pending_tx(&self) -> usize {
        self.tx_buffer.len()
    }

    /// Get forward count (bytes acknowledged by peer)
    #[must_use]
    pub const fn forward_count(&self) -> u32 {
        self.fwd_cnt
    }

    /// Queue data for transmission
    pub fn queue_tx(&mut self, data: &[u8]) {
        self.tx_buffer.extend_from_slice(data);
    }

    /// Acknowledge received bytes
    pub fn acknowledge(&mut self, bytes: u32) {
        self.fwd_cnt = self.fwd_cnt.saturating_add(bytes);
    }
}

/// Virtio vsock device
#[cfg(feature = "std")]
pub struct VirtioVsock {
    /// Context ID for this device
    cid: u64,
    /// Receive queue
    rx_queue: VirtQueue,
    /// Transmit queue
    tx_queue: VirtQueue,
    /// Event queue
    event_queue: VirtQueue,
    /// Active connections
    connections: std::sync::RwLock<HashMap<(VsockAddr, VsockAddr), VsockConnection>>,
    /// Device ready
    ready: AtomicBool,
}

#[cfg(feature = "std")]
impl VirtioVsock {
    /// Create a new vsock device
    #[must_use]
    pub fn new(cid: u64) -> Self {
        Self {
            cid,
            rx_queue: VirtQueue::with_size(DEFAULT_QUEUE_SIZE),
            tx_queue: VirtQueue::with_size(DEFAULT_QUEUE_SIZE),
            event_queue: VirtQueue::with_size(DEFAULT_QUEUE_SIZE),
            connections: std::sync::RwLock::new(HashMap::new()),
            ready: AtomicBool::new(false),
        }
    }

    /// Get device CID
    #[must_use]
    pub const fn cid(&self) -> u64 {
        self.cid
    }

    /// Check if device is ready
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    /// Activate the device
    pub fn activate(&self) {
        self.rx_queue.set_ready(true);
        self.tx_queue.set_ready(true);
        self.event_queue.set_ready(true);
        self.ready.store(true, Ordering::Release);
    }

    /// Deactivate the device
    pub fn deactivate(&self) {
        self.ready.store(false, Ordering::Release);
        self.rx_queue.set_ready(false);
        self.tx_queue.set_ready(false);
        self.event_queue.set_ready(false);
    }

    /// Connect to a remote address
    pub fn connect(&self, remote: VsockAddr) -> Result<()> {
        if !self.is_ready() {
            return Err(KernelError::DeviceNotReady);
        }

        let local = VsockAddr::new(self.cid, 0); // Ephemeral port
        let conn = VsockConnection::new(local, remote);

        let mut connections = self
            .connections
            .write()
            .map_err(|_| KernelError::ResourceBusy)?;
        connections.insert((local, remote), conn);

        Ok(())
    }

    /// Get number of active connections
    pub fn connection_count(&self) -> usize {
        self.connections.read().map(|c| c.len()).unwrap_or(0)
    }

    /// Send data (mock)
    pub fn send(&self, _remote: VsockAddr, _data: &[u8]) -> Result<usize> {
        if !self.is_ready() {
            return Err(KernelError::DeviceNotReady);
        }
        // Mock: just return data length
        Ok(_data.len())
    }

    /// Receive data (mock)
    pub fn recv(&self, _remote: VsockAddr, _buffer: &mut [u8]) -> Result<usize> {
        if !self.is_ready() {
            return Err(KernelError::DeviceNotReady);
        }
        // Mock: return 0 (no data)
        Ok(0)
    }
}

#[cfg(feature = "std")]
impl std::fmt::Debug for VirtioVsock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtioVsock")
            .field("cid", &self.cid)
            .field("ready", &self.is_ready())
            .field("connections", &self.connection_count())
            .finish()
    }
}

// ============================================================================
// VIRTIO BLOCK
// ============================================================================

/// Block request type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum BlockRequestType {
    /// Read sectors
    In = 0,
    /// Write sectors
    Out = 1,
    /// Flush
    Flush = 4,
    /// Discard
    Discard = 11,
    /// Write zeroes
    WriteZeroes = 13,
}

impl From<u32> for BlockRequestType {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::In,
            1 => Self::Out,
            4 => Self::Flush,
            11 => Self::Discard,
            13 => Self::WriteZeroes,
            _ => Self::In, // Default
        }
    }
}

/// Block request header
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BlockRequestHeader {
    /// Request type
    pub request_type: u32,
    /// Reserved
    pub reserved: u32,
    /// Sector (512-byte blocks)
    pub sector: u64,
}

impl BlockRequestHeader {
    /// Header size
    pub const SIZE: usize = 16;

    /// Create a new read request
    #[must_use]
    pub const fn read(sector: u64) -> Self {
        Self {
            request_type: BlockRequestType::In as u32,
            reserved: 0,
            sector,
        }
    }

    /// Create a new write request
    #[must_use]
    pub const fn write(sector: u64) -> Self {
        Self {
            request_type: BlockRequestType::Out as u32,
            reserved: 0,
            sector,
        }
    }

    /// Create a flush request
    #[must_use]
    pub const fn flush() -> Self {
        Self {
            request_type: BlockRequestType::Flush as u32,
            reserved: 0,
            sector: 0,
        }
    }

    /// Get request type
    #[must_use]
    pub fn get_type(&self) -> BlockRequestType {
        BlockRequestType::from(self.request_type)
    }
}

/// Block device configuration
#[derive(Debug, Clone)]
pub struct BlockConfig {
    /// Capacity in 512-byte sectors
    pub capacity: u64,
    /// Block size
    pub blk_size: u32,
    /// Read-only flag
    pub read_only: bool,
}

impl Default for BlockConfig {
    fn default() -> Self {
        Self {
            capacity: 0,
            blk_size: 512,
            read_only: false,
        }
    }
}

/// Virtio block device
#[cfg(feature = "std")]
pub struct VirtioBlock {
    /// Configuration
    config: BlockConfig,
    /// Request queue
    queue: VirtQueue,
    /// Backend storage (mock: in-memory)
    storage: std::sync::RwLock<Vec<u8>>,
    /// Device ready
    ready: AtomicBool,
    /// I/O stats
    reads: AtomicU64,
    writes: AtomicU64,
}

#[cfg(feature = "std")]
impl VirtioBlock {
    /// Create a new block device
    pub fn new(config: BlockConfig) -> Self {
        let storage_size = (config.capacity * 512) as usize;
        Self {
            config,
            queue: VirtQueue::with_size(DEFAULT_QUEUE_SIZE),
            storage: std::sync::RwLock::new(vec![0u8; storage_size]),
            ready: AtomicBool::new(false),
            reads: AtomicU64::new(0),
            writes: AtomicU64::new(0),
        }
    }

    /// Create with capacity in MiB
    #[must_use]
    pub fn with_capacity_mib(mib: u64) -> Self {
        let sectors = mib * 1024 * 1024 / 512;
        Self::new(BlockConfig {
            capacity: sectors,
            ..Default::default()
        })
    }

    /// Get capacity in sectors
    #[must_use]
    pub const fn capacity(&self) -> u64 {
        self.config.capacity
    }

    /// Get capacity in bytes
    #[must_use]
    pub const fn capacity_bytes(&self) -> u64 {
        self.config.capacity * 512
    }

    /// Check if read-only
    #[must_use]
    pub const fn is_read_only(&self) -> bool {
        self.config.read_only
    }

    /// Check if device is ready
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    /// Activate the device
    pub fn activate(&self) {
        self.queue.set_ready(true);
        self.ready.store(true, Ordering::Release);
    }

    /// Deactivate the device
    pub fn deactivate(&self) {
        self.ready.store(false, Ordering::Release);
        self.queue.set_ready(false);
    }

    /// Read sectors
    pub fn read(&self, sector: u64, buffer: &mut [u8]) -> Result<usize> {
        if !self.is_ready() {
            return Err(KernelError::DeviceNotReady);
        }

        let offset = (sector * 512) as usize;
        let len = buffer.len();

        let storage = self
            .storage
            .read()
            .map_err(|_| KernelError::ResourceBusy)?;

        if offset + len > storage.len() {
            return Err(KernelError::InvalidArgument);
        }

        buffer.copy_from_slice(&storage[offset..offset + len]);
        self.reads.fetch_add(1, Ordering::Relaxed);

        Ok(len)
    }

    /// Write sectors
    pub fn write(&self, sector: u64, data: &[u8]) -> Result<usize> {
        if !self.is_ready() {
            return Err(KernelError::DeviceNotReady);
        }

        if self.config.read_only {
            return Err(KernelError::NotSupported);
        }

        let offset = (sector * 512) as usize;
        let len = data.len();

        let mut storage = self
            .storage
            .write()
            .map_err(|_| KernelError::ResourceBusy)?;

        if offset + len > storage.len() {
            return Err(KernelError::InvalidArgument);
        }

        storage[offset..offset + len].copy_from_slice(data);
        self.writes.fetch_add(1, Ordering::Relaxed);

        Ok(len)
    }

    /// Flush (no-op for in-memory)
    pub fn flush(&self) -> Result<()> {
        if !self.is_ready() {
            return Err(KernelError::DeviceNotReady);
        }
        Ok(())
    }

    /// Get read count
    #[must_use]
    pub fn read_count(&self) -> u64 {
        self.reads.load(Ordering::Relaxed)
    }

    /// Get write count
    #[must_use]
    pub fn write_count(&self) -> u64 {
        self.writes.load(Ordering::Relaxed)
    }
}

#[cfg(feature = "std")]
impl std::fmt::Debug for VirtioBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtioBlock")
            .field("capacity_mib", &(self.capacity_bytes() / 1024 / 1024))
            .field("read_only", &self.is_read_only())
            .field("ready", &self.is_ready())
            .field("reads", &self.read_count())
            .field("writes", &self.write_count())
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
    // VirtqDesc Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_virtq_desc_new() {
        let desc = VirtqDesc::new(0x1000, 4096, VirtqDesc::F_NEXT, 1);
        assert_eq!(desc.addr, 0x1000);
        assert_eq!(desc.len, 4096);
        assert!(desc.has_next());
        assert_eq!(desc.next, 1);
    }

    #[test]
    fn test_virtq_desc_flags() {
        let desc = VirtqDesc::new(0, 0, VirtqDesc::F_WRITE, 0);
        assert!(desc.is_write_only());
        assert!(!desc.has_next());
        assert!(!desc.is_indirect());

        let desc2 = VirtqDesc::new(0, 0, VirtqDesc::F_INDIRECT, 0);
        assert!(desc2.is_indirect());
    }

    // ------------------------------------------------------------------------
    // VirtQueue Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_virtqueue_new() {
        let config = VirtQueueConfig::default();
        let queue = VirtQueue::new(config);

        assert!(!queue.is_ready());
        assert_eq!(queue.size(), DEFAULT_QUEUE_SIZE);
    }

    #[test]
    fn test_virtqueue_with_size() {
        let queue = VirtQueue::with_size(128);
        assert_eq!(queue.size(), 128);
    }

    #[test]
    fn test_virtqueue_ready() {
        let queue = VirtQueue::with_size(64);
        assert!(!queue.is_ready());

        queue.set_ready(true);
        assert!(queue.is_ready());

        queue.set_ready(false);
        assert!(!queue.is_ready());
    }

    #[test]
    fn test_virtqueue_pending() {
        let queue = VirtQueue::with_size(64);

        queue.add_pending(0).unwrap();
        queue.add_pending(1).unwrap();
        queue.add_pending(2).unwrap();

        assert_eq!(queue.pending_count(), 3);

        assert_eq!(queue.pop_pending().unwrap(), Some(2));
        assert_eq!(queue.pop_pending().unwrap(), Some(1));
        assert_eq!(queue.pop_pending().unwrap(), Some(0));
        assert_eq!(queue.pop_pending().unwrap(), None);
    }

    #[test]
    fn test_virtqueue_notify_not_ready() {
        let queue = VirtQueue::with_size(64);
        assert!(queue.notify().is_err());
    }

    #[test]
    fn test_virtqueue_notify_ready() {
        let queue = VirtQueue::with_size(64);
        queue.set_ready(true);
        assert!(queue.notify().is_ok());
    }

    // ------------------------------------------------------------------------
    // VsockAddr Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_vsock_addr_new() {
        let addr = VsockAddr::new(3, 1234);
        assert_eq!(addr.cid, 3);
        assert_eq!(addr.port, 1234);
    }

    #[test]
    fn test_vsock_addr_host() {
        let addr = VsockAddr::host(8080);
        assert!(addr.is_host());
        assert_eq!(addr.cid, VsockAddr::HOST_CID);
        assert_eq!(addr.port, 8080);
    }

    #[test]
    fn test_vsock_addr_display() {
        let addr = VsockAddr::new(3, 1234);
        assert_eq!(format!("{}", addr), "3:1234");
    }

    // ------------------------------------------------------------------------
    // VsockPacketHeader Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_vsock_packet_header() {
        let src = VsockAddr::new(3, 1000);
        let dst = VsockAddr::host(2000);
        let header = VsockPacketHeader::new(src, dst, VsockPacketType::Request);

        assert_eq!(header.src(), src);
        assert_eq!(header.dst(), dst);
        assert_eq!(header.get_type(), VsockPacketType::Request);
    }

    #[test]
    fn test_vsock_packet_type_from() {
        assert_eq!(VsockPacketType::from(1), VsockPacketType::Request);
        assert_eq!(VsockPacketType::from(5), VsockPacketType::Rw);
        assert_eq!(VsockPacketType::from(99), VsockPacketType::Invalid);
    }

    // ------------------------------------------------------------------------
    // VsockConnection Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_vsock_connection_new() {
        let local = VsockAddr::new(3, 1000);
        let remote = VsockAddr::host(2000);
        let conn = VsockConnection::new(local, remote);

        assert_eq!(conn.local(), local);
        assert_eq!(conn.remote(), remote);
        assert_eq!(conn.state(), VsockConnState::Disconnected);
        assert!(!conn.is_connected());
    }

    #[test]
    fn test_vsock_connection_credit() {
        let conn = VsockConnection::new(VsockAddr::new(3, 1000), VsockAddr::host(2000));
        assert!(conn.available_credit() > 0);
    }

    #[test]
    fn test_vsock_connection_tx_buffer() {
        let mut conn = VsockConnection::new(VsockAddr::new(3, 1000), VsockAddr::host(2000));
        assert_eq!(conn.pending_tx(), 0);

        conn.queue_tx(b"hello");
        assert_eq!(conn.pending_tx(), 5);

        conn.queue_tx(b" world");
        assert_eq!(conn.pending_tx(), 11);
    }

    #[test]
    fn test_vsock_connection_forward_count() {
        let mut conn = VsockConnection::new(VsockAddr::new(3, 1000), VsockAddr::host(2000));
        assert_eq!(conn.forward_count(), 0);

        conn.acknowledge(100);
        assert_eq!(conn.forward_count(), 100);

        conn.acknowledge(50);
        assert_eq!(conn.forward_count(), 150);
    }

    // ------------------------------------------------------------------------
    // VirtioVsock Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_virtio_vsock_new() {
        let vsock = VirtioVsock::new(3);
        assert_eq!(vsock.cid(), 3);
        assert!(!vsock.is_ready());
        assert_eq!(vsock.connection_count(), 0);
    }

    #[test]
    fn test_virtio_vsock_activate() {
        let vsock = VirtioVsock::new(3);
        assert!(!vsock.is_ready());

        vsock.activate();
        assert!(vsock.is_ready());

        vsock.deactivate();
        assert!(!vsock.is_ready());
    }

    #[test]
    fn test_virtio_vsock_connect() {
        let vsock = VirtioVsock::new(3);
        vsock.activate();

        let remote = VsockAddr::host(8080);
        vsock.connect(remote).unwrap();

        assert_eq!(vsock.connection_count(), 1);
    }

    #[test]
    fn test_virtio_vsock_connect_not_ready() {
        let vsock = VirtioVsock::new(3);
        let remote = VsockAddr::host(8080);
        assert!(vsock.connect(remote).is_err());
    }

    #[test]
    fn test_virtio_vsock_send() {
        let vsock = VirtioVsock::new(3);
        vsock.activate();

        let data = b"hello";
        let len = vsock.send(VsockAddr::host(8080), data).unwrap();
        assert_eq!(len, 5);
    }

    #[test]
    fn test_virtio_vsock_debug() {
        let vsock = VirtioVsock::new(3);
        let debug = format!("{:?}", vsock);
        assert!(debug.contains("VirtioVsock"));
        assert!(debug.contains("cid"));
    }

    // ------------------------------------------------------------------------
    // BlockRequestHeader Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_block_request_read() {
        let req = BlockRequestHeader::read(100);
        assert_eq!(req.get_type(), BlockRequestType::In);
        assert_eq!(req.sector, 100);
    }

    #[test]
    fn test_block_request_write() {
        let req = BlockRequestHeader::write(200);
        assert_eq!(req.get_type(), BlockRequestType::Out);
        assert_eq!(req.sector, 200);
    }

    #[test]
    fn test_block_request_flush() {
        let req = BlockRequestHeader::flush();
        assert_eq!(req.get_type(), BlockRequestType::Flush);
    }

    // ------------------------------------------------------------------------
    // VirtioBlock Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_virtio_block_new() {
        let config = BlockConfig {
            capacity: 2048, // 1 MiB
            ..Default::default()
        };
        let block = VirtioBlock::new(config);

        assert_eq!(block.capacity(), 2048);
        assert_eq!(block.capacity_bytes(), 2048 * 512);
        assert!(!block.is_ready());
    }

    #[test]
    fn test_virtio_block_with_capacity() {
        let block = VirtioBlock::with_capacity_mib(1);
        assert_eq!(block.capacity_bytes(), 1024 * 1024);
    }

    #[test]
    fn test_virtio_block_activate() {
        let block = VirtioBlock::with_capacity_mib(1);
        assert!(!block.is_ready());

        block.activate();
        assert!(block.is_ready());

        block.deactivate();
        assert!(!block.is_ready());
    }

    #[test]
    fn test_virtio_block_read_write() {
        let block = VirtioBlock::with_capacity_mib(1);
        block.activate();

        // Write data
        let data = vec![0xAB; 512];
        let written = block.write(0, &data).unwrap();
        assert_eq!(written, 512);
        assert_eq!(block.write_count(), 1);

        // Read back
        let mut buffer = vec![0u8; 512];
        let read = block.read(0, &mut buffer).unwrap();
        assert_eq!(read, 512);
        assert_eq!(buffer, data);
        assert_eq!(block.read_count(), 1);
    }

    #[test]
    fn test_virtio_block_read_not_ready() {
        let block = VirtioBlock::with_capacity_mib(1);
        let mut buffer = vec![0u8; 512];
        assert!(block.read(0, &mut buffer).is_err());
    }

    #[test]
    fn test_virtio_block_write_not_ready() {
        let block = VirtioBlock::with_capacity_mib(1);
        let data = vec![0u8; 512];
        assert!(block.write(0, &data).is_err());
    }

    #[test]
    fn test_virtio_block_read_only() {
        let config = BlockConfig {
            capacity: 2048,
            read_only: true,
            ..Default::default()
        };
        let block = VirtioBlock::new(config);
        block.activate();

        assert!(block.is_read_only());

        let data = vec![0u8; 512];
        assert!(block.write(0, &data).is_err());
    }

    #[test]
    fn test_virtio_block_out_of_bounds() {
        let block = VirtioBlock::with_capacity_mib(1);
        block.activate();

        let mut buffer = vec![0u8; 512];
        // Try to read beyond capacity
        assert!(block.read(1000000, &mut buffer).is_err());
    }

    #[test]
    fn test_virtio_block_flush() {
        let block = VirtioBlock::with_capacity_mib(1);
        block.activate();
        assert!(block.flush().is_ok());
    }

    #[test]
    fn test_virtio_block_debug() {
        let block = VirtioBlock::with_capacity_mib(1);
        let debug = format!("{:?}", block);
        assert!(debug.contains("VirtioBlock"));
        assert!(debug.contains("capacity_mib"));
    }

    // ------------------------------------------------------------------------
    // Integration Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_virtio_block_multiple_sectors() {
        let block = VirtioBlock::with_capacity_mib(1);
        block.activate();

        // Write multiple sectors
        for sector in 0..10 {
            let data = vec![sector as u8; 512];
            block.write(sector, &data).unwrap();
        }

        // Read and verify
        for sector in 0..10 {
            let mut buffer = vec![0u8; 512];
            block.read(sector, &mut buffer).unwrap();
            assert!(buffer.iter().all(|&b| b == sector as u8));
        }

        assert_eq!(block.read_count(), 10);
        assert_eq!(block.write_count(), 10);
    }
}
