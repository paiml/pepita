//! `io_uring` kernel interface.
//!
//! This module provides Rust types that match the Linux kernel's `io_uring`
//! interface exactly, as defined in `include/uapi/linux/io_uring.h`.
//!
//! ## ABI Compatibility
//!
//! All structures are `#[repr(C)]` and sized/aligned to match the kernel ABI:
//!
//! - [`IoUringSqe`]: 64 bytes (Submission Queue Entry)
//! - [`IoUringCqe`]: 16 bytes (Completion Queue Entry)
//!
//! ## Key Operations for ublk
//!
//! The ublk driver uses `IORING_OP_URING_CMD` (opcode 46) to pass commands
//! between userspace and kernel. This enables the high-performance data path.
//!
//! ## Example
//!
//! ```rust
//! use pepita::io_uring::{IoUringSqe, IoUringCqe, IORING_OP_URING_CMD};
//!
//! // Verify ABI sizes
//! assert_eq!(core::mem::size_of::<IoUringSqe>(), 64);
//! assert_eq!(core::mem::size_of::<IoUringCqe>(), 16);
//! ```

// ============================================================================
// OPCODES
// ============================================================================

/// No operation
pub const IORING_OP_NOP: u8 = 0;

/// Read from file descriptor
pub const IORING_OP_READV: u8 = 1;

/// Write to file descriptor
pub const IORING_OP_WRITEV: u8 = 2;

/// Fsync file
pub const IORING_OP_FSYNC: u8 = 3;

/// Read from fixed file
pub const IORING_OP_READ_FIXED: u8 = 4;

/// Write to fixed file
pub const IORING_OP_WRITE_FIXED: u8 = 5;

/// Poll file descriptor
pub const IORING_OP_POLL_ADD: u8 = 6;

/// Remove poll request
pub const IORING_OP_POLL_REMOVE: u8 = 7;

/// Sync file range
pub const IORING_OP_SYNC_FILE_RANGE: u8 = 8;

/// Send message
pub const IORING_OP_SENDMSG: u8 = 9;

/// Receive message
pub const IORING_OP_RECVMSG: u8 = 10;

/// Timeout operation
pub const IORING_OP_TIMEOUT: u8 = 11;

/// Remove timeout
pub const IORING_OP_TIMEOUT_REMOVE: u8 = 12;

/// Accept connection
pub const IORING_OP_ACCEPT: u8 = 13;

/// Cancel async operation
pub const IORING_OP_ASYNC_CANCEL: u8 = 14;

/// Link timeout
pub const IORING_OP_LINK_TIMEOUT: u8 = 15;

/// Connect to socket
pub const IORING_OP_CONNECT: u8 = 16;

/// Fallocate file
pub const IORING_OP_FALLOCATE: u8 = 17;

/// Open file
pub const IORING_OP_OPENAT: u8 = 18;

/// Close file
pub const IORING_OP_CLOSE: u8 = 19;

/// Update registered files
pub const IORING_OP_FILES_UPDATE: u8 = 20;

/// Stat file
pub const IORING_OP_STATX: u8 = 21;

/// Read operation
pub const IORING_OP_READ: u8 = 22;

/// Write operation
pub const IORING_OP_WRITE: u8 = 23;

/// Advise on file
pub const IORING_OP_FADVISE: u8 = 24;

/// Advise on memory
pub const IORING_OP_MADVISE: u8 = 25;

/// Send on socket
pub const IORING_OP_SEND: u8 = 26;

/// Receive on socket
pub const IORING_OP_RECV: u8 = 27;

/// Open file (at2 variant)
pub const IORING_OP_OPENAT2: u8 = 28;

/// Epoll control
pub const IORING_OP_EPOLL_CTL: u8 = 29;

/// Splice data
pub const IORING_OP_SPLICE: u8 = 30;

/// Provide buffers
pub const IORING_OP_PROVIDE_BUFFERS: u8 = 31;

/// Remove buffers
pub const IORING_OP_REMOVE_BUFFERS: u8 = 32;

/// Tee operation
pub const IORING_OP_TEE: u8 = 33;

/// Shutdown socket
pub const IORING_OP_SHUTDOWN: u8 = 34;

/// Rename file (at variant)
pub const IORING_OP_RENAMEAT: u8 = 35;

/// Unlink file (at variant)
pub const IORING_OP_UNLINKAT: u8 = 36;

/// Make directory (at variant)
pub const IORING_OP_MKDIRAT: u8 = 37;

/// Create symlink (at variant)
pub const IORING_OP_SYMLINKAT: u8 = 38;

