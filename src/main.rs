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

/// The usage text, as a string, so the same bytes can go to stdout for
/// `--help` (where it is the requested output) and to stderr for a usage
/// error (where stdout belongs to whatever the caller was redirecting).
fn usage() -> String {
    format!(
        "pepita {} — Sovereign AI kernel interface verification\n\
         \n\
         Usage: pepita [OPTIONS]\n\
         \n\
         Options:\n\
         \x20 -V, --version  Print version\n\
         \x20 -h, --help     Print help\n\
         \n\
         Run without arguments to perform full ABI verification.",
        env!("CARGO_PKG_VERSION")
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--version" | "-V" => {
                println!("pepita {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            // ANYTHING ELSE IS A MISTAKE, AND IT HAS TO COST SOMETHING.
            //
            // This arm used to be `_ => {}`, falling through to the full ABI
            // verification: `pepita zzz-notacommand` printed 999 bytes of a
            // successful-looking report to stdout and exited 0. A typo in a
            // script — `pepita --verify` for `pepita`, a stale flag, a
            // subcommand from a version that does not exist yet — bought a
            // green exit and a plausible report, so the script carried on as
            // though the command it MEANT to run had run.
            //
            // Found by the nightly crux audit's C1 check
            // (exit-nonzero-on-garbage): paiml/infra#396.
            //
            // The diagnostic goes to STDERR and the exit code is 2, the usual
            // usage-error code, so `pepita > abi.txt` still writes only the
            // report on the success path and writes nothing at all on this one.
            other => {
                eprintln!("pepita: unrecognised argument '{other}'");
                eprintln!();
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
    }

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
    println!("  PAGE_SIZE:          {PAGE_SIZE}");
    println!("  BLK_MQ_MAX_DEPTH:   {BLK_MQ_MAX_DEPTH}");
    println!("  BLK_MQ_MAX_HW_QUEUES: {BLK_MQ_MAX_HW_QUEUES}");
    println!("  IORING_OP_NOP:      {IORING_OP_NOP}");
    println!("  IORING_OP_READ:     {IORING_OP_READ}");
    println!("  IORING_OP_URING_CMD: {IORING_OP_URING_CMD}");
    println!("  UBLK_U_CMD_ADD_DEV: 0x{UBLK_U_CMD_ADD_DEV:08x}");
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
        Err(e) => println!("  TagSetConfig: invalid - {e}"),
    }
    println!();

    // Summary
    let all_ok = ublk_ctrl_ok
        && ublk_io_desc_ok
        && ublk_io_cmd_ok
        && sqe_ok
        && cqe_ok
        && phys_align_ok
        && virt_align_ok
        && sqe_align_ok;

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
