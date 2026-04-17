mod common;

/// Tests for top-level CLI behaviour: --version, --help, and US-016
/// (removed legacy --list / --init flags).

#[test]
fn version_flag_prints_version() {
    let dir = common::tmpdir("version");
    let out = common::run(&["--version"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let combined = common::stdout(&out) + &common::stderr(&out);
    assert!(
        combined.contains("runsteps"),
        "version output missing binary name"
    );
}

// ---------------------------------------------------------------------------
// US-016: legacy top-level --list and --init flags are removed
// ---------------------------------------------------------------------------

#[test]
fn legacy_list_flag_exits_nonzero_with_unexpected_argument() {
    let dir = common::tmpdir("us016-list");
    let out = common::run(&["--list"], &dir);
    assert!(
        !out.status.success(),
        "expected nonzero exit for removed --list flag"
    );
    let err = common::stderr(&out);
    assert!(
        err.contains("unexpected argument") || err.contains("--list"),
        "expected 'unexpected argument' in stderr, got: {err}"
    );
}

#[test]
fn legacy_init_flag_exits_nonzero_with_unexpected_argument() {
    let dir = common::tmpdir("us016-init");
    let out = common::run(&["--init"], &dir);
    assert!(
        !out.status.success(),
        "expected nonzero exit for removed --init flag"
    );
    let err = common::stderr(&out);
    assert!(
        err.contains("unexpected argument") || err.contains("--init"),
        "expected 'unexpected argument' in stderr, got: {err}"
    );
}