/// Create link (at variant)
pub const IORING_OP_LINKAT: u8 = 39;

/// Message ring
pub const IORING_OP_MSG_RING: u8 = 40;

/// Getxattr
pub const IORING_OP_FGETXATTR: u8 = 41;

/// Setxattr
pub const IORING_OP_FSETXATTR: u8 = 42;

/// Getxattr
pub const IORING_OP_GETXATTR: u8 = 43;

/// Setxattr
pub const IORING_OP_SETXATTR: u8 = 44;

/// Socket operation
pub const IORING_OP_SOCKET: u8 = 45;

/// `URING_CMD` passthrough (used by ublk)
pub const IORING_OP_URING_CMD: u8 = 46;

/// Send to socket with zerocopy
pub const IORING_OP_SEND_ZC: u8 = 47;

/// Sendmsg with zerocopy
pub const IORING_OP_SENDMSG_ZC: u8 = 48;

/// Last opcode (for validation)
pub const IORING_OP_LAST: u8 = 49;

// ============================================================================
// SQE FLAGS
// ============================================================================

/// Use fixed file (registered file slot)
pub const IOSQE_FIXED_FILE: u8 = 1 << 0;

/// Issue after linked ops complete
pub const IOSQE_IO_DRAIN: u8 = 1 << 1;

/// Link with next SQE
pub const IOSQE_IO_LINK: u8 = 1 << 2;

/// Hard link with next SQE
pub const IOSQE_IO_HARDLINK: u8 = 1 << 3;

/// Run in async context
pub const IOSQE_ASYNC: u8 = 1 << 4;

/// Use registered buffer
pub const IOSQE_BUFFER_SELECT: u8 = 1 << 5;

/// Don't generate CQE on completion
pub const IOSQE_CQE_SKIP_SUCCESS: u8 = 1 << 6;

// ============================================================================
// CQE FLAGS
// ============================================================================

/// More data available (multishot)
pub const IORING_CQE_F_MORE: u32 = 1 << 1;

/// Socket notification
pub const IORING_CQE_F_SOCK_NONEMPTY: u32 = 1 << 2;

/// Buffer notification
pub const IORING_CQE_F_NOTIF: u32 = 1 << 3;

// ============================================================================
// SUBMISSION QUEUE ENTRY (64 bytes)
// ============================================================================

/// `io_uring` Submission Queue Entry (SQE).
///
/// This structure is exactly 64 bytes to match the kernel ABI.
/// Used to submit I/O operations to the kernel.
///
/// ## Layout (64 bytes)
///
/// | Offset | Size | Field |
/// |--------|------|-------|
/// | 0 | 1 | opcode |
/// | 1 | 1 | flags |
/// | 2 | 2 | ioprio |
/// | 4 | 4 | fd |
/// | 8 | 8 | off |
/// | 16 | 8 | addr |
/// | 24 | 4 | len |
/// | 28 | 4 | op_flags |
/// | 32 | 8 | user_data |
/// | 40 | 2 | buf_index |
/// | 42 | 2 | personality |
/// | 44 | 4 | splice_fd_in |
/// | 48 | 8 | addr3 |
/// | 56 | 8 | __pad2 |
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoUringSqe {
    /// Operation code (`IORING_OP`_*)
    pub opcode: u8,
    /// SQE flags (IOSQE_*)
    pub flags: u8,
    /// I/O priority
    pub ioprio: u16,
    /// File descriptor
    pub fd: i32,
    /// Offset (operation-dependent)
    pub off: u64,
    /// Address (operation-dependent)
    pub addr: u64,
    /// Length (operation-dependent)
    pub len: u32,
    /// Operation-specific flags
    pub op_flags: u32,
    /// User data (returned in CQE)
    pub user_data: u64,
    /// Buffer index (for buffer selection)
    pub buf_index: u16,
    /// Personality ID
    pub personality: u16,
    /// Splice file descriptor
    pub splice_fd_in: i32,
    /// Third address field
    pub addr3: u64,
    /// Padding to 64 bytes
    pub __pad2: [u64; 1],
}

