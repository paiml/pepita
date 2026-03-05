//! Error types for Pepita kernel interfaces.
//!
//! This module provides error handling following the Iron Lotus principle
//! of explicit error handling - no panics in normal operation paths.

use core::fmt;

/// Result type alias for Pepita operations.
pub type Result<T> = core::result::Result<T, KernelError>;

/// Kernel error enumeration.
///
/// Represents all possible error conditions in Pepita kernel operations.
/// Each variant maps to a specific failure mode with clear semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KernelError {
    // ========================================================================
    // Memory Errors
    // ========================================================================
    /// Out of memory - allocation failed
    OutOfMemory,

    /// Invalid memory address provided
    InvalidAddress,

    /// Address alignment violation
    AlignmentError,

    /// Memory region overlap detected
    OverlappingRegion,

    // ========================================================================
    // I/O Errors
    // ========================================================================
    /// I/O operation timed out
    IoTimeout,

    /// Device not ready for operation
    DeviceNotReady,

    /// Invalid I/O request parameters
    InvalidRequest,

    /// I/O operation was cancelled
    Cancelled,

    // ========================================================================
    // ublk Errors
    // ========================================================================
    /// ublk request queue is full
    UblkQueueFull,

    /// Invalid ublk tag
    UblkInvalidTag,

    /// ublk device is busy
    UblkDeviceBusy,

    /// ublk device not found
    UblkDeviceNotFound,

    /// Invalid ublk device ID
    UblkInvalidDeviceId,

    /// ublk operation not permitted
    UblkNotPermitted,

    // ========================================================================
    // io_uring Errors
    // ========================================================================
    /// io_uring submission queue full
    IoUringSubmitFull,

    /// io_uring completion queue overflow
    IoUringCqOverflow,

    /// Invalid io_uring opcode
    IoUringInvalidOpcode,

    // ========================================================================
    // Block Layer Errors
    // ========================================================================
    /// Block device error
    BlockError,

    /// No tags available for request
    NoTagsAvailable,

    /// Invalid queue ID
    InvalidQueueId,

    // ========================================================================
    // Generic Errors
    // ========================================================================
    /// Operation not supported
    NotSupported,

    /// Invalid argument
    InvalidArgument,

    /// Resource temporarily unavailable
    WouldBlock,

    /// Operation interrupted
    Interrupted,

    /// Resource is busy (e.g., lock contention)
    ResourceBusy,
}

impl KernelError {
    /// Convert error to POSIX-compatible errno value.
    ///
    /// Returns negative errno values as per Linux kernel convention.
    #[must_use]
    pub const fn to_errno(self) -> i32 {
        match self {
            Self::OutOfMemory => -12,          // ENOMEM
            Self::InvalidAddress => -14,       // EFAULT
            Self::AlignmentError => -22,       // EINVAL
            Self::OverlappingRegion => -22,    // EINVAL
            Self::IoTimeout => -110,           // ETIMEDOUT
            Self::DeviceNotReady => -19,       // ENODEV
            Self::InvalidRequest => -22,       // EINVAL
            Self::Cancelled => -125,           // ECANCELED
            Self::UblkQueueFull => -11,        // EAGAIN
            Self::UblkInvalidTag => -22,       // EINVAL
            Self::UblkDeviceBusy => -16,       // EBUSY
            Self::UblkDeviceNotFound => -19,   // ENODEV
            Self::UblkInvalidDeviceId => -22,  // EINVAL
            Self::UblkNotPermitted => -1,      // EPERM
            Self::IoUringSubmitFull => -11,    // EAGAIN
            Self::IoUringCqOverflow => -75,    // EOVERFLOW
            Self::IoUringInvalidOpcode => -22, // EINVAL
            Self::BlockError => -5,            // EIO
            Self::NoTagsAvailable => -11,      // EAGAIN
            Self::InvalidQueueId => -22,       // EINVAL
            Self::NotSupported => -95,         // EOPNOTSUPP
            Self::InvalidArgument => -22,      // EINVAL
            Self::WouldBlock => -11,           // EAGAIN
            Self::Interrupted => -4,           // EINTR
            Self::ResourceBusy => -16,         // EBUSY
        }
    }

