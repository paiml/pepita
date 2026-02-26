//! ublk (Userspace Block Device) kernel interface.
//!
//! This module provides Rust types that match the Linux kernel's ublk interface
//! exactly, as defined in `include/uapi/linux/ublk_cmd.h`.
//!
//! ## ABI Compatibility
//!
//! All structures are `#[repr(C)]` and sized/aligned to match the kernel ABI:
//!
//! - [`UblkCtrlCmd`]: 32 bytes, 8-byte aligned
//! - [`UblkIoDesc`]: 24 bytes, 8-byte aligned
//! - [`UblkIoCmd`]: 16 bytes, 8-byte aligned
//!
//! ## Usage
//!
//! ```rust
//! use pepita::ublk::{UblkCtrlCmd, UblkIoDesc, UblkIoCmd};
//! use pepita::ublk::{UBLK_U_CMD_ADD_DEV, UBLK_U_IO_FETCH_REQ};
//!
//! // Create a control command
//! let ctrl = UblkCtrlCmd::new(0); // Device ID 0
//! assert_eq!(ctrl.dev_id(), 0);
//!
//! // Verify ioctl encodings
//! assert_eq!(UBLK_U_CMD_ADD_DEV, 0xc020_7504);
//! ```

// ============================================================================
// IOCTL COMMAND OPCODES
// ============================================================================

/// ioctl: Add a new ublk device
pub const UBLK_U_CMD_ADD_DEV: u32 = 0xc020_7504;

/// ioctl: Delete a ublk device
pub const UBLK_U_CMD_DEL_DEV: u32 = 0xc020_7505;

/// ioctl: Start a ublk device
pub const UBLK_U_CMD_START_DEV: u32 = 0xc020_7506;

/// ioctl: Stop a ublk device
pub const UBLK_U_CMD_STOP_DEV: u32 = 0xc020_7507;

/// ioctl: Set device parameters
pub const UBLK_U_CMD_SET_PARAMS: u32 = 0xc020_7508;

/// ioctl: Get device parameters
pub const UBLK_U_CMD_GET_PARAMS: u32 = 0x8020_7509;

/// ioctl: Get queue affinity
pub const UBLK_U_CMD_GET_QUEUE_AFFINITY: u32 = 0x8020_750a;

/// ioctl: Get device info
pub const UBLK_U_CMD_GET_DEV_INFO: u32 = 0x8020_750b;

// ============================================================================
// IO COMMAND OPCODES (via io_uring URING_CMD)
// ============================================================================

/// I/O command: Fetch request from kernel
pub const UBLK_U_IO_FETCH_REQ: u32 = 0xc010_7520;

/// I/O command: Commit result and fetch next request
pub const UBLK_U_IO_COMMIT_AND_FETCH_REQ: u32 = 0xc010_7521;

/// I/O command: Need to get data (for writes)
pub const UBLK_U_IO_NEED_GET_DATA: u32 = 0xc010_7522;

// ============================================================================
// DEVICE CAPABILITY FLAGS
// ============================================================================

/// Device supports zero-copy I/O
pub const UBLK_F_SUPPORT_ZERO_COPY: u64 = 1 << 0;

/// Device uses `URING_CMD` with ioctl encoding
pub const UBLK_F_URING_CMD_COMP_IN_TASK: u64 = 1 << 1;

/// Device needs to get data for writes
pub const UBLK_F_NEED_GET_DATA: u64 = 1 << 2;

/// Device uses user copy (vs kernel copy)
pub const UBLK_F_USER_COPY: u64 = 1 << 7;

/// Device uses ioctl-encoded commands
pub const UBLK_F_CMD_IOCTL_ENCODE: u64 = 1 << 6;

/// Device supports unprivileged operation
pub const UBLK_F_UNPRIVILEGED_DEV: u64 = 1 << 5;

// ============================================================================
// I/O OPERATION FLAGS
// ============================================================================

/// Operation: Read from device
pub const UBLK_IO_OP_READ: u32 = 0;

/// Operation: Write to device
pub const UBLK_IO_OP_WRITE: u32 = 1;