impl IoUringSqe {
    /// Create a new zeroed SQE.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            opcode: 0,
            flags: 0,
            ioprio: 0,
            fd: -1,
            off: 0,
            addr: 0,
            len: 0,
            op_flags: 0,
            user_data: 0,
            buf_index: 0,
            personality: 0,
            splice_fd_in: 0,
            addr3: 0,
            __pad2: [0],
        }
    }

    /// Create an SQE for NOP operation.
    #[must_use]
    pub const fn nop(user_data: u64) -> Self {
        let mut sqe = Self::new();
        sqe.opcode = IORING_OP_NOP;
        sqe.user_data = user_data;
        sqe
    }

    /// Create an SQE for `URING_CMD` (ublk passthrough).
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor for ublk char device
    /// * `cmd_op` - ublk command opcode
    /// * `user_data` - User data returned in CQE
    #[must_use]
    pub const fn uring_cmd(fd: i32, cmd_op: u32, user_data: u64) -> Self {
        let mut sqe = Self::new();
        sqe.opcode = IORING_OP_URING_CMD;
        sqe.fd = fd;
        sqe.op_flags = cmd_op;
        sqe.user_data = user_data;
        sqe
    }

    /// Create an SQE for read operation.
    #[must_use]
    pub const fn read(fd: i32, buf: u64, len: u32, offset: u64, user_data: u64) -> Self {
        let mut sqe = Self::new();
        sqe.opcode = IORING_OP_READ;
        sqe.fd = fd;
        sqe.addr = buf;
        sqe.len = len;
        sqe.off = offset;
        sqe.user_data = user_data;
        sqe
    }

    /// Create an SQE for write operation.
    #[must_use]
    pub const fn write(fd: i32, buf: u64, len: u32, offset: u64, user_data: u64) -> Self {
        let mut sqe = Self::new();
        sqe.opcode = IORING_OP_WRITE;
        sqe.fd = fd;
        sqe.addr = buf;
        sqe.len = len;
        sqe.off = offset;
        sqe.user_data = user_data;
        sqe
    }

    /// Set the async flag.
    pub fn set_async(&mut self) {
        self.flags |= IOSQE_ASYNC;
    }

    /// Set the link flag.
    pub fn set_link(&mut self) {
        self.flags |= IOSQE_IO_LINK;
    }

    /// Set the fixed file flag.
    pub fn set_fixed_file(&mut self) {
        self.flags |= IOSQE_FIXED_FILE;
    }

    /// Check if this is a `URING_CMD` operation.
    #[must_use]
    pub const fn is_uring_cmd(&self) -> bool {
        self.opcode == IORING_OP_URING_CMD
    }

    /// Check if this is a NOP operation.
    #[must_use]
    pub const fn is_nop(&self) -> bool {
        self.opcode == IORING_OP_NOP
    }
}

impl Default for IoUringSqe {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// COMPLETION QUEUE ENTRY (16 bytes)
// ============================================================================

/// `io_uring` Completion Queue Entry (CQE).
///
/// This structure is exactly 16 bytes to match the kernel ABI.
/// Returned by the kernel when an operation completes.
///
/// ## Layout (16 bytes)
///
/// | Offset | Size | Field |
/// |--------|------|-------|
/// | 0 | 8 | user_data |
/// | 8 | 4 | res |
/// | 12 | 4 | flags |
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoUringCqe {
    /// User data from the corresponding SQE
    pub user_data: u64,
    /// Result of the operation (positive = success, negative = errno)
    pub res: i32,
    /// CQE flags
    pub flags: u32,
}

impl IoUringCqe {
    /// Create a new CQE (typically created by kernel).
    #[must_use]
    pub const fn new(user_data: u64, res: i32, flags: u32) -> Self {
        Self { user_data, res, flags }
    }

    /// Check if the operation succeeded.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.res >= 0
    }

    /// Check if the operation failed.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        self.res < 0
    }

    /// Get the error code (as positive errno).
    ///
    /// Returns 0 if not an error.
    #[must_use]
    pub const fn errno(&self) -> i32 {
        if self.res < 0 {
            -self.res
        } else {
            0
        }
    }

    /// Get the success result.
    ///
    /// Returns `None` if operation failed.
    #[must_use]
    pub const fn result(&self) -> Option<u32> {
        if self.res >= 0 {
            Some(self.res as u32)
        } else {
            None
        }
    }

    /// Check if more data is available (multishot).
    #[must_use]
    pub const fn has_more(&self) -> bool {
        (self.flags & IORING_CQE_F_MORE) != 0
    }
}

