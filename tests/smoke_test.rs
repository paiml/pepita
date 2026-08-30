#![allow(clippy::borrow_as_ptr)]
//! Smoke tests for basic functionality

#[test]
fn test_version_exists() {
    // Verify the crate version string is valid semver
    let version = env!("CARGO_PKG_VERSION");
    assert!(!version.is_empty());
    let parts: Vec<&str> = version.split('.').collect();
    assert_eq!(parts.len(), 3, "Version should be semver: {version}");
}

#[test]
fn test_package_name() {
    let name = env!("CARGO_PKG_NAME");
    assert!(!name.is_empty());
}

// ── CLI argv handling (paiml/infra#396) ────────────────────────────────────
//
// `pepita <anything>` used to fall through to the full ABI verification and
// exit 0, so a typo in a script bought a green exit and 999 bytes of a
// plausible report. These assert the three states the binary has, and the
// unknown-argument one is the regression: revert `other =>` to `_ => {}` in
// src/main.rs and `unknown_argument_is_a_usage_error` fails on the status.

fn pepita() -> std::process::Command {
    std::process::Command::new(env!("CARGO_BIN_EXE_pepita"))
}

#[test]
fn unknown_argument_is_a_usage_error() {
    let out = pepita().arg("zzz-notacommand").output().expect("run pepita");
    assert_eq!(
        out.status.code(),
        Some(2),
        "an unrecognised argument must exit non-zero; got {:?}",
        out.status.code()
    );
    assert!(
        out.stdout.is_empty(),
        "a failing invocation must not write to stdout, so `pepita bad > out` leaves out empty; got {} bytes",
        out.stdout.len()
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("zzz-notacommand"),
        "the diagnostic must name the offending argument; stderr was: {err}"
    );
}

#[test]
fn version_and_help_still_succeed() {
    for flag in ["--version", "-V"] {
        let out = pepita().arg(flag).output().expect("run pepita");
        assert!(out.status.success(), "`pepita {flag}` must exit 0");
        assert!(
            String::from_utf8_lossy(&out.stdout).contains(env!("CARGO_PKG_VERSION")),
            "`pepita {flag}` must print the crate version on stdout"
        );
    }
    for flag in ["--help", "-h"] {
        let out = pepita().arg(flag).output().expect("run pepita");
        assert!(out.status.success(), "`pepita {flag}` must exit 0");
        assert!(
            String::from_utf8_lossy(&out.stdout).contains("Usage: pepita"),
            "`pepita {flag}` must print usage on stdout"
        );
    }
}

#[test]
fn no_arguments_still_runs_the_verification() {
    let out = pepita().output().expect("run pepita");
    assert!(out.status.success(), "bare `pepita` must exit 0");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("ABI Verification"),
        "bare `pepita` must still perform the full ABI verification"
    );
}