/// Operation: Flush device buffers
pub const UBLK_IO_OP_FLUSH: u32 = 2;

/// Operation: Discard sectors
pub const UBLK_IO_OP_DISCARD: u32 = 3;

/// Operation: Write zeroes
pub const UBLK_IO_OP_WRITE_ZEROES: u32 = 4;

/// Flag: Force unit access
pub const UBLK_IO_F_FUA: u32 = 1 << 8;

// ============================================================================
// CONTROL COMMAND STRUCTURE (32 bytes)
// ============================================================================

/// ublk control command structure.
///
/// Used for device management operations (add, delete, start, stop, params).
/// Passed to `/dev/ublk-control` via `io_uring` `URING_CMD`.
///
/// ## Layout (32 bytes total)
///
/// | Offset | Size | Field |
/// |--------|------|-------|
/// | 0 | 4 | dev_id |
/// | 4 | 2 | queue_id |
/// | 6 | 2 | len |
/// | 8 | 8 | addr |
/// | 16 | 8 | data[0] |
/// | 24 | 2 | dev_path_len |
/// | 26 | 2 | pad |
/// | 28 | 4 | reserved |
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UblkCtrlCmd {
    dev_id: u32,
    queue_id: u16,
    len: u16,
    addr: u64,
    data: [u64; 1],
    dev_path_len: u16,
    pad: u16,
    reserved: u32,
}

impl UblkCtrlCmd {
    /// Create a new control command for a device.
    ///
    /// # Arguments
    ///
    /// * `dev_id` - Device identifier
    #[must_use]
    pub const fn new(dev_id: u32) -> Self {
        Self {
            dev_id,
            queue_id: 0,
            len: 0,
            addr: 0,
            data: [0],
            dev_path_len: 0,
            pad: 0,
            reserved: 0,
        }
    }

    /// Create a control command for a specific queue.
    #[must_use]
    pub const fn with_queue(dev_id: u32, queue_id: u16) -> Self {
        Self { dev_id, queue_id, len: 0, addr: 0, data: [0], dev_path_len: 0, pad: 0, reserved: 0 }
    }

    /// Get the device ID.
    #[must_use]
    pub const fn dev_id(&self) -> u32 {
        self.dev_id
    }

    /// Get the queue ID.
    #[must_use]
    pub const fn queue_id(&self) -> u16 {
        self.queue_id
    }

    /// Get the data length.
    #[must_use]
    pub const fn len(&self) -> u16 {
        self.len
    }

    /// Get the data address.
    #[must_use]
    pub const fn addr(&self) -> u64 {
        self.addr
    }

    /// Set the data address and length.
    pub fn set_data(&mut self, addr: u64, len: u16) {
        self.addr = addr;
        self.len = len;
    }

    /// Get the extra data field.
    #[must_use]
    pub const fn data(&self) -> u64 {
        self.data[0]
    }

    /// Set the extra data field.
    pub fn set_extra_data(&mut self, data: u64) {
        self.data[0] = data;
    }
}

impl Default for UblkCtrlCmd {
    fn default() -> Self {
        Self::new(0)
    }
}

// ============================================================================
// I/O DESCRIPTOR STRUCTURE (24 bytes)
// ============================================================================

/// ublk I/O descriptor structure.
///
/// Describes a single I/O request from the kernel to userspace.
/// These are stored in a memory-mapped array shared with the kernel.
///
/// ## Layout (24 bytes total)
///
/// | Offset | Size | Field |
/// |--------|------|-------|
/// | 0 | 4 | `op_flags` |
/// | 4 | 4 | `nr_sectors` |
/// | 8 | 8 | `start_sector` |
/// | 16 | 8 | addr |
///
/// ## Zero-Copy Design
///
/// The `addr` field contains a userspace buffer address for data transfer.
/// With `UBLK_F_USER_COPY`, the server provides the buffer; otherwise,
/// the kernel provides it via `UBLK_F_SUPPORT_ZERO_COPY`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UblkIoDesc {
    op_flags: u32,
    nr_sectors: u32,
    start_sector: u64,
    addr: u64,
}