impl Default for IoUringCqe {
    fn default() -> Self {
        Self::new(0, 0, 0)
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
    // ABI Size Tests (Falsification Checklist Points 4-5)
    // ------------------------------------------------------------------------

    #[test]
    fn abi_io_uring_sqe_size() {
        assert_eq!(size_of::<IoUringSqe>(), 64, "IoUringSqe must be exactly 64 bytes");
    }

    #[test]
    fn abi_io_uring_cqe_size() {
        assert_eq!(size_of::<IoUringCqe>(), 16, "IoUringCqe must be exactly 16 bytes");
    }

    // ------------------------------------------------------------------------
    // ABI Alignment Tests
    // ------------------------------------------------------------------------

    #[test]
    fn abi_io_uring_sqe_alignment() {
        assert!(align_of::<IoUringSqe>() >= 8, "IoUringSqe must have at least 8-byte alignment");
    }

    #[test]
    fn abi_io_uring_cqe_alignment() {
        assert!(align_of::<IoUringCqe>() >= 8, "IoUringCqe must have at least 8-byte alignment");
    }

    // ------------------------------------------------------------------------
    // ABI Offset Tests
    // ------------------------------------------------------------------------

    #[test]
    fn abi_sqe_offsets() {
        let sqe = IoUringSqe::new();
        let base = &sqe as *const _ as usize;

        assert_eq!(&sqe.opcode as *const _ as usize - base, 0, "opcode at offset 0");
        assert_eq!(&sqe.flags as *const _ as usize - base, 1, "flags at offset 1");
        assert_eq!(&sqe.ioprio as *const _ as usize - base, 2, "ioprio at offset 2");
        assert_eq!(&sqe.fd as *const _ as usize - base, 4, "fd at offset 4");
        assert_eq!(&sqe.off as *const _ as usize - base, 8, "off at offset 8");
        assert_eq!(&sqe.addr as *const _ as usize - base, 16, "addr at offset 16");
        assert_eq!(&sqe.len as *const _ as usize - base, 24, "len at offset 24");
        assert_eq!(&sqe.op_flags as *const _ as usize - base, 28, "op_flags at offset 28");
        assert_eq!(&sqe.user_data as *const _ as usize - base, 32, "user_data at offset 32");
        assert_eq!(&sqe.buf_index as *const _ as usize - base, 40, "buf_index at offset 40");
        assert_eq!(&sqe.personality as *const _ as usize - base, 42, "personality at offset 42");
        assert_eq!(&sqe.splice_fd_in as *const _ as usize - base, 44, "splice_fd_in at offset 44");
        assert_eq!(&sqe.addr3 as *const _ as usize - base, 48, "addr3 at offset 48");
    }

    #[test]
    fn abi_cqe_offsets() {
        let cqe = IoUringCqe::default();
        let base = &cqe as *const _ as usize;

        assert_eq!(&cqe.user_data as *const _ as usize - base, 0, "user_data at offset 0");
        assert_eq!(&cqe.res as *const _ as usize - base, 8, "res at offset 8");
        assert_eq!(&cqe.flags as *const _ as usize - base, 12, "flags at offset 12");
    }

    // ------------------------------------------------------------------------
    // Opcode Tests
    // ------------------------------------------------------------------------

    #[test]
    fn abi_uring_cmd_opcode() {
        assert_eq!(IORING_OP_URING_CMD, 46, "IORING_OP_URING_CMD must be 46 for ublk");
    }

    #[test]
    fn abi_opcodes_are_distinct() {
        let opcodes: [u8; 10] = [
            IORING_OP_NOP,
            IORING_OP_READV,
            IORING_OP_WRITEV,
            IORING_OP_READ,
            IORING_OP_WRITE,
            IORING_OP_URING_CMD,
            IORING_OP_FSYNC,
            IORING_OP_CLOSE,
            IORING_OP_SEND,
            IORING_OP_RECV,
        ];

        for i in 0..opcodes.len() {
            for j in (i + 1)..opcodes.len() {
                assert_ne!(opcodes[i], opcodes[j], "opcodes must be distinct");
            }
        }
    }

    #[test]
    fn abi_opcodes_within_range() {
        let opcodes = [IORING_OP_NOP, IORING_OP_READ, IORING_OP_WRITE, IORING_OP_URING_CMD];

        for op in opcodes {
            assert!(op < IORING_OP_LAST, "opcode {} must be < LAST", op);
        }
    }

    // ------------------------------------------------------------------------
    // SQE Flag Tests
    // ------------------------------------------------------------------------

    #[test]
    fn abi_sqe_flags_are_powers_of_two() {
        let flags = [
            IOSQE_FIXED_FILE,
            IOSQE_IO_DRAIN,
            IOSQE_IO_LINK,
            IOSQE_IO_HARDLINK,
            IOSQE_ASYNC,
            IOSQE_BUFFER_SELECT,
            IOSQE_CQE_SKIP_SUCCESS,
        ];

        for flag in flags {
            assert!(flag.is_power_of_two(), "flag 0x{:x} must be power of two", flag);
        }
    }

    // ------------------------------------------------------------------------
    // SQE Functional Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_sqe_new() {
        let sqe = IoUringSqe::new();
        assert_eq!(sqe.opcode, 0);
        assert_eq!(sqe.fd, -1);
        assert_eq!(sqe.user_data, 0);
    }

