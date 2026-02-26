# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Domain-specific Verus verification specs for kernel interfaces
- Makefile targets for verification and quality gates
- Fixed Cargo.toml duplicate key issue

## [0.1.0] - 2026-01-04

### Added
- Initial release of pepita: tiny first-principles Rust kernel for Sovereign AI
- `no_std` compatible kernel interfaces for io_uring, ublk, and blk-mq
- Core kernel modules:
  - `ublk` - Userspace block device driver interfaces
  - `virtio` - VirtIO device abstraction layer
  - `transport` - Zero-copy transport primitives
  - `scheduler` - Cooperative task scheduler
  - `pool` - Lock-free memory pool allocator
  - `simd` - SIMD-accelerated data path operations
  - `task` - Async task primitives for kernel workloads
  - `vmm` - Virtual machine monitor interfaces
  - `zram` - Compressed RAM block device support
- Property-based testing for all kernel interfaces
- MIT OR Apache-2.0 dual licensing

[Unreleased]: https://github.com/paiml/pepita/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/paiml/pepita/releases/tag/v0.1.0