impl UblkIoDesc {
    /// Create a new I/O descriptor.
    ///
    /// # Arguments
    ///
    /// * `op` - Operation type (read, write, etc.)
    /// * `start_sector` - Starting sector (LBA)
    /// * `nr_sectors` - Number of sectors
    #[must_use]
    pub const fn new(op: u32, start_sector: u64, nr_sectors: u32) -> Self {
        Self { op_flags: op, nr_sectors, start_sector, addr: 0 }
    }

    /// Get the operation type (lower 8 bits of `op_flags`).
    #[must_use]
    pub const fn op(&self) -> u32 {
        self.op_flags & 0xFF
    }

    /// Get the operation flags (upper bits).
    #[must_use]
    pub const fn flags(&self) -> u32 {
        self.op_flags & !0xFF
    }

    /// Get the full `op_flags` field.
    #[must_use]
    pub const fn op_flags(&self) -> u32 {
        self.op_flags
    }

    /// Check if this is a read operation.
    #[must_use]
    pub const fn is_read(&self) -> bool {
        self.op() == UBLK_IO_OP_READ
    }

    /// Check if this is a write operation.
    #[must_use]
    pub const fn is_write(&self) -> bool {
        self.op() == UBLK_IO_OP_WRITE
    }

    /// Check if this is a flush operation.
    #[must_use]
    pub const fn is_flush(&self) -> bool {
        self.op() == UBLK_IO_OP_FLUSH
    }

    /// Check if FUA (Force Unit Access) is set.
    #[must_use]
    pub const fn is_fua(&self) -> bool {
        (self.op_flags & UBLK_IO_F_FUA) != 0
    }

    /// Get the number of sectors.
    #[must_use]
    pub const fn nr_sectors(&self) -> u32 {
        self.nr_sectors
    }

    /// Get the starting sector (LBA).
    #[must_use]
    pub const fn start_sector(&self) -> u64 {
        self.start_sector
    }

    /// Get the buffer address.
    #[must_use]
    pub const fn addr(&self) -> u64 {
        self.addr
    }

    /// Set the buffer address.
    pub fn set_addr(&mut self, addr: u64) {
        self.addr = addr;
    }

    /// Calculate the byte offset from sector.
    #[must_use]
    pub const fn byte_offset(&self) -> u64 {
        self.start_sector * 512
    }

    /// Calculate the byte length from sectors.
    #[must_use]
    pub const fn byte_len(&self) -> u64 {
        self.nr_sectors as u64 * 512
    }
}

impl Default for UblkIoDesc {
    fn default() -> Self {
        Self::new(UBLK_IO_OP_READ, 0, 0)
    }
}

// ============================================================================
// I/O COMMAND STRUCTURE (16 bytes)
// ============================================================================

/// ublk I/O command structure.
///
/// Used to communicate I/O results back to the kernel via `io_uring`.
/// Embedded in the `io_uring` SQE for `URING_CMD` operations.
///
/// ## Layout (16 bytes total)
///
/// | Offset | Size | Field |
/// |--------|------|-------|
/// | 0 | 2 | q_id |
/// | 2 | 2 | tag |
/// | 4 | 4 | result |
/// | 8 | 8 | addr |
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UblkIoCmd {
    q_id: u16,
    tag: u16,
    result: i32,
    addr: u64,
}

impl UblkIoCmd {
    /// Create a new I/O command.
    ///
    /// # Arguments
    ///
    /// * `q_id` - Queue identifier
    /// * `tag` - Request tag (matches blk-mq tag)
    #[must_use]
    pub const fn new(q_id: u16, tag: u16) -> Self {
        Self { q_id, tag, result: 0, addr: 0 }
    }

    /// Create a completed I/O command with result.
    #[must_use]
    pub const fn completed(q_id: u16, tag: u16, result: i32) -> Self {
        Self { q_id, tag, result, addr: 0 }
    }

    /// Get the queue ID.
    #[must_use]
    pub const fn q_id(&self) -> u16 {
        self.q_id
    }

    /// Get the request tag.
    #[must_use]
    pub const fn tag(&self) -> u16 {
        self.tag
    }