    /// Create error from POSIX errno value.
    ///
    /// # Arguments
    ///
    /// * `errno` - Negative errno value
    ///
    /// # Returns
    ///
    /// Corresponding `KernelError` variant, or `InvalidArgument` for unknown values.
    #[must_use]
    pub const fn from_errno(errno: i32) -> Self {
        match errno {
            -12 => Self::OutOfMemory,
            -14 => Self::InvalidAddress,
            -110 => Self::IoTimeout,
            -19 => Self::DeviceNotReady,
            -125 => Self::Cancelled,
            -16 => Self::UblkDeviceBusy,
            -1 => Self::UblkNotPermitted,
            -75 => Self::IoUringCqOverflow,
            -5 => Self::BlockError,
            -95 => Self::NotSupported,
            -4 => Self::Interrupted,
            -11 => Self::WouldBlock,
            _ => Self::InvalidArgument,
        }
    }

    /// Check if error is retriable.
    ///
    /// Returns `true` if the operation should be retried after a delay.
    #[must_use]
    pub const fn is_retriable(self) -> bool {
        matches!(
            self,
            Self::WouldBlock
                | Self::UblkQueueFull
                | Self::IoUringSubmitFull
                | Self::NoTagsAvailable
                | Self::UblkDeviceBusy
        )
    }

    /// Check if error is a resource exhaustion error.
    #[must_use]
    pub const fn is_resource_error(self) -> bool {
        matches!(
            self,
            Self::OutOfMemory | Self::UblkQueueFull | Self::NoTagsAvailable
        )
    }
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfMemory => write!(f, "out of memory"),
            Self::InvalidAddress => write!(f, "invalid address"),
            Self::AlignmentError => write!(f, "alignment error"),
            Self::OverlappingRegion => write!(f, "overlapping memory region"),
            Self::IoTimeout => write!(f, "I/O timeout"),
            Self::DeviceNotReady => write!(f, "device not ready"),
            Self::InvalidRequest => write!(f, "invalid request"),
            Self::Cancelled => write!(f, "operation cancelled"),
            Self::UblkQueueFull => write!(f, "ublk queue full"),
            Self::UblkInvalidTag => write!(f, "invalid ublk tag"),
            Self::UblkDeviceBusy => write!(f, "ublk device busy"),
            Self::UblkDeviceNotFound => write!(f, "ublk device not found"),
            Self::UblkInvalidDeviceId => write!(f, "invalid ublk device ID"),
            Self::UblkNotPermitted => write!(f, "ublk operation not permitted"),
            Self::IoUringSubmitFull => write!(f, "io_uring submission queue full"),
            Self::IoUringCqOverflow => write!(f, "io_uring completion queue overflow"),
            Self::IoUringInvalidOpcode => write!(f, "invalid io_uring opcode"),
            Self::BlockError => write!(f, "block device error"),
            Self::NoTagsAvailable => write!(f, "no tags available"),
            Self::InvalidQueueId => write!(f, "invalid queue ID"),
            Self::NotSupported => write!(f, "operation not supported"),
            Self::InvalidArgument => write!(f, "invalid argument"),
            Self::WouldBlock => write!(f, "operation would block"),
            Self::Interrupted => write!(f, "operation interrupted"),
            Self::ResourceBusy => write!(f, "resource is busy"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for KernelError {}

// ============================================================================
// TESTS (EXTREME TDD)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // Errno Conversion Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_errno_roundtrip_common_errors() {
        let errors = [
            KernelError::OutOfMemory,
            KernelError::IoTimeout,
            KernelError::DeviceNotReady,
            KernelError::Cancelled,
            KernelError::UblkDeviceBusy,
            KernelError::IoUringCqOverflow,
            KernelError::BlockError,
            KernelError::NotSupported,
            KernelError::Interrupted,
            KernelError::WouldBlock,
        ];

        for error in errors {
            let errno = error.to_errno();
            let recovered = KernelError::from_errno(errno);
            // Note: Some errors share the same errno, so we check errno matches
            assert_eq!(
                recovered.to_errno(),
                errno,
                "errno roundtrip failed for {:?}",
                error
            );
        }
    }

    #[test]
    fn test_errno_values_are_negative() {
        let errors = [
            KernelError::OutOfMemory,
            KernelError::InvalidAddress,
            KernelError::IoTimeout,
            KernelError::UblkQueueFull,
            KernelError::IoUringSubmitFull,
            KernelError::BlockError,
        ];

        for error in errors {
            assert!(
                error.to_errno() < 0,
                "errno for {:?} should be negative",
                error
            );
        }
    }

    #[test]
    fn test_errno_specific_values() {
        // Verify specific errno mappings match POSIX
        assert_eq!(KernelError::OutOfMemory.to_errno(), -12); // ENOMEM
        assert_eq!(KernelError::InvalidAddress.to_errno(), -14); // EFAULT
        assert_eq!(KernelError::IoTimeout.to_errno(), -110); // ETIMEDOUT
        assert_eq!(KernelError::UblkDeviceBusy.to_errno(), -16); // EBUSY
        assert_eq!(KernelError::NotSupported.to_errno(), -95); // EOPNOTSUPP
    }

