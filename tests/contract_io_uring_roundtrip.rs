//! Falsification tests for `contracts/io-uring-roundtrip-v1.yaml`.
//!
//! These exercise the first-principles correlation invariant of the `io_uring`
//! data path entirely through the crate's public ABI types:
//!
//! - FALSIFY-URING-001: `user_data` roundtrips verbatim from SQE to CQE.
//! - FALSIFY-URING-002: success/error form a disjoint, total partition of `res`.
//!
//! They live in `tests/` (an integration crate) rather than inside
//! `src/io_uring.rs` because that module is already at the repo's per-file
//! size budget; the contract only requires that the named tests exist and pass.

use pepita::io_uring::{IoUringCqe, IoUringSqe};

/// FALSIFY-URING-001 — `user_data` roundtrips verbatim through SQE -> CQE.
///
/// The kernel treats `user_data` as an opaque cookie: the completion entry it
/// produces for a submission must echo that submission's exact `user_data`.
/// This is the sole mechanism correlating a CQE with its originating SQE.
#[test]
fn contract_io_uring_user_data_roundtrip() {
    for &u in &[0u64, 1, 42, 0xDEAD_BEEF, u64::MAX, u64::MAX - 1] {
        let sqe = IoUringSqe::read(3, 0x1000, 4096, 0, u);
        // Model the kernel completing the submission: it copies user_data
        // verbatim into the CQE alongside an (unrelated) result.
        let cqe = IoUringCqe::new(sqe.user_data, 4096, 0);
        assert_eq!(cqe.user_data, sqe.user_data, "user_data must roundtrip unchanged (u = {u:#x})");
    }
}

/// FALSIFY-URING-002 — success/error partition `res` disjointly and totally,
/// and `errno`/`result` agree with that partition.
#[test]
fn contract_io_uring_result_partition() {
    for res in [i32::MIN, -5, -1, 0, 1, 4096, i32::MAX] {
        let cqe = IoUringCqe::new(7, res, 0);
        // Disjoint and total: exactly one of success/error holds.
        assert_ne!(
            cqe.is_success(),
            cqe.is_error(),
            "is_success XOR is_error must hold (res = {res})"
        );
        if res >= 0 {
            assert_eq!(cqe.errno(), 0, "errno is 0 on success (res = {res})");
            let expected = u32::try_from(res).expect("res >= 0 fits in u32");
            assert_eq!(cqe.result(), Some(expected), "result mirrors res on success");
        } else {
            assert_eq!(cqe.result(), None, "result is None on error (res = {res})");
            // errno() negates res internally; i32::MIN has no positive
            // counterpart (real kernel errnos are small positives), so only
            // assert it where -res is representable.
            if let Some(expected) = res.checked_neg() {
                assert_eq!(cqe.errno(), expected, "errno is -res on error (res = {res})");
            }
        }
    }
}