    /// Get the result code.
    #[must_use]
    pub const fn result(&self) -> i32 {
        self.result
    }

    /// Set the result code.
    pub fn set_result(&mut self, result: i32) {
        self.result = result;
    }

    /// Get the buffer address.
    #[must_use]
    pub const fn addr(&self) -> u64 {
        self.addr
    }

    /// Set the buffer address.
    pub fn set_addr(&mut self, addr: u64) {
        self.addr = addr;
    }

    /// Check if the command completed successfully.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.result >= 0
    }
}

impl Default for UblkIoCmd {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

// ============================================================================
// TESTS (EXTREME TDD - ABI COMPATIBILITY)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    // ------------------------------------------------------------------------
    // ABI Size Tests (Falsification Checklist Points 1-5)
    // ------------------------------------------------------------------------

    #[test]
    fn abi_ublk_ctrl_cmd_size() {
        assert_eq!(size_of::<UblkCtrlCmd>(), 32, "UblkCtrlCmd must be exactly 32 bytes");
    }

    #[test]
    fn abi_ublk_io_desc_size() {
        assert_eq!(size_of::<UblkIoDesc>(), 24, "UblkIoDesc must be exactly 24 bytes");
    }

    #[test]
    fn abi_ublk_io_cmd_size() {
        assert_eq!(size_of::<UblkIoCmd>(), 16, "UblkIoCmd must be exactly 16 bytes");
    }

    // ------------------------------------------------------------------------
    // ABI Alignment Tests (Falsification Checklist Point 11)
    // ------------------------------------------------------------------------

    #[test]
    fn abi_ublk_ctrl_cmd_alignment() {
        assert!(align_of::<UblkCtrlCmd>() >= 4, "UblkCtrlCmd must have at least 4-byte alignment");
    }

    #[test]
    fn abi_ublk_io_desc_alignment() {
        assert!(align_of::<UblkIoDesc>() >= 4, "UblkIoDesc must have at least 4-byte alignment");
    }

    #[test]
    fn abi_ublk_io_cmd_alignment() {
        assert!(align_of::<UblkIoCmd>() >= 2, "UblkIoCmd must have at least 2-byte alignment");
    }

    // ------------------------------------------------------------------------
    // ABI Offset Tests (Falsification Checklist Points 6-10)
    // ------------------------------------------------------------------------

    #[test]
    fn abi_ublk_ctrl_cmd_offsets() {
        // Create a zeroed struct and check field offsets
        let cmd = UblkCtrlCmd::default();
        let base = &cmd as *const _ as usize;

        let dev_id_offset = &cmd.dev_id as *const _ as usize - base;
        let queue_id_offset = &cmd.queue_id as *const _ as usize - base;
        let len_offset = &cmd.len as *const _ as usize - base;
        let addr_offset = &cmd.addr as *const _ as usize - base;

        assert_eq!(dev_id_offset, 0, "dev_id must be at offset 0");
        assert_eq!(queue_id_offset, 4, "queue_id must be at offset 4");
        assert_eq!(len_offset, 6, "len must be at offset 6");
        assert_eq!(addr_offset, 8, "addr must be at offset 8");
    }

    #[test]
    fn abi_ublk_io_desc_offsets() {
        let desc = UblkIoDesc::default();
        let base = &desc as *const _ as usize;

        let op_flags_offset = &desc.op_flags as *const _ as usize - base;
        let nr_sectors_offset = &desc.nr_sectors as *const _ as usize - base;
        let start_sector_offset = &desc.start_sector as *const _ as usize - base;
        let addr_offset = &desc.addr as *const _ as usize - base;

        assert_eq!(op_flags_offset, 0, "op_flags must be at offset 0");
        assert_eq!(nr_sectors_offset, 4, "nr_sectors must be at offset 4");
        assert_eq!(start_sector_offset, 8, "start_sector must be at offset 8");
        assert_eq!(addr_offset, 16, "addr must be at offset 16");
    }

