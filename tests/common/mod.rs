#![allow(dead_code)]
//! Shared test helpers for runsteps integration tests.
use std::fs;
use std::process::{Command, Output};

/// Path to the compiled binary, injected by Cargo at build time.
pub fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_runsteps")
}

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Create a unique temp directory for a test.
pub fn tmpdir(label: &str) -> std::path::PathBuf {
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "runsteps-{}-{}-{}",
        label,
        std::process::id(),
        n
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Run the binary with the given args from the given working directory.
pub fn run(args: &[&str], cwd: &std::path::Path) -> Output {
    Command::new(bin())
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("failed to spawn runsteps")
}

/// Run the binary with an isolated `RUNSTEPS_CACHE_DIR` so history tests
/// do not interfere with each other or with real user history.
pub fn run_with_cache(args: &[&str], cwd: &std::path::Path, cache_dir: &std::path::Path) -> Output {
    Command::new(bin())
        .args(args)
        .current_dir(cwd)
        .env("RUNSTEPS_CACHE_DIR", cache_dir)
        .output()
        .expect("failed to spawn runsteps")
}

/// Extract stdout as a String.
pub fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// Extract stderr as a String.
pub fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// Assert stderr contains the given substring.
pub fn assert_stderr_contains(o: &Output, needle: &str) {
    let err = stderr(o);
    assert!(
        err.contains(needle),
        "expected stderr to contain {:?}, got: {}",
        needle,
        err
    );
}
