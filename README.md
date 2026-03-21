<p align="center">
[![CI](https://github.com/paiml/pepita/actions/workflows/ci.yml/badge.svg)](https://github.com/paiml/pepita/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/pepita.svg)](https://crates.io/crates/pepita)
[![Documentation](https://docs.rs/pepita/badge.svg)](https://docs.rs/pepita)

# Pepita: Sovereign AI Kernel Interfaces

**Tiny first-principles Rust kernel for bare-metal AI inference on embedded and edge devices.**

</p>

[![Rust](https://img.shields.io/badge/rust-100%25-orange.svg)](https://www.rust-lang.org/)
[![Zero C/C++](https://img.shields.io/badge/C%2FC%2B%2B-0%25-blue.svg)](#)
[![Tests](https://img.shields.io/badge/tests-417%20passing-green.svg)](#)

## Table of Contents

- [Installation](#installation)
- [Usage](#usage)
- [Contributing](#contributing)
- [License](#license)


**Pepita** is a pure Rust library providing minimal kernel interfaces and distributed computing primitives for Sovereign AI workloads. It combines low-level Linux kernel interfaces (`ublk`, `io_uring`, `blk-mq`) with high-level infrastructure (`scheduler`, `executor`, `vmm`, `simd`, `gpu`).

## Design Principles (Iron Lotus Framework)

- **First-Principles Rust**: Zero external dependencies in kernel mode
- **Pure Rust Sovereignty**: 100% auditable, zero C/C++ dependencies
- **Work-Stealing Scheduler**: Blumofe-Leiserson algorithm
- **Toyota Way Quality**: Jidoka, Poka-yoke, Genchi Genbutsu
- **EXTREME TDD**: 417 tests, comprehensive coverage

## Installation

```toml
[dependencies]
pepita = { path = "../pepita" }
```


## Usage

```rust
use pepita::scheduler::Scheduler;
use pepita::task::{Task, Priority};

// Create a work-stealing scheduler with 4 workers
let scheduler = Scheduler::new(4);

// Submit a task
let task = Task::new("compute")
    .with_priority(Priority::High);
scheduler.submit(task);
```

## Module Overview

### Core Kernel Interfaces (`no_std` compatible)

These modules define data structures compatible with the Linux kernel ABI, enabling userspace block devices and async I/O without any kernel modifications.

| Module | Purpose | Key Types |
|--------|---------|-----------|
| **`io_uring`** | Linux async I/O interface. Submit I/O operations and receive completions without syscall overhead per operation. | `IoUringSqe`, `IoUringCqe` |
| **`ublk`** | Userspace block device driver. Implement virtual disks entirely in userspace (like loop devices, but programmable). | `UblkCtrlCmd`, `UblkIoDesc`, `UblkIoCmd` |
| **`blk_mq`** | Multi-queue block layer. Manage parallel I/O queues for high-performance NVMe-style storage. | `TagSetConfig`, `Request`, `RequestOp` |
| **`memory`** | Physical/virtual memory management. DMA-safe allocations, page management, address translation. | `DmaBuffer`, `PageAllocator`, `Pfn`, `PhysAddr`, `VirtAddr` |
| **`error`** | Unified error types for all pepita operations. | `KernelError`, `Result` |

### Distributed Computing (`std` required)

These modules provide the runtime for executing tasks across CPU cores with work-stealing load balancing.

| Module | Purpose | Key Types |
|--------|---------|-----------|
| **`scheduler`** | Work-stealing scheduler (Blumofe-Leiserson). Each worker has a deque - pushes/pops from bottom, thieves steal from top. Provides automatic load balancing. | `Scheduler`, `WorkerDeque` |
| **`executor`** | Execution backends. Takes tasks and runs them on CPU threads, returning stdout/stderr/exit code. | `CpuExecutor`, `Backend` |
| **`task`** | Task definitions. Wraps a binary with arguments, environment, timeout, priority, and backend selection. | `Task`, `TaskId`, `ExecutionResult`, `Priority` |
| **`pool`** | High-level API combining scheduler + executor. Simple `submit(task)` interface for common use cases. | `Pool`, `PoolBuilder` |
| **`transport`** | Wire protocol for distributed communication. Message framing, length-prefixed serialization. | `Message`, `Transport` |
| **`fault`** | Fault tolerance primitives. Retry policies, circuit breakers, failure detection for distributed systems. | `RetryPolicy`, `CircuitBreaker` |

### Sovereign Infrastructure (`std` required)

These modules provide the building blocks for a complete Docker/Lambda/Kubernetes replacement in pure Rust.

| Module | Purpose | Key Types |
|--------|---------|-----------|
| **`zram`** | Compressed RAM block device. Stores pages in LZ4-compressed form in memory. Same-page deduplication, zero-page optimization. Typically 3-4x compression ratio. | `ZramDevice`, `ZramConfig`, `ZramCompressor`, `ZramStats` |
| **`vmm`** | KVM-based MicroVM runtime. Creates lightweight VMs with configurable vCPUs, memory, and kernel. Sub-100ms boot time. Used for serverless isolation. | `MicroVm`, `VmConfig`, `VmState`, `ExitReason` |
| **`virtio`** | Virtio device implementations for VM communication. Standard Linux virtio protocol for high-performance VM I/O. | `VirtQueue`, `VirtioVsock`, `VirtioBlock`, `VsockAddr` |
| **`simd`** | SIMD-accelerated vector operations. Auto-detects AVX-512/AVX2/SSE4.1/NEON and uses best available. | `SimdCapabilities`, `SimdOps`, `MatrixOps` |
| **`gpu`** | GPU compute via wgpu (Vulkan/Metal/DX12). Cross-platform GPU detection and compute shader execution. | `GpuDevice`, `ComputeKernel`, `GpuBuffer` |

## Module Details

### `io_uring` - Async I/O

```rust
use pepita::io_uring::{IoUringSqe, IoUringCqe, IORING_OP_URING_CMD};

// Submission queue entry - describes an I/O operation
let sqe = IoUringSqe::new(IORING_OP_URING_CMD, fd, addr, len);

// Completion queue entry - result of the operation
// user_data links back to the original submission
let cqe: IoUringCqe = /* from kernel */;
assert_eq!(cqe.res, 0); // Success
```

**Why it matters**: io_uring eliminates syscall overhead by batching I/O operations. One syscall can submit hundreds of operations and reap hundreds of completions.

### `ublk` - Userspace Block Devices

```rust
use pepita::ublk::{UblkCtrlCmd, UblkIoDesc, UBLK_U_CMD_ADD_DEV};

// Control command - add a new block device
let cmd = UblkCtrlCmd::new(UBLK_U_CMD_ADD_DEV, dev_id);

// I/O descriptor - describes a read/write request
let io_desc: UblkIoDesc = /* from kernel */;
let sector = io_desc.start_sector();
let num_sectors = io_desc.nr_sectors();
```

**Why it matters**: ublk allows implementing block devices (virtual disks, compressed storage, network-backed storage) entirely in userspace with near-native performance.

### `zram` - Compressed Memory

```rust
use pepita::zram::{ZramDevice, ZramConfig, ZramCompressor};

// Create a 1GB compressed RAM device
let config = ZramConfig::with_size(1024 * 1024 * 1024)
    .compressor(ZramCompressor::Lz4);
let device = ZramDevice::new(config)?;

// Write a page (4KB)
let data = [0u8; 4096];
device.write_page(0, &data)?;

// Check compression stats
let stats = device.stats();
println!("Compression ratio: {:.2}x", stats.compression_ratio());
println!("Zero pages (free): {}", stats.zero_pages);
```

**Why it matters**: zram provides swap/storage that lives in compressed RAM. A 4GB system can effectively have 12-16GB of memory for compressible workloads.

### `vmm` - MicroVM Runtime

```rust
use pepita::vmm::{MicroVm, VmConfig, VmState};

// Configure a MicroVM
let config = VmConfig::builder()
    .vcpus(2)
    .memory_mb(256)
    .kernel_path("/boot/vmlinuz")
    .build()?;

// Create and run
let vm = MicroVm::create(config)?;
assert_eq!(vm.state(), VmState::Created);

vm.start()?;
assert_eq!(vm.state(), VmState::Running);

// VM runs until exit
let exit_reason = vm.run()?;
```

**Why it matters**: MicroVMs provide hardware-level isolation (like Docker) with sub-100ms cold start (like Lambda). Each function runs in its own VM with dedicated vCPUs and memory.

### `virtio` - VM Device Communication

```rust
use pepita::virtio::{VirtioVsock, VirtioBlock, VsockAddr};

// Vsock - socket communication between VM and host
let vsock = VirtioVsock::new(3); // CID 3
vsock.activate();
vsock.connect(VsockAddr::host(8080))?;
vsock.send(b"Hello from VM")?;

// Block device - virtual disk for the VM
let block = VirtioBlock::new(1024 * 1024 * 1024); // 1GB
block.activate();
block.write(0, &sector_data)?;
```

**Why it matters**: virtio is the standard interface for high-performance VM I/O. Vsock enables networking without a virtual NIC, block devices provide storage.

### `simd` - Vector Operations

```rust
use pepita::simd::{SimdCapabilities, SimdOps, MatrixOps};

// Detect CPU capabilities
let caps = SimdCapabilities::detect();
println!("Best width: {}-bit", caps.best_vector_width()); // 512 for AVX-512

// Vector operations
let ops = SimdOps::new();
let a = vec![1.0f32; 1024];
let b = vec![2.0f32; 1024];
let mut c = vec![0.0f32; 1024];

ops.vadd_f32(&a, &b, &mut c);  // c = a + b (SIMD accelerated)
ops.vmul_f32(&a, &b, &mut c);  // c = a * b
let dot = ops.dot_f32(&a, &b); // dot product

// Matrix multiplication
let matrix_ops = MatrixOps::new();
matrix_ops.matmul_f32(&mat_a, &mat_b, &mut mat_c, m, k, n);
```

**Why it matters**: SIMD provides 4-16x speedup for numerical operations. AVX-512 processes 16 floats per instruction vs 1 for scalar code.

### `scheduler` - Work Stealing

```rust
use pepita::scheduler::Scheduler;
use pepita::task::{Task, Priority};

let scheduler = Scheduler::with_workers(4);

// Submit tasks with priorities
let task = Task::builder()
    .binary("./compute")
    .priority(Priority::High)
    .build()?;

scheduler.submit(task).await?;

// Work stealing happens automatically:
// - Idle workers steal from busy workers' queues
// - Provably optimal load balancing (Blumofe-Leiserson)
```

**Why it matters**: Work stealing provides automatic load balancing. If one worker finishes early, it steals work from others rather than sitting idle.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                           User Code                              │
└──────────────────────────────┬──────────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────────┐
│                          pool.rs                                 │
│                    (High-level Pool API)                         │
└──────────────────────────────┬──────────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────────┐
│                       scheduler.rs                               │
│              (Work-Stealing, Blumofe-Leiserson)                  │
└──────────────────────────────┬──────────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────────┐
│                       executor.rs                                │
│                    (Backend Dispatch)                            │
├─────────────┬─────────────┬─────────────┬───────────────────────┤
│   CPU       │    GPU      │   MicroVM   │        SIMD           │
│ (threads)   │  (wgpu)     │   (KVM)     │    (AVX/NEON)         │
└─────────────┴──────┬──────┴──────┬──────┴───────────┬───────────┘
                     │             │                  │
              ┌──────▼──────┐ ┌────▼─────┐    ┌───────▼───────┐
              │   gpu.rs    │ │  vmm.rs  │    │   simd.rs     │
              │   (wgpu)    │ │  (KVM)   │    │ (AVX-512/NEON)│
              └─────────────┘ └────┬─────┘    └───────────────┘
                                   │
                            ┌──────▼──────┐
                            │  virtio.rs  │
                            │(vsock,block)│
                            └─────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    Kernel Interfaces (no_std)                    │
├─────────────┬─────────────┬─────────────┬───────────────────────┤
│  io_uring   │    ublk     │   blk_mq    │       memory          │
│ (async I/O) │(block dev)  │ (multiqueue)│   (DMA/pages)         │
└─────────────┴─────────────┴─────────────┴───────────────────────┘
```

## Integration with Repartir

Pepita provides the low-level primitives that [repartir](../repartir) uses for its high-level distributed computing API:

```rust
// repartir uses pepita's SIMD executor
use repartir::executor::simd::{SimdExecutor, SimdTask};

let executor = SimdExecutor::new(); // Uses pepita::simd internally
let task = SimdTask::vadd_f32(a, b);
let result = executor.execute_simd(task).await?;

// repartir uses pepita's MicroVM for serverless
use repartir::executor::microvm::MicroVmExecutor;

let executor = MicroVmExecutor::new(config)?; // Uses pepita::vmm internally
```

## Test Results

```
running 417 tests
test result: ok. 417 passed; 0 failed; 0 ignored
```

## Contributing

Contributions are welcome! Please see the [CONTRIBUTING.md](CONTRIBUTING.md) guide for details.


## MSRV

Minimum Supported Rust Version: **1.75**

## See Also

- [Cookbook](examples/) — 1 runnable example

## License

MIT License - see [LICENSE](LICENSE) file for details.

---

**Built with the Iron Lotus Framework**
*Quality is not inspected in; it is built in.*