    #[test]
    fn test_unknown_errno_returns_invalid_argument() {
        assert_eq!(KernelError::from_errno(-9999), KernelError::InvalidArgument);
        assert_eq!(KernelError::from_errno(0), KernelError::InvalidArgument);
        assert_eq!(KernelError::from_errno(100), KernelError::InvalidArgument);
    }

    // ------------------------------------------------------------------------
    // Retriable Error Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_retriable_errors() {
        assert!(KernelError::WouldBlock.is_retriable());
        assert!(KernelError::UblkQueueFull.is_retriable());
        assert!(KernelError::IoUringSubmitFull.is_retriable());
        assert!(KernelError::NoTagsAvailable.is_retriable());
        assert!(KernelError::UblkDeviceBusy.is_retriable());
    }

    #[test]
    fn test_non_retriable_errors() {
        assert!(!KernelError::OutOfMemory.is_retriable());
        assert!(!KernelError::InvalidAddress.is_retriable());
        assert!(!KernelError::InvalidRequest.is_retriable());
        assert!(!KernelError::UblkDeviceNotFound.is_retriable());
        assert!(!KernelError::NotSupported.is_retriable());
    }

    // ------------------------------------------------------------------------
    // Resource Error Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_resource_errors() {
        assert!(KernelError::OutOfMemory.is_resource_error());
        assert!(KernelError::UblkQueueFull.is_resource_error());
        assert!(KernelError::NoTagsAvailable.is_resource_error());
    }

    #[test]
    fn test_non_resource_errors() {
        assert!(!KernelError::InvalidAddress.is_resource_error());
        assert!(!KernelError::IoTimeout.is_resource_error());
        assert!(!KernelError::NotSupported.is_resource_error());
    }

    // ------------------------------------------------------------------------
    // Display Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_display_not_empty() {
        let errors = [
            KernelError::OutOfMemory,
            KernelError::InvalidAddress,
            KernelError::IoTimeout,
            KernelError::UblkQueueFull,
            KernelError::IoUringSubmitFull,
            KernelError::BlockError,
        ];

        for error in errors {
            let display = format!("{}", error);
            assert!(!display.is_empty(), "display for {:?} is empty", error);
            assert!(
                !display.contains("KernelError"),
                "display should be human-readable"
            );
        }
    }

    // ------------------------------------------------------------------------
    // Property Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_all_errors_have_errno() {
        // Ensure all error variants have a valid errno mapping
        let errors = [
            KernelError::OutOfMemory,
            KernelError::InvalidAddress,
            KernelError::AlignmentError,
            KernelError::OverlappingRegion,
            KernelError::IoTimeout,
            KernelError::DeviceNotReady,
            KernelError::InvalidRequest,
            KernelError::Cancelled,
            KernelError::UblkQueueFull,
            KernelError::UblkInvalidTag,
            KernelError::UblkDeviceBusy,
            KernelError::UblkDeviceNotFound,
            KernelError::UblkInvalidDeviceId,
            KernelError::UblkNotPermitted,
            KernelError::IoUringSubmitFull,
            KernelError::IoUringCqOverflow,
            KernelError::IoUringInvalidOpcode,
            KernelError::BlockError,
            KernelError::NoTagsAvailable,
            KernelError::InvalidQueueId,
            KernelError::NotSupported,
            KernelError::InvalidArgument,
            KernelError::WouldBlock,
            KernelError::Interrupted,
            KernelError::ResourceBusy,
        ];

        for error in errors {
            let errno = error.to_errno();
            assert!(errno < 0, "{:?} should have negative errno", error);
            assert!(errno > -256, "{:?} errno {} is too negative", error, errno);
        }
    }

    #[test]
    fn test_error_is_copy() {
        let error = KernelError::OutOfMemory;
        let copy = error;
        assert_eq!(error, copy);
    }

    #[test]
    fn test_error_is_clone() {
        let error = KernelError::IoTimeout;
        #[allow(clippy::clone_on_copy)]
        let cloned = error.clone();
        assert_eq!(error, cloned);
    }

    #[test]
    fn test_error_equality() {
        assert_eq!(KernelError::OutOfMemory, KernelError::OutOfMemory);
        assert_ne!(KernelError::OutOfMemory, KernelError::InvalidAddress);
    }
}
