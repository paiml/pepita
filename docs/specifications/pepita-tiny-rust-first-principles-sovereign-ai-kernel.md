<div align="center">

# Pepita: Next-Gen First-Principles Rust MicroVM Kernel for Sovereign AI

[![CI](https://img.shields.io/badge/CI-Jidoka%20Gates-green)](.github/workflows/jidoka-gates.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT%2FApache--2.0-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-100%25-orange.svg)](https://www.rust-lang.org/)
[![Type](https://img.shields.io/badge/Type-MicroVM%2FUnikernel-blue.svg)](#)
[![Stack](https://img.shields.io/badge/Stack-Sovereign%20AI-purple.svg)](#)

</div>

**Pepita** is a **Next-Gen Linux-Inspired MicroVM Kernel** designed to power the **Sovereign AI Stack** (PAIML). Built from **pure first-principles Rust**, it eliminates the bloat of general-purpose kernels to provide a hyper-optimized, secure execution environment for `trueno-ublk` and AI workloads. By targeting virtualization first (KVM/QEMU, Firecracker), Pepita delivers bare-metal performance with the isolation of a VM.

## Table of Contents

- [Features](#features)
- [Executive Summary](#executive-summary)
- [Architecture Overview](#architecture-overview)
- [Sovereign Stack Integration](#sovereign-stack-integration)
- [Iron Lotus Framework](#iron-lotus-framework)
- [Kernel Interface Specifications](#kernel-interface-specifications)
- [First-Principles Rust Constraints](#first-principles-rust-constraints)
- [Testing Strategy (Certeza Methodology)](#testing-strategy-certeza-methodology)
- [Performance Requirements](#performance-requirements)
- [Popperian Falsification Checklist (100 Points)](#popperian-falsification-checklist-100-points)
- [Comparison with Existing Systems](#comparison-with-existing-systems)
- [Roadmap](#roadmap)
- [License](#license)

## Features

- ✅ **MicroVM Native**: Optimized for KVM/QEMU/Firecracker (virtio-first design)
- ✅ **Linux ABI Compatible**: Runs standard Linux binaries (userspace) via binary compatibility
- ✅ **100% Rust, Zero C/C++**: True sovereignty through complete auditability
- ✅ **First-Principles Design**: No external crates in kernel space; custom `alloc` and `core` only
- ✅ **Sovereign Stack Optimized**: Specific optimizations for `trueno-ublk` and `paiml` components
- ✅ **io_uring Native**: The primary I/O interface; legacy syscalls minimized
- ✅ **Unikernel Efficiency**: Single address space option for maximum throughput
- ✅ **Iron Lotus Quality**: Toyota Way principles operationalized as kernel invariants
- ✅ **Certeza Testing**: Three-tiered validation (unit → integration → formal verification)

## Executive Summary

**Version:** 1.0.0-draft (MicroVM Edition)
**Status:** DRAFT - Iteration 2 (Sovereign Stack)
**Last Updated:** 2026-01-04
**Quality Framework:** Iron Lotus + Certeza

Pepita is not a general-purpose operating system. It is a **specialized MicroVM kernel** inspired by Linux but built for the specific requirements of the **Sovereign AI Stack**. It discards legacy hardware support in favor of a **virtio-only architecture**, allowing it to boot in milliseconds and dedicate 99% of resources to AI inference and data processing.

**The "Next-Gen" Philosophy:**
1.  **Virtualization is the Hardware**: We don't write drivers for 1990s sound cards. We write drivers for `virtio-blk`, `virtio-net`, and `virtio-gpu`.
2.  **Linux Compatibility, Not Cloning**: We implement the *interface* (syscalls, /dev/ublk) that the Sovereign Stack expects, but the *implementation* is pure Rust.
3.  **Sovereignty via Simplicity**: A 30 million line kernel cannot be audited. A 50k line kernel can be.

**Key Differentiators:**
- **MicroVM Architecture**: Designed to run as a guest on KVM, enabling high density and isolation.
- **Sovereign Stack Integration**: Pre-tuned for `trueno-ublk` (block storage) and `trueno-zram` (memory compression).
- **Zero-Copy I/O**: `io_uring` passthrough from userspace to virtio backend.

**Subsystem Budget (≤50K LoC Target):**

| Subsystem | Lines (Est.) | Justification |
|-----------|--------------|---------------|
| io_uring | 8,000 | Core async I/O for ublk data plane |
| ublk driver | 3,000 | Userspace block device interface |
| virtio drivers | 6,000 | virtio-blk, virtio-pci (MicroVM focus) |
| Memory (buddy) | 4,000 | Page allocation |
| Memory (mmap) | 3,000 | Userspace buffer mapping |
| Scheduler | 2,000 | Basic round-robin / Cooperative |
| Interrupts | 1,500 | IRQ handling (MSI-X focus) |
| Boot/Arch | 3,000 | PVH / Linux Boot Protocol |
| VFS (minimal) | 3,000 | /dev filesystem only |
| **Total** | **~33,500** | Target: ≤50K LoC |

## Architecture Overview

### MicroVM Layer Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                 Sovereign AI Stack (Userspace)                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │ trueno-ublk │  │ trueno-zram │  │      PAIML Agents       │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
└───────────────────────────┬─────────────────────────────────────┘
                            │ Linux ABI (Syscalls / io_uring)
┌───────────────────────────▼─────────────────────────────────────┐
│                    Pepita MicroVM Kernel                        │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                    io_uring Subsystem                       ││
│  │  - Primary Async Interface for all I/O                      ││
│  └─────────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────┐ ┌─────────────────────────────┐│
│  │        ublk Driver          │ │      Memory Manager         ││
│  │  (Zero-Copy Passthrough)    │ │   (Hugepage Optimized)      ││
│  └─────────────────────────────┘ └─────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                    Virtio Drivers                           ││
│  │  - virtio-blk (Storage)                                     ││
│  │  - virtio-gpu (Compute Passthrough)                         ││
│  │  - virtio-net (Management)                                  ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
                            │ Virtio / MMIO
┌───────────────────────────▼─────────────────────────────────────┐
│                       Hypervisor                                │
│                (KVM / QEMU / Firecracker)                       │
└─────────────────────────────────────────────────────────────────┘
```

### Data Flow: Sovereign Stack Optimization

The kernel is specifically tuned to reduce latency for the Sovereign Stack's unique I/O patterns:

1.  **AI Model Loading**: `virtio-blk` -> `io_uring` -> `trueno-ublk` (decompression) -> GPU Memory. Pepita optimizes this path by removing VFS caching overhead.
2.  **Inference Paging**: `trueno-zram` (swap) -> `io_uring` -> Memory. Pepita ensures these page faults are handled via async I/O where possible.

## Sovereign Stack Integration

Pepita is the "OS layer" of the Sovereign AI Stack:

- **Trueno (Storage/Compression):** Pepita provides the `ublk` interface required by Trueno, optimized for high-throughput `io_uring` commands.
- **Batuta (Orchestration):** Pepita exposes a minimal control plane (via virtio-serial or vsock) for Batuta to manage the MicroVM lifecycle.
- **Aprender (Training):** Pepita supports hugepages and GPU passthrough (via VFIO/virtio-gpu) to allow raw hardware access for training jobs.

## Iron Lotus Framework

Pepita embodies Toyota Production System principles operationalized for kernel development:

### Genchi Genbutsu (現地現物 - "Go and See")

- **Radical Transparency**: Every kernel operation traceable from syscall → hardware
- **No Black Boxes**: 100% pure Rust, zero opaque C/C++ libraries
- **AST-Level Inspection**: Code structure visible via pmat analysis
- **Tracing Built-In**: Runtime observability for every subsystem

### Jidoka (自働化 - "Automation with Human Touch")

- **Panic on Invariant Violation**: No silent failures, immediate stop-the-line
- **Compile-Time Enforcement**: Rust type system as automated quality gate
- **Andon Cord**: Build fails immediately on any defect
- **No Manual Checks**: Machines verify before humans review

### Kaizen (改善 - "Continuous Improvement")

- **Technical Debt Grading**: TDG score must never decrease
- **Ratchet Effect**: Each commit improves or maintains quality
- **Five Whys**: Root cause analysis for all kernel panics
- **Falsification Checklist**: 100-point scientific validation

### Muda (無駄 - "Waste Elimination")

- **No Overproduction**: Zero YAGNI features, minimal subsystems only
- **No Waiting**: io_uring batch submission, async I/O everywhere
- **No Transportation**: Zero-copy I/O paths, single language
- **No Defects**: EXTREME TDD with mutation testing (≥80% score)

### Poka-yoke (ポカヨケ - "Mistake-Proofing")

- **Type-Safe Syscall Interfaces**: Lifetime enforcement at API boundaries
- **Newtype Wrappers**: PhysAddr, VirtAddr, Pfn prevent type confusion
- **Bounds Checking**: All array accesses validated
- **SAFETY Comments**: Every unsafe block documented

### Heijunka (平準化 - "Level Scheduling")

- **io_uring Batch Submission**: Fair queue scheduling
- **Per-CPU Request Queues**: Load distribution via blk-mq
- **Throughput Variance ≤10%**: Consistent performance

## Kernel Interface Specifications

### io_uring Interface (8,000 LoC)

```rust
//! Pepita io_uring implementation (first-principles)
//! No external dependencies - only core::* and kernel primitives

#![no_std]

use core::sync::atomic::{AtomicU32, Ordering};

/// Submission Queue Entry (64 bytes, kernel ABI)
/// Matches Linux include/uapi/linux/io_uring.h exactly
#[repr(C)]
pub struct IoUringSqe {
    pub opcode: u8,
    pub flags: u8,
    pub ioprio: u16,
    pub fd: i32,
    pub off: u64,
    pub addr: u64,
    pub len: u32,
    pub op_flags: u32,
    pub user_data: u64,
    pub buf_index: u16,
    pub personality: u16,
    pub splice_fd_in: i32,
    pub addr3: u64,
    pub __pad2: [u64; 1],
}

/// Completion Queue Entry (16 bytes, kernel ABI)
#[repr(C)]
pub struct IoUringCqe {
    pub user_data: u64,
    pub res: i32,
    pub flags: u32,
}

/// URING_CMD opcode for ublk passthrough
pub const IORING_OP_URING_CMD: u8 = 46;

/// Required io_uring operations for ublk
pub trait IoUringOps {
    /// Submit SQE batch to kernel
    fn submit(&mut self, count: u32) -> Result<u32, IoUringError>;

    /// Wait for CQE completions
    fn wait(&mut self, min_complete: u32) -> Result<u32, IoUringError>;

    /// Get next CQE (non-blocking)
    fn peek_cqe(&self) -> Option<&IoUringCqe>;

    /// Advance CQ head after processing
    fn cq_advance(&mut self, count: u32);

    /// Register fixed buffers for zero-copy
    fn register_buffers(&mut self, buffers: &[IoVec]) -> Result<(), IoUringError>;
}
```

### ublk Kernel Interface (3,000 LoC)

```rust
//! ublk kernel interface (matches include/uapi/linux/ublk_cmd.h)
//! Direct port from Linux kernel headers - zero external dependencies

/// Control command payload (32 bytes) - matches kernel ublksrv_ctrl_cmd
#[repr(C)]
pub struct UblkCtrlCmd {
    pub dev_id: u32,
    pub queue_id: u16,
    pub len: u16,
    pub addr: u64,
    pub data: [u64; 1],
    pub dev_path_len: u16,
    pub pad: u16,
    pub reserved: u32,
}

/// I/O descriptor (24 bytes) - mmap'd to userspace
/// Zero-copy via shared memory region
#[repr(C)]
pub struct UblkIoDesc {
    pub op_flags: u32,      // Operation type + flags
    pub nr_sectors: u32,    // Request size in sectors
    pub start_sector: u64,  // LBA offset
    pub addr: u64,          // Buffer address (userspace)
}

/// I/O command (16 bytes) - via io_uring SQE
#[repr(C)]
pub struct UblkIoCmd {
    pub q_id: u16,
    pub tag: u16,
    pub result: i32,
    pub addr: u64,
}

// ioctl-encoded command opcodes (matches kernel exactly)
pub const UBLK_U_CMD_ADD_DEV: u32 = 0xc020_7504;
pub const UBLK_U_CMD_DEL_DEV: u32 = 0xc020_7505;
pub const UBLK_U_CMD_START_DEV: u32 = 0xc020_7506;
pub const UBLK_U_CMD_STOP_DEV: u32 = 0xc020_7507;
pub const UBLK_U_CMD_SET_PARAMS: u32 = 0xc020_7508;
pub const UBLK_U_CMD_GET_PARAMS: u32 = 0x8020_7509;

pub const UBLK_U_IO_FETCH_REQ: u32 = 0xc010_7520;
pub const UBLK_U_IO_COMMIT_AND_FETCH_REQ: u32 = 0xc010_7521;

// Device capability flags
pub const UBLK_F_SUPPORT_ZERO_COPY: u64 = 1 << 0;
pub const UBLK_F_USER_COPY: u64 = 1 << 7;
pub const UBLK_F_CMD_IOCTL_ENCODE: u64 = 1 << 6;
```

### blk-mq Interface (5,000 LoC)

```rust
//! Block multi-queue (blk-mq) abstractions
//! Based on Linux kernel block/blk-mq.c patterns

/// Block device operations trait (Rust-native vtable)
pub trait BlockOps: Send + Sync {
    /// Queue data associated with each hardware queue
    type QueueData: Send + Sync;

    /// Process a request from the block layer
    /// Called when I/O is submitted to /dev/ublkb*
    fn queue_rq(
        queue_data: &Self::QueueData,
        request: Request,
        is_last: bool,
    ) -> Result<(), BlockError>;

    /// Commit outstanding requests
    fn commit_rqs(queue_data: &Self::QueueData);

    /// Handle request completion
    fn complete(request: Request);
}

/// Tag set configuration (matches kernel blk_mq_tag_set)
pub struct TagSetConfig {
    pub nr_hw_queues: u16,
    pub queue_depth: u16,
    pub numa_node: i32,
    pub flags: u32,
}

/// Request structure (mirrors kernel struct request)
pub struct Request {
    pub tag: u16,
    pub queue_id: u16,
    pub op: RequestOp,
    pub sector: u64,
    pub nr_sectors: u32,
    pub bio_vec: BioVec,
}

pub enum RequestOp {
    Read,
    Write,
    Flush,
    Discard,
    WriteZeroes,
}
```

### Memory Management Interface (7,000 LoC)

```rust
//! Memory management for ublk buffers
//! Buddy allocator + mmap for userspace sharing

/// Page frame number (type-safe wrapper)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pfn(u64);

/// Physical address (type-safe wrapper)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysAddr(u64);

/// Virtual address (type-safe wrapper)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtAddr(u64);

/// Page allocator trait (buddy system implementation)
pub trait PageAllocator {
    /// Allocate contiguous pages (order = log2(count))
    fn alloc_pages(&mut self, order: u32, flags: AllocFlags) -> Option<Pfn>;

    /// Free previously allocated pages
    fn free_pages(&mut self, pfn: Pfn, order: u32);

    /// Get total free pages
    fn free_count(&self) -> usize;
}

/// Memory mapping for userspace (io_uring buffer registration)
pub trait MmapOps {
    /// Map kernel pages into userspace
    fn mmap_pages(
        &mut self,
        vma: &mut VmArea,
        pfn: Pfn,
        count: usize,
        prot: Protection,
    ) -> Result<VirtAddr, MmapError>;

    /// Unmap pages from userspace
    fn munmap(&mut self, addr: VirtAddr, len: usize) -> Result<(), MmapError>;
}

/// DMA buffer management (for GPU passthrough)
pub struct DmaBuffer {
    pub phys: PhysAddr,
    pub virt: VirtAddr,
    pub size: usize,
    pub direction: DmaDirection,
}
```

## First-Principles Rust Constraints

### Dependency Policy

**Allowed in Kernel Space:**
- `core` (Rust standard library core)
- `alloc` (heap allocation with custom allocator)
- Kernel-internal crates defined within Pepita repository

**Prohibited in Kernel Space:**
- Any crates.io dependencies
- `std` library
- External `no_std` crates
- C library bindings (libc, nix)

**Rationale:** True digital sovereignty requires complete auditability. Every line of kernel code must be visible, reviewable, and free from external supply chain risks.

### Unsafe Isolation Pattern

Following Rust-for-Linux patterns, unsafe code must be:

1. **Contained in abstraction modules** - Never in driver logic
2. **Documented with safety invariants** - Every `unsafe` block requires `// SAFETY:` comment
3. **Minimized through type design** - Use newtype wrappers, NonNull, etc.

```rust
// GOOD: Unsafe contained in abstraction
pub mod mmio {
    pub struct MmioRegion {
        base: core::ptr::NonNull<u8>,
        size: usize,
    }

    impl MmioRegion {
        /// # Safety
        /// - `base` must be valid MMIO address
        /// - Region must not overlap with other mappings
        pub unsafe fn new(base: *mut u8, size: usize) -> Self {
            Self {
                base: NonNull::new_unchecked(base),
                size,
            }
        }

        pub fn read32(&self, offset: usize) -> u32 {
            assert!(offset + 4 <= self.size);
            // SAFETY: Bounds checked above, volatile for MMIO
            unsafe {
                core::ptr::read_volatile(
                    self.base.as_ptr().add(offset) as *const u32
                )
            }
        }
    }
}

// Driver code is SAFE - no unsafe needed
fn init_device(mmio: &MmioRegion) {
    let status = mmio.read32(STATUS_OFFSET);  // Safe API
}
```

### Error Handling

No panics in normal operation paths. Use Result types with explicit error enums:

```rust
#[derive(Debug)]
pub enum KernelError {
    // Memory errors
    OutOfMemory,
    InvalidAddress,

    // I/O errors
    IoTimeout,
    DeviceNotReady,
    InvalidRequest,

    // ublk specific
    UblkQueueFull,
    UblkInvalidTag,
    UblkDeviceBusy,
}

// No unwrap() in kernel code - explicit error handling only
pub fn process_request(req: &Request) -> Result<(), KernelError> {
    let buffer = alloc_buffer(req.size)
        .ok_or(KernelError::OutOfMemory)?;

    // Process...
    Ok(())
}
```

## Testing Strategy (Certeza Methodology)

Pepita uses a three-tiered testing approach validated by the Certeza framework:

### Tier 1: ON-SAVE (Sub-Second)

Fast feedback for flow state:

```bash
make tier1
```

- Unit tests for struct layouts (ABI verification)
- `cargo check` for type errors
- `cargo clippy` for lint violations
- `cargo fmt` for formatting

**Target**: < 3 seconds

### Tier 2: ON-COMMIT (1-5 Minutes)

Comprehensive pre-commit gate:

```bash
make tier2
```

- All unit tests (ABI, memory safety, functional)
- Property-based tests (proptest for edge cases)
- Coverage analysis (target ≥95%)
- Documentation tests
- Integration tests with mock hardware

**Target**: 1-5 minutes

### Tier 3: ON-MERGE (Hours)

Exhaustive validation:

```bash
make tier3
```

- Mutation testing (cargo-mutants, target ≥80%)
- Formal verification (Kani, for critical paths)
- QEMU/KVM integration tests
- Performance benchmarks vs Linux kernel
- Falsification checklist verification

**Target**: 1-6 hours (run overnight or in CI)

### Built-In Kernel Unit Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[kernel_test]
    fn test_ublk_io_desc_layout() {
        // Verify ABI compatibility with Linux kernel
        assert_eq!(core::mem::size_of::<UblkIoDesc>(), 24);
        assert_eq!(core::mem::offset_of!(UblkIoDesc, op_flags), 0);
        assert_eq!(core::mem::offset_of!(UblkIoDesc, nr_sectors), 4);
        assert_eq!(core::mem::offset_of!(UblkIoDesc, start_sector), 8);
        assert_eq!(core::mem::offset_of!(UblkIoDesc, addr), 16);
    }

    #[kernel_test]
    fn test_ioctl_encoding() {
        // Verify ioctl numbers match kernel headers
        assert_eq!(UBLK_U_CMD_ADD_DEV, 0xc020_7504);
        assert_eq!(UBLK_U_IO_FETCH_REQ, 0xc010_7520);
    }
}
```

## Performance Requirements

### Throughput Targets

| Metric | Target | Measurement | Comparable |
|--------|--------|-------------|------------|
| ublk IOPS (4K random) | ≥500K IOPS | fio benchmark | Linux: 600K |
| ublk bandwidth (seq) | ≥10 GB/s | fio 128K blocks | Linux: 12 GB/s |
| io_uring submit latency | ≤5 μs | perf trace | Linux: 3 μs |
| Context switch | ≤2 μs | cyclictest | Linux: 1.5 μs |
| Memory footprint | ≤16 MB | /proc/meminfo | Linux: 50+ MB |

### Latency Requirements

Following Gregg & Hazelwood (2011) PCIe overhead analysis:

```
Dispatch rule: compute_time > 5 × transfer_time

For trueno-ublk compression on PCIe 4.0 x16:
- Effective bandwidth: ~28 GB/s
- 4KB page transfer: 4096 / 28e9 ≈ 146 ns
- Minimum useful compute: 5 × 146 ns = 730 ns

LZ4 compression must take >730ns per page to justify GPU offload.
CPU SIMD paths (AVX-512, NEON) preferred for small batches.
GPU beneficial only for batches ≥10K pages.
```

### Performance Budget

| Operation | Budget | Justification |
|-----------|--------|---------------|
| Syscall entry/exit | ≤200 ns | Minimal overhead |
| io_uring SQE processing | ≤500 ns | Batch amortization |
| blk-mq tag allocation | ≤100 ns | Lock-free design |
| ublk request forward | ≤1 μs | Zero-copy path |
| Page allocation (order 0) | ≤500 ns | Buddy allocator |

## Popperian Falsification Checklist (100 Points)

Following Karl Popper's principle of falsifiability: a specification is scientific only if it can be proven wrong. Each item below represents a testable claim that, if falsified, indicates a specification failure.

### Section A: Structural Invariants (Points 1-20)

| # | Falsifiable Claim | Test Method | Pass Criteria |
|---|-------------------|-------------|---------------|
| 1 | UblkCtrlCmd is exactly 32 bytes | `size_of::<UblkCtrlCmd>()` | == 32 |
| 2 | UblkIoDesc is exactly 24 bytes | `size_of::<UblkIoDesc>()` | == 24 |
| 3 | UblkIoCmd is exactly 16 bytes | `size_of::<UblkIoCmd>()` | == 16 |
| 4 | IoUringSqe is exactly 64 bytes | `size_of::<IoUringSqe>()` | == 64 |
| 5 | IoUringCqe is exactly 16 bytes | `size_of::<IoUringCqe>()` | == 16 |
| 6 | UblkCtrlCmd.dev_id offset is 0 | `offset_of!(UblkCtrlCmd, dev_id)` | == 0 |
| 7 | UblkCtrlCmd.queue_id offset is 4 | `offset_of!(UblkCtrlCmd, queue_id)` | == 4 |
| 8 | UblkCtrlCmd.addr offset is 8 | `offset_of!(UblkCtrlCmd, addr)` | == 8 |
| 9 | UblkIoDesc.start_sector offset is 8 | `offset_of!(UblkIoDesc, start_sector)` | == 8 |
| 10 | UblkIoCmd.result offset is 4 | `offset_of!(UblkIoCmd, result)` | == 4 |
| 11 | All structs have 8-byte alignment | `align_of::<T>()` for all | == 8 |
| 12 | UBLK_U_CMD_ADD_DEV == 0xc0207504 | ioctl encoding verification | Match |
| 13 | UBLK_U_IO_FETCH_REQ == 0xc0107520 | ioctl encoding verification | Match |
| 14 | SECTOR_SIZE == 512 | Constant check | == 512 |
| 15 | PAGE_SIZE == 4096 | Constant check | == 4096 |
| 16 | Maximum queue depth ≤ 32768 | Config validation | ≤ 32768 |
| 17 | Tag width is u16 | Type check | 16 bits |
| 18 | Queue ID width is u16 | Type check | 16 bits |
| 19 | Device ID width is u32 | Type check | 32 bits |
| 20 | All flag constants are powers of 2 | Bit pattern check | Single bit |

### Section B: Memory Safety (Points 21-40)

| # | Falsifiable Claim | Test Method | Pass Criteria |
|---|-------------------|-------------|---------------|
| 21 | No use-after-free in ublk daemon | Miri + AddressSanitizer | Zero errors |
| 22 | No double-free in page allocator | Miri + custom tracker | Zero errors |
| 23 | No buffer overflows in io_uring | Bounds check + fuzzing | Zero overflows |
| 24 | No null pointer dereferences | Static analysis + Miri | Zero violations |
| 25 | mmap regions don't overlap | Runtime validation | No overlaps |
| 26 | DMA buffers are properly aligned | Alignment assertion | % alignment == 0 |
| 27 | All unsafe blocks have SAFETY comments | grep + lint | 100% coverage |
| 28 | No raw pointer arithmetic in safe code | Clippy lint | Zero violations |
| 29 | Lifetime annotations prevent dangling refs | Compile-time check | Compiles cleanly |
| 30 | RefCell/Mutex prevent data races | ThreadSanitizer | Zero races |
| 31 | Page table entries are valid | Hardware walk verification | All valid |
| 32 | Interrupt handlers don't allocate | Static analysis | No alloc calls |
| 33 | Signal handlers are async-signal-safe | Code audit | Only safe ops |
| 34 | Stack canaries detect overflow | Runtime check | Canary intact |
| 35 | Heap metadata is protected | Guard pages | Access trapped |
| 36 | User pointers are validated | copy_from_user pattern | All checked |
| 37 | Kernel pointers never leak to user | Output sanitization | No leaks |
| 38 | ASLR entropy is sufficient | Address analysis | ≥ 28 bits |
| 39 | W^X enforced (no WX pages) | Page table scan | Zero WX pages |
| 40 | SMAP/SMEP enabled | CPU feature check | Both enabled |

### Section C: Functional Correctness (Points 41-60)

| # | Falsifiable Claim | Test Method | Pass Criteria |
|---|-------------------|-------------|---------------|
| 41 | ublk device creation succeeds | ADD_DEV command | Returns dev_id |
| 42 | ublk device deletion succeeds | DEL_DEV command | Returns 0 |
| 43 | ublk params can be set/get | SET/GET_PARAMS round-trip | Params match |
| 44 | io_uring FETCH_REQ works | Submit + wait | CQE received |
| 45 | io_uring COMMIT_AND_FETCH works | Submit + verify | Request completed |
| 46 | Block device appears in /dev | stat(/dev/ublkbN) | Exists |
| 47 | Block device is readable | read() syscall | Returns data |
| 48 | Block device is writable | write() syscall | Returns count |
| 49 | Read after write returns same data | Write then read | Data matches |
| 50 | Discard operation completes | DISCARD request | Returns success |
| 51 | Flush operation completes | FLUSH request | Returns success |
| 52 | Multiple queues work independently | Parallel I/O | No interference |
| 53 | Tag reuse works correctly | Rapid alloc/free | No tag collision |
| 54 | Queue affinity is respected | CPU binding check | Correct CPU |
| 55 | Device survives userspace crash | Kill + recovery | Device functional |
| 56 | Hot removal is clean | Remove during I/O | No kernel panic |
| 57 | Zero-copy path avoids copies | perf memory trace | Zero extra copies |
| 58 | USER_COPY fallback works | Disable zero-copy | Still functional |
| 59 | Error codes match POSIX | errno verification | Correct codes |
| 60 | Resource limits enforced | Exceed limits | Returns -ENOMEM |

### Section D: Performance (Points 61-80)

| # | Falsifiable Claim | Test Method | Pass Criteria |
|---|-------------------|-------------|---------------|
| 61 | 4K random read IOPS ≥ 500K | fio benchmark | ≥ 500,000 |
| 62 | 4K random write IOPS ≥ 400K | fio benchmark | ≥ 400,000 |
| 63 | 128K sequential read ≥ 10 GB/s | fio benchmark | ≥ 10 GB/s |
| 64 | 128K sequential write ≥ 8 GB/s | fio benchmark | ≥ 8 GB/s |
| 65 | io_uring submit latency ≤ 5 μs | perf trace | ≤ 5000 ns |
| 66 | Context switch ≤ 2 μs | cyclictest | ≤ 2000 ns |
| 67 | Interrupt latency ≤ 10 μs | hwlat_detector | ≤ 10000 ns |
| 68 | Memory footprint ≤ 16 MB | /proc/meminfo | ≤ 16 MB |
| 69 | CPU usage scales linearly | Multi-core test | Linear scaling |
| 70 | No priority inversion | rt-tests | No inversions |
| 71 | Throughput variance ≤ 10% | Statistical analysis | CV ≤ 0.10 |
| 72 | P99 latency ≤ 2× P50 | Latency histogram | Ratio ≤ 2.0 |
| 73 | P999 latency ≤ 5× P50 | Latency histogram | Ratio ≤ 5.0 |
| 74 | No regression vs baseline | A/B comparison | Within 5% |
| 75 | NUMA-aware allocation | numastat | Local allocation |
| 76 | Cache efficiency ≥ 95% | perf stat | Hit rate ≥ 95% |
| 77 | TLB miss rate ≤ 1% | perf stat | Miss rate ≤ 1% |
| 78 | Branch prediction ≥ 99% | perf stat | Accuracy ≥ 99% |
| 79 | IPC ≥ 2.0 for I/O path | perf stat | IPC ≥ 2.0 |
| 80 | Power efficiency (perf/watt) | Power measurement | Meets target |

### Section E: Security (Points 81-90)

| # | Falsifiable Claim | Test Method | Pass Criteria |
|---|-------------------|-------------|---------------|
| 81 | Unprivileged users can't create devices | Permission test | Returns -EPERM |
| 82 | Device isolation between users | Multi-user test | No cross-access |
| 83 | Capability checks enforced | CAP_SYS_ADMIN test | Required for ops |
| 84 | seccomp filters work | Restricted syscalls | Blocked correctly |
| 85 | Namespace isolation works | Container test | Isolated devices |
| 86 | No information leaks via timing | Timing analysis | Constant time |
| 87 | Kernel memory not readable | /dev/mem test | Access denied |
| 88 | KASLR effective | Address prediction | Random layout |
| 89 | Stack protector works | Overflow attempt | Detected + killed |
| 90 | CFI prevents ROP/JOP | Control flow test | Attacks blocked |

### Section F: Compatibility (Points 91-100)

| # | Falsifiable Claim | Test Method | Pass Criteria |
|---|-------------------|-------------|---------------|
| 91 | ABI matches Linux 6.0+ | struct comparison | Byte-identical |
| 92 | Works with liburing | liburing test suite | All tests pass |
| 93 | Works with ublksrv | ublksrv test suite | All tests pass |
| 94 | trueno-ublk integrates | Integration test | Compression works |
| 95 | x86_64 support complete | Architecture test | Full function |
| 96 | aarch64 support complete | Architecture test | Full function |
| 97 | QEMU/KVM compatible | VM test | Boots + works |
| 98 | Virtio-blk backend works | Virtio test | I/O functional |
| 99 | NVMe backend works | NVMe test | I/O functional |
| 100 | Upgrade path from Linux | Migration test | Seamless switch |

## Comparison with Existing Systems

| Feature | Pepita | Linux Kernel | Theseus | Tock OS |
|---------|--------|--------------|---------|---------|
| Language | Rust (100%) | C + Rust | Rust (100%) | Rust (100%) |
| C Dependencies | Zero | Extensive | Zero | Minimal |
| Lines of Code | ≤50K target | 30M+ | ~200K | ~50K |
| ublk Support | Native | Native | ❌ | ❌ |
| io_uring Support | Native | Native | ❌ | ❌ |
| MicroVM Native | ✅ (Primary) | ⚠️ (Requires strip) | ❌ | ❌ |
| Memory Safety | Guaranteed | Partial (Rust parts) | Guaranteed | Guaranteed |
| Sovereign AI Focus | ✅ | ❌ | ❌ | ❌ |
| Quality Framework | Iron Lotus | ❌ | ❌ | ❌ |

**Star Ratings (Suitability for Sovereign AI):**

| Criterion | Pepita | Linux | Theseus | Tock |
|-----------|--------|-------|---------|------|
| Auditability | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| Memory Safety | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Block I/O Perf | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐ |
| Minimalism | ⭐⭐⭐⭐⭐ | ⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ |
| PAIML Integration | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐ | ⭐ |

## Roadmap

### v0.1: Bootstrap (Phase 1)
- [ ] Minimal boot: Linux Boot Protocol (x86_64)
- [ ] Memory: Buddy allocator, basic mmap
- [ ] Interrupts: IDT setup, IRQ handling
- [ ] Virtio: Basic virtio-pci enumeration

### v0.2: I/O Subsystem (Phase 2)
- [ ] io_uring: SQ/CQ rings, basic operations
- [ ] io_uring: URING_CMD passthrough
- [ ] io_uring: Buffer registration
- [ ] Virtio: virtio-blk driver (polled mode)

### v0.3: Block Layer (Phase 3)
- [ ] blk-mq: Request queues, tag management
- [ ] blk-mq: Multi-queue support
- [ ] ublk: Control plane (/dev/ublk-control)
- [ ] ublk: Data plane (char + block devices)

### v1.0: Integration (Phase 4)
- [ ] trueno-ublk: Full integration
- [ ] trueno-zram: Compression validation
- [ ] Performance: Benchmark suite (fio, perf)
- [ ] Security: Hardening pass

### v2.0: Production (Phase 5)
- [ ] aarch64 support
- [ ] GPU passthrough (VFIO/virtio-gpu)
- [ ] QEMU/KVM certification
- [ ] Iron Lotus quality gates: TDG ≥ 95, Coverage ≥ 95%, Mutation ≥ 80%

## License

MIT OR Apache-2.0 (consistent with Rust ecosystem)