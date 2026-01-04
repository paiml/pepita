//! # Pepita: Sovereign AI Distributed Computing Primitives
//!
//! Pepita provides minimal kernel interfaces and distributed computing primitives
//! for running Sovereign AI workloads. It combines low-level kernel interfaces
//! (`ublk`, `io_uring`, `blk-mq`) with high-level distributed execution
//! (`scheduler`, `executor`, `pool`).
//!
//! ## Design Principles (Iron Lotus Framework)
//!
//! - **First-Principles Rust**: Zero external dependencies in kernel mode
//! - **Pure Rust Sovereignty**: 100% auditable, zero C/C++ dependencies
//! - **Work-Stealing Scheduler**: Blumofe-Leiserson algorithm [1]
//! - **Toyota Way Quality**: Jidoka, Poka-yoke, Genchi Genbutsu
//! - **Certeza Testing**: 95% coverage, 80% mutation score
//!
//! ## Features
//!
//! - `std` (default): Standard library support for testing and distributed mode
//! - `kernel`: True `#![no_std]` mode for kernel integration
//!
//! ## Example (Kernel Interfaces)
//!
//! ```rust
//! use pepita::ublk::{UblkCtrlCmd, UblkIoDesc, UblkIoCmd};
//! use pepita::io_uring::{IoUringSqe, IoUringCqe};
//!
//! // Verify ABI compatibility with Linux kernel
//! assert_eq!(core::mem::size_of::<UblkCtrlCmd>(), 32);
//! assert_eq!(core::mem::size_of::<UblkIoDesc>(), 24);
//! assert_eq!(core::mem::size_of::<IoUringSqe>(), 64);
//! ```
//!
//! ## Example (Distributed Computing)
//!
//! ```rust,ignore
//! use pepita::pool::Pool;
//! use pepita::task::Task;
//! use pepita::executor::Backend;
//!
//! // Create execution pool
//! let pool = Pool::builder()
//!     .cpu_workers(4)
//!     .build()?;
//!
//! // Submit task
//! let task = Task::binary("./worker")
//!     .args(vec!["--input", "data.bin"])
//!     .backend(Backend::Cpu)
//!     .build();
//!
//! let result = pool.submit(task)?;
//! ```
//!
//! ## References
//!
//! [1] Blumofe & Leiserson (1999). "Scheduling Multithreaded Computations
//!     by Work Stealing." Journal of the ACM.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(unsafe_code)] // Temporarily deny all unsafe until properly audited
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

// Core modules - kernel interfaces (no_std compatible)
pub mod blk_mq;
pub mod error;
pub mod io_uring;
pub mod memory;
pub mod ublk;

// Distributed computing modules (std only)
#[cfg(feature = "std")]
pub mod executor;
#[cfg(feature = "std")]
pub mod fault;
#[cfg(feature = "std")]
pub mod pool;
#[cfg(feature = "std")]
pub mod scheduler;
#[cfg(feature = "std")]
pub mod task;
#[cfg(feature = "std")]
pub mod transport;

// Sovereign infrastructure modules (std only)
#[cfg(feature = "std")]
pub mod gpu;
#[cfg(feature = "std")]
pub mod simd;
#[cfg(feature = "std")]
pub mod virtio;
#[cfg(feature = "std")]
pub mod vmm;
#[cfg(feature = "std")]
pub mod zram;

// Re-exports for convenience
pub use blk_mq::{BlockOps, Request, RequestOp, TagSetConfig};
pub use error::{KernelError, Result};
pub use io_uring::{IoUringCqe, IoUringSqe, IORING_OP_URING_CMD};
pub use memory::{DmaBuffer, DmaDirection, PageAllocator, Pfn, PhysAddr, VirtAddr};
pub use ublk::{
    UblkCtrlCmd, UblkIoCmd, UblkIoDesc, UBLK_F_CMD_IOCTL_ENCODE, UBLK_F_SUPPORT_ZERO_COPY,
    UBLK_F_USER_COPY, UBLK_U_CMD_ADD_DEV, UBLK_U_CMD_DEL_DEV, UBLK_U_CMD_GET_PARAMS,
    UBLK_U_CMD_SET_PARAMS, UBLK_U_CMD_START_DEV, UBLK_U_CMD_STOP_DEV,
    UBLK_U_IO_COMMIT_AND_FETCH_REQ, UBLK_U_IO_FETCH_REQ,
};

/// Kernel constants
pub mod constants {
    /// Sector size in bytes (standard block device sector)
    pub const SECTOR_SIZE: u32 = 512;

    /// Page size in bytes (standard x86_64/aarch64)
    pub const PAGE_SIZE: usize = 4096;

    /// Maximum queue depth for ublk devices
    pub const MAX_QUEUE_DEPTH: u16 = 32768;

    /// Maximum number of hardware queues
    pub const MAX_HW_QUEUES: u16 = 128;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify all public types are accessible
    #[test]
    fn test_public_api_accessible() {
        // ublk types
        let _ = core::mem::size_of::<UblkCtrlCmd>();
        let _ = core::mem::size_of::<UblkIoDesc>();
        let _ = core::mem::size_of::<UblkIoCmd>();

        // io_uring types
        let _ = core::mem::size_of::<IoUringSqe>();
        let _ = core::mem::size_of::<IoUringCqe>();

        // memory types
        let _ = core::mem::size_of::<Pfn>();
        let _ = core::mem::size_of::<PhysAddr>();
        let _ = core::mem::size_of::<VirtAddr>();

        // blk-mq types
        let _ = core::mem::size_of::<Request>();
        let _ = core::mem::size_of::<RequestOp>();
    }

    /// Verify constants are correct
    #[test]
    fn test_constants() {
        assert_eq!(constants::SECTOR_SIZE, 512);
        assert_eq!(constants::PAGE_SIZE, 4096);
        assert_eq!(constants::MAX_QUEUE_DEPTH, 32768);
    }
}
