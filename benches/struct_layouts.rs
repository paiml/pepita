#![allow(clippy::borrow_as_ptr)]
//! Struct layout benchmarks for Pepita kernel interfaces.
//!
//! These benchmarks verify that struct operations are zero-cost abstractions
//! and that layout calculations are optimized at compile time.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pepita::{
    blk_mq::{Request, RequestOp, TagSetConfig},
    io_uring::{IoUringCqe, IoUringSqe},
    memory::{DmaBuffer, PhysAddr, VirtAddr},
    ublk::{UblkCtrlCmd, UblkIoCmd, UblkIoDesc},
};

/// Benchmark struct construction overhead.
fn bench_struct_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("struct_construction");

    group.bench_function("UblkCtrlCmd::new", |b| {
        b.iter(|| {
            let cmd = UblkCtrlCmd::new(black_box(0));
            black_box(cmd)
        });
    });

    group.bench_function("UblkIoDesc::new", |b| {
        b.iter(|| {
            let desc = UblkIoDesc::new(black_box(0), black_box(1), black_box(8));
            black_box(desc)
        });
    });

    group.bench_function("IoUringSqe::nop", |b| {
        b.iter(|| {
            let sqe = IoUringSqe::nop(black_box(0));
            black_box(sqe)
        });
    });

    group.bench_function("IoUringSqe::read", |b| {
        b.iter(|| {
            let sqe = IoUringSqe::read(
                black_box(3),
                black_box(0x1000),
                black_box(4096),
                black_box(0),
                black_box(0),
            );
            black_box(sqe)
        });
    });

    group.bench_function("Request::new", |b| {
        b.iter(|| {
            let req = Request::new(black_box(0), black_box(0), black_box(RequestOp::Read));
            black_box(req)
        });
    });

    group.finish();
}

/// Benchmark address type operations.
fn bench_address_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("address_operations");

    group.bench_function("PhysAddr::new", |b| {
        b.iter(|| {
            let addr = PhysAddr::new(black_box(0x1000_0000));
            black_box(addr)
        });
    });

    group.bench_function("PhysAddr::page_align_down", |b| {
        let addr = PhysAddr::new(0x1234_5678);
        b.iter(|| {
            let aligned = black_box(addr).page_align_down();
            black_box(aligned)
        });
    });

    group.bench_function("PhysAddr::as_u64", |b| {
        let addr = PhysAddr::new(0x1000_0000);
        b.iter(|| {
            let val = black_box(addr).as_u64();
            black_box(val)
        });
    });

    group.bench_function("VirtAddr::new", |b| {
        b.iter(|| {
            let addr = VirtAddr::new(black_box(0xffff_8000_0000_0000));
            black_box(addr)
        });
    });

    group.finish();
}

/// Benchmark DMA buffer operations.
fn bench_dma_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("dma_buffer");

    group.bench_function("DmaBuffer::new", |b| {
        b.iter(|| {
            let buf = DmaBuffer::new(
                black_box(PhysAddr::new(0x1000_0000)),
                black_box(VirtAddr::new(0xffff_8000_0000_0000)),
                black_box(4096),
                pepita::memory::DmaDirection::Bidirectional,
            );
            black_box(buf)
        });
    });

    group.finish();
}

/// Benchmark configuration validation.
fn bench_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation");

    group.bench_function("TagSetConfig::validate_valid", |b| {
        let config = TagSetConfig::new(4, 128);
        b.iter(|| {
            let result = black_box(&config).validate();
            black_box(result)
        });
    });

    group.bench_function("TagSetConfig::validate_invalid", |b| {
        let config = TagSetConfig::new(0, 128); // Invalid - 0 queues
        b.iter(|| {
            let result = black_box(&config).validate();
            black_box(result)
        });
    });

    group.finish();
}

/// Benchmark struct sizes (compile-time verification).
fn bench_struct_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("struct_sizes");

    group.bench_function("size_of_all_structs", |b| {
        b.iter(|| {
            let sizes = (
                core::mem::size_of::<UblkCtrlCmd>(),
                core::mem::size_of::<UblkIoDesc>(),
                core::mem::size_of::<UblkIoCmd>(),
                core::mem::size_of::<IoUringSqe>(),
                core::mem::size_of::<IoUringCqe>(),
                core::mem::size_of::<Request>(),
                core::mem::size_of::<PhysAddr>(),
                core::mem::size_of::<VirtAddr>(),
                core::mem::size_of::<DmaBuffer>(),
            );
            black_box(sizes)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_struct_construction,
    bench_address_operations,
    bench_dma_buffer,
    bench_validation,
    bench_struct_sizes,
);
criterion_main!(benches);
