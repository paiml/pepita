//! Pepita CLI - Kernel interface verification tool.
//!
//! This binary provides utilities for verifying ABI compatibility
//! and testing kernel interface structures.

use pepita::{
    blk_mq::{Request, RequestOp, TagSetConfig, BLK_MQ_MAX_DEPTH, BLK_MQ_MAX_HW_QUEUES},
    io_uring::{IoUringCqe, IoUringSqe, IORING_OP_NOP, IORING_OP_READ, IORING_OP_URING_CMD},
    memory::{PhysAddr, VirtAddr, PAGE_SIZE},
    ublk::{UblkCtrlCmd, UblkIoCmd, UblkIoDesc, UBLK_U_CMD_ADD_DEV},
};

fn main() {
    println!("Pepita - Tiny First-Principles Rust Kernel Interfaces");
    println!("======================================================");
    println!();

    print_struct_info();
    print_constants();
    verify_abi();
}

/// Print struct size information.
fn print_struct_info() {
    println!("Struct Sizes (ABI Verification):");
    println!("  UblkCtrlCmd:  {} bytes (expected: 32)", size_of::<UblkCtrlCmd>());
    println!("  UblkIoDesc:   {} bytes (expected: 24)", size_of::<UblkIoDesc>());
    println!("  UblkIoCmd:    {} bytes (expected: 16)", size_of::<UblkIoCmd>());
    println!("  IoUringSqe:   {} bytes (expected: 64)", size_of::<IoUringSqe>());
    println!("  IoUringCqe:   {} bytes (expected: 16)", size_of::<IoUringCqe>());
    println!("  Request:      {} bytes", size_of::<Request>());
    println!("  PhysAddr:     {} bytes", size_of::<PhysAddr>());
    println!("  VirtAddr:     {} bytes", size_of::<VirtAddr>());
    println!();
}

/// Print important constants.
fn print_constants() {
    println!("Kernel Constants:");
    println!("  PAGE_SIZE:          {}", PAGE_SIZE);
    println!("  BLK_MQ_MAX_DEPTH:   {}", BLK_MQ_MAX_DEPTH);
    println!("  BLK_MQ_MAX_HW_QUEUES: {}", BLK_MQ_MAX_HW_QUEUES);
    println!("  IORING_OP_NOP:      {}", IORING_OP_NOP);
    println!("  IORING_OP_READ:     {}", IORING_OP_READ);
    println!("  IORING_OP_URING_CMD: {}", IORING_OP_URING_CMD);
    println!("  UBLK_U_CMD_ADD_DEV: 0x{:08x}", UBLK_U_CMD_ADD_DEV);
    println!();
}

/// Verify ABI compatibility.
fn verify_abi() {
    println!("ABI Verification:");

    // Verify struct sizes match Linux kernel expectations
    let ublk_ctrl_ok = size_of::<UblkCtrlCmd>() == 32;
    let ublk_io_desc_ok = size_of::<UblkIoDesc>() == 24;
    let ublk_io_cmd_ok = size_of::<UblkIoCmd>() == 16;
    let sqe_ok = size_of::<IoUringSqe>() == 64;
    let cqe_ok = size_of::<IoUringCqe>() == 16;

    println!("  UblkCtrlCmd size: {}", if ublk_ctrl_ok { "OK" } else { "FAIL" });
    println!("  UblkIoDesc size:  {}", if ublk_io_desc_ok { "OK" } else { "FAIL" });
    println!("  UblkIoCmd size:   {}", if ublk_io_cmd_ok { "OK" } else { "FAIL" });
    println!("  IoUringSqe size:  {}", if sqe_ok { "OK" } else { "FAIL" });
    println!("  IoUringCqe size:  {}", if cqe_ok { "OK" } else { "FAIL" });
    println!();

    // Verify alignment
    let phys_align_ok = align_of::<PhysAddr>() == 8;
    let virt_align_ok = align_of::<VirtAddr>() == 8;
    let sqe_align_ok = align_of::<IoUringSqe>() == 8;

    println!("Alignment Verification:");
    println!("  PhysAddr align:   {}", if phys_align_ok { "OK" } else { "FAIL" });
    println!("  VirtAddr align:   {}", if virt_align_ok { "OK" } else { "FAIL" });
    println!("  IoUringSqe align: {}", if sqe_align_ok { "OK" } else { "FAIL" });
    println!();

    // Test struct construction
    println!("Construction Tests:");
    let ctrl = UblkCtrlCmd::new(0);
    println!("  UblkCtrlCmd: dev_id={}", ctrl.dev_id());

    let sqe = IoUringSqe::nop(0);
    println!("  IoUringSqe::nop: opcode={}", sqe.opcode);

    let req = Request::new(0, 0, RequestOp::Read);
    println!("  Request: op={:?}, tag={}", req.op(), req.tag());

    let config = TagSetConfig::new(4, 128);
    match config.validate() {
        Ok(()) => println!("  TagSetConfig: valid"),
        Err(e) => println!("  TagSetConfig: invalid - {}", e),
    }
    println!();

    // Summary
    let all_ok = ublk_ctrl_ok && ublk_io_desc_ok && ublk_io_cmd_ok && sqe_ok && cqe_ok
        && phys_align_ok && virt_align_ok && sqe_align_ok;

    if all_ok {
        println!("All ABI checks passed!");
    } else {
        println!("WARNING: Some ABI checks failed!");
        std::process::exit(1);
    }
}

fn size_of<T>() -> usize {
    core::mem::size_of::<T>()
}

fn align_of<T>() -> usize {
    core::mem::align_of::<T>()
}