    #[test]
    fn abi_ublk_io_cmd_offsets() {
        let cmd = UblkIoCmd::default();
        let base = &cmd as *const _ as usize;

        let q_id_offset = &cmd.q_id as *const _ as usize - base;
        let tag_offset = &cmd.tag as *const _ as usize - base;
        let result_offset = &cmd.result as *const _ as usize - base;
        let addr_offset = &cmd.addr as *const _ as usize - base;

        assert_eq!(q_id_offset, 0, "q_id must be at offset 0");
        assert_eq!(tag_offset, 2, "tag must be at offset 2");
        assert_eq!(result_offset, 4, "result must be at offset 4");
        assert_eq!(addr_offset, 8, "addr must be at offset 8");
    }

    // ------------------------------------------------------------------------
    // ioctl Encoding Tests (Falsification Checklist Points 12-13)
    // ------------------------------------------------------------------------

    #[test]
    fn abi_ioctl_cmd_add_dev() {
        assert_eq!(UBLK_U_CMD_ADD_DEV, 0xc020_7504, "UBLK_U_CMD_ADD_DEV must match kernel");
    }

    #[test]
    fn abi_ioctl_cmd_del_dev() {
        assert_eq!(UBLK_U_CMD_DEL_DEV, 0xc020_7505, "UBLK_U_CMD_DEL_DEV must match kernel");
    }

    #[test]
    fn abi_ioctl_cmd_start_dev() {
        assert_eq!(UBLK_U_CMD_START_DEV, 0xc020_7506, "UBLK_U_CMD_START_DEV must match kernel");
    }

    #[test]
    fn abi_ioctl_cmd_stop_dev() {
        assert_eq!(UBLK_U_CMD_STOP_DEV, 0xc020_7507, "UBLK_U_CMD_STOP_DEV must match kernel");
    }

    #[test]
    fn abi_ioctl_cmd_set_params() {
        assert_eq!(UBLK_U_CMD_SET_PARAMS, 0xc020_7508, "UBLK_U_CMD_SET_PARAMS must match kernel");
    }

    #[test]
    fn abi_ioctl_cmd_get_params() {
        assert_eq!(UBLK_U_CMD_GET_PARAMS, 0x8020_7509, "UBLK_U_CMD_GET_PARAMS must match kernel");
    }

    #[test]
    fn abi_ioctl_io_fetch_req() {
        assert_eq!(UBLK_U_IO_FETCH_REQ, 0xc010_7520, "UBLK_U_IO_FETCH_REQ must match kernel");
    }

    #[test]
    fn abi_ioctl_io_commit_and_fetch() {
        assert_eq!(
            UBLK_U_IO_COMMIT_AND_FETCH_REQ, 0xc010_7521,
            "UBLK_U_IO_COMMIT_AND_FETCH_REQ must match kernel"
        );
    }

    // ------------------------------------------------------------------------
    // Flag Tests (Falsification Checklist Point 20)
    // ------------------------------------------------------------------------

    #[test]
    fn abi_flags_are_powers_of_two() {
        let flags = [
            UBLK_F_SUPPORT_ZERO_COPY,
            UBLK_F_URING_CMD_COMP_IN_TASK,
            UBLK_F_NEED_GET_DATA,
            UBLK_F_USER_COPY,
            UBLK_F_CMD_IOCTL_ENCODE,
            UBLK_F_UNPRIVILEGED_DEV,
        ];

        for flag in flags {
            assert!(flag.is_power_of_two(), "flag 0x{:x} must be power of two", flag);
        }
    }

    #[test]
    fn abi_io_op_flags_are_distinct() {
        let ops = [
            UBLK_IO_OP_READ,
            UBLK_IO_OP_WRITE,
            UBLK_IO_OP_FLUSH,
            UBLK_IO_OP_DISCARD,
            UBLK_IO_OP_WRITE_ZEROES,
        ];

        for i in 0..ops.len() {
            for j in (i + 1)..ops.len() {
                assert_ne!(ops[i], ops[j], "ops {:?} and {:?} must be distinct", ops[i], ops[j]);
            }
        }
    }