    #[test]
    fn test_sqe_nop() {
        let sqe = IoUringSqe::nop(12345);
        assert!(sqe.is_nop());
        assert_eq!(sqe.user_data, 12345);
    }

    #[test]
    fn test_sqe_uring_cmd() {
        let sqe = IoUringSqe::uring_cmd(10, 0x1234, 42);
        assert!(sqe.is_uring_cmd());
        assert_eq!(sqe.fd, 10);
        assert_eq!(sqe.op_flags, 0x1234);
        assert_eq!(sqe.user_data, 42);
    }

    #[test]
    fn test_sqe_read() {
        let sqe = IoUringSqe::read(5, 0x1000, 4096, 0, 100);
        assert_eq!(sqe.opcode, IORING_OP_READ);
        assert_eq!(sqe.fd, 5);
        assert_eq!(sqe.addr, 0x1000);
        assert_eq!(sqe.len, 4096);
        assert_eq!(sqe.off, 0);
        assert_eq!(sqe.user_data, 100);
    }

    #[test]
    fn test_sqe_write() {
        let sqe = IoUringSqe::write(5, 0x2000, 8192, 512, 200);
        assert_eq!(sqe.opcode, IORING_OP_WRITE);
        assert_eq!(sqe.fd, 5);
        assert_eq!(sqe.addr, 0x2000);
        assert_eq!(sqe.len, 8192);
        assert_eq!(sqe.off, 512);
        assert_eq!(sqe.user_data, 200);
    }

    #[test]
    fn test_sqe_flags() {
        let mut sqe = IoUringSqe::new();
        assert_eq!(sqe.flags, 0);

        sqe.set_async();
        assert_eq!(sqe.flags & IOSQE_ASYNC, IOSQE_ASYNC);

        sqe.set_link();
        assert_eq!(sqe.flags & IOSQE_IO_LINK, IOSQE_IO_LINK);

        sqe.set_fixed_file();
        assert_eq!(sqe.flags & IOSQE_FIXED_FILE, IOSQE_FIXED_FILE);
    }

    // ------------------------------------------------------------------------
    // CQE Functional Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_cqe_success() {
        let cqe = IoUringCqe::new(42, 4096, 0);
        assert!(cqe.is_success());
        assert!(!cqe.is_error());
        assert_eq!(cqe.result(), Some(4096));
        assert_eq!(cqe.errno(), 0);
    }

    #[test]
    fn test_cqe_error() {
        let cqe = IoUringCqe::new(42, -5, 0); // EIO
        assert!(!cqe.is_success());
        assert!(cqe.is_error());
        assert_eq!(cqe.result(), None);
        assert_eq!(cqe.errno(), 5);
    }

    #[test]
    fn test_cqe_zero_result() {
        let cqe = IoUringCqe::new(0, 0, 0);
        assert!(cqe.is_success());
        assert_eq!(cqe.result(), Some(0));
    }

    #[test]
    fn test_cqe_has_more() {
        let cqe_no_more = IoUringCqe::new(0, 0, 0);
        assert!(!cqe_no_more.has_more());

        let cqe_more = IoUringCqe::new(0, 0, IORING_CQE_F_MORE);
        assert!(cqe_more.has_more());
    }

    // ------------------------------------------------------------------------
    // Default/Clone/Copy Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_sqe_default() {
        let sqe = IoUringSqe::default();
        assert_eq!(sqe.opcode, 0);
        assert_eq!(sqe.fd, -1);
    }

    #[test]
    fn test_cqe_default() {
        let cqe = IoUringCqe::default();
        assert_eq!(cqe.user_data, 0);
        assert_eq!(cqe.res, 0);
        assert_eq!(cqe.flags, 0);
    }

    #[test]
    fn test_sqe_copy() {
        let sqe1 = IoUringSqe::nop(999);
        let sqe2 = sqe1;
        assert_eq!(sqe1, sqe2);
    }

    #[test]
    fn test_cqe_copy() {
        let cqe1 = IoUringCqe::new(1, 2, 3);
        let cqe2 = cqe1;
        assert_eq!(cqe1, cqe2);
    }
}