    // ------------------------------------------------------------------------
    // Functional Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_ctrl_cmd_new() {
        let cmd = UblkCtrlCmd::new(42);
        assert_eq!(cmd.dev_id(), 42);
        assert_eq!(cmd.queue_id(), 0);
        assert_eq!(cmd.len(), 0);
        assert_eq!(cmd.addr(), 0);
    }

    #[test]
    fn test_ctrl_cmd_with_queue() {
        let cmd = UblkCtrlCmd::with_queue(10, 5);
        assert_eq!(cmd.dev_id(), 10);
        assert_eq!(cmd.queue_id(), 5);
    }

    #[test]
    fn test_ctrl_cmd_set_data() {
        let mut cmd = UblkCtrlCmd::new(0);
        cmd.set_data(0x1234_5678, 100);
        assert_eq!(cmd.addr(), 0x1234_5678);
        assert_eq!(cmd.len(), 100);
    }

    #[test]
    fn test_io_desc_new() {
        let desc = UblkIoDesc::new(UBLK_IO_OP_WRITE, 1000, 8);
        assert_eq!(desc.op(), UBLK_IO_OP_WRITE);
        assert_eq!(desc.start_sector(), 1000);
        assert_eq!(desc.nr_sectors(), 8);
    }

    #[test]
    fn test_io_desc_op_detection() {
        let read = UblkIoDesc::new(UBLK_IO_OP_READ, 0, 0);
        assert!(read.is_read());
        assert!(!read.is_write());

        let write = UblkIoDesc::new(UBLK_IO_OP_WRITE, 0, 0);
        assert!(write.is_write());
        assert!(!write.is_read());

        let flush = UblkIoDesc::new(UBLK_IO_OP_FLUSH, 0, 0);
        assert!(flush.is_flush());
    }

    #[test]
    fn test_io_desc_fua_flag() {
        let normal = UblkIoDesc::new(UBLK_IO_OP_WRITE, 0, 0);
        assert!(!normal.is_fua());

        let fua = UblkIoDesc::new(UBLK_IO_OP_WRITE | UBLK_IO_F_FUA, 0, 0);
        assert!(fua.is_fua());
    }

    #[test]
    fn test_io_desc_byte_calculations() {
        let desc = UblkIoDesc::new(UBLK_IO_OP_READ, 100, 8);
        assert_eq!(desc.byte_offset(), 100 * 512);
        assert_eq!(desc.byte_len(), 8 * 512);
    }

    #[test]
    fn test_io_cmd_new() {
        let cmd = UblkIoCmd::new(3, 42);
        assert_eq!(cmd.q_id(), 3);
        assert_eq!(cmd.tag(), 42);
        assert_eq!(cmd.result(), 0);
    }

    #[test]
    fn test_io_cmd_completed() {
        let success = UblkIoCmd::completed(0, 1, 4096);
        assert!(success.is_success());
        assert_eq!(success.result(), 4096);

        let error = UblkIoCmd::completed(0, 1, -5); // EIO
        assert!(!error.is_success());
        assert_eq!(error.result(), -5);
    }

    #[test]
    fn test_io_cmd_set_result() {
        let mut cmd = UblkIoCmd::new(0, 0);
        cmd.set_result(1024);
        assert_eq!(cmd.result(), 1024);
    }

    // ------------------------------------------------------------------------
    // Default Trait Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_defaults() {
        let ctrl = UblkCtrlCmd::default();
        assert_eq!(ctrl.dev_id(), 0);

        let desc = UblkIoDesc::default();
        assert!(desc.is_read());

        let cmd = UblkIoCmd::default();
        assert_eq!(cmd.q_id(), 0);
        assert_eq!(cmd.tag(), 0);
    }

    // ------------------------------------------------------------------------
    // Clone/Copy Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_copy_semantics() {
        let cmd1 = UblkIoCmd::new(1, 2);
        let cmd2 = cmd1; // Copy
        assert_eq!(cmd1, cmd2);
    }

    #[test]
    fn test_clone() {
        let desc = UblkIoDesc::new(UBLK_IO_OP_WRITE, 500, 16);
        #[allow(clippy::clone_on_copy)]
        let cloned = desc.clone();
        assert_eq!(desc, cloned);
    }
}
