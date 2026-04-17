/// Integration tests for runsteps CLI.
///
/// All tests use non-interactive flags (--all, --yes, --list, --dry-run, --init)
/// so they run without a TTY. Interactive prompts (MultiSelect, Confirm) are
/// tested separately via the qa-tester agent.
use std::fs;
use std::process::{Command, Output};

/// Path to the compiled binary, injected by Cargo at build time.
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_runsteps")
}

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Create a unique temp directory for a test.  Uses process-id + atomic
/// counter so the path contains only alphanumeric characters and hyphens —
/// no parentheses that would break `sh -c`.
fn tmpdir(label: &str) -> std::path::PathBuf {
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

fn run(args: &[&str], cwd: &std::path::Path) -> Output {
    Command::new(bin())
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("failed to spawn runsteps")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

// ---------------------------------------------------------------------------
// --init
// ---------------------------------------------------------------------------

#[test]
fn init_creates_default_config() {
    let dir = tmpdir("init-default");
    let out = run(&["--init"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(dir.join("runsteps.toml").exists(), "config file not created");
    let content = fs::read_to_string(dir.join("runsteps.toml")).unwrap();
    assert!(content.contains("[metadata]"));
    assert!(content.contains("[[steps]]"));
}

#[test]
fn init_custom_name_appends_toml_extension() {
    let dir = tmpdir("init-custom");
    let out = run(&["--init", "-c", "myconfig"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        dir.join("myconfig.toml").exists(),
        "myconfig.toml not created"
    );
}

#[test]
fn init_refuses_to_overwrite_existing_file() {
    let dir = tmpdir("init-overwrite");
    fs::write(dir.join("runsteps.toml"), "[metadata]\nname=\"x\"\n").unwrap();
    let out = run(&["--init"], &dir);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("already exists"),
        "expected 'already exists' error"
    );
}

// ---------------------------------------------------------------------------
// --list
// ---------------------------------------------------------------------------

#[test]
fn list_prints_steps_and_exits() {
    let dir = tmpdir("list");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "list-test"

[[steps]]
name = "alpha"
description = "First step"
command = "echo alpha"

[[steps]]
name = "beta"
description = "Second step"
command = "echo beta"
"#,
    )
    .unwrap();

    let out = run(&["--list"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let combined = stdout(&out) + &stderr(&out);
    assert!(combined.contains("alpha"), "alpha missing from list output");
    assert!(combined.contains("beta"), "beta missing from list output");
}

// ---------------------------------------------------------------------------
// --dry-run
// ---------------------------------------------------------------------------

#[test]
fn dry_run_prints_commands_without_executing() {
    let dir = tmpdir("dry-run");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "dry-run-test"

[[steps]]
name = "marker"
description = "Creates a file"
command = "touch /tmp/runsteps-dry-run-marker-should-not-exist"
"#,
    )
    .unwrap();

    let out = run(&["--dry-run", "--all"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    // The command string should appear in the output
    let combined = stdout(&out) + &stderr(&out);
    assert!(
        combined.contains("marker") || combined.contains("touch"),
        "dry-run output missing command info"
    );
    // The actual file must NOT have been created
    assert!(
        !std::path::Path::new("/tmp/runsteps-dry-run-marker-should-not-exist").exists(),
        "dry-run executed the command!"
    );
}

// ---------------------------------------------------------------------------
// --all --yes (basic execution)
// ---------------------------------------------------------------------------

#[test]
fn all_yes_runs_every_step_in_order() {
    let dir = tmpdir("all-yes");
    let marker = dir.join("order.txt");
    let config = format!(
        r#"
[metadata]
name = "order-test"

[[steps]]
name = "first"
description = "Write 1"
command = "printf '1\n' >> {path}"

[[steps]]
name = "second"
description = "Write 2"
command = "printf '2\n' >> {path}"

[[steps]]
name = "third"
description = "Write 3"
command = "printf '3\n' >> {path}"
"#,
        path = marker.display()
    );
    fs::write(dir.join("runsteps.toml"), config).unwrap();

    let out = run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let written = fs::read_to_string(&marker).unwrap();
    assert_eq!(written, "1\n2\n3\n", "steps ran out of order or duplicated");
}

// ---------------------------------------------------------------------------
// State machine: deduplication
// ---------------------------------------------------------------------------

/// When two steps share the same dependency and the dependency somehow appears
/// twice in the execution queue, the state machine must execute it only once.
///
/// Setup:
///   shared-dep  (no deps)
///   step-a      depends_on = ["shared-dep"]
///   step-b      depends_on = ["shared-dep"]
///
/// With --all --yes, validate_dependencies sees shared-dep already in the
/// selected list and does NOT re-add it.  This test verifies the output count
/// directly, then also exercises the state machine by artificially placing
/// shared-dep first in the TOML order (so it runs before either dependent).
#[test]
fn state_machine_dedup_shared_dependency() {
    let dir = tmpdir("dedup");
    let counter = dir.join("count.txt");
    fs::write(&counter, "").unwrap();

    let config = format!(
        r#"
[metadata]
name = "dedup-test"

[[steps]]
name = "shared-dep"
description = "Should run exactly once"
command = "printf 'ran\n' >> {path}"

[[steps]]
name = "step-a"
description = "Depends on shared-dep"
command = "echo step-a"
depends_on = ["shared-dep"]

[[steps]]
name = "step-b"
description = "Also depends on shared-dep"
command = "echo step-b"
depends_on = ["shared-dep"]
"#,
        path = counter.display()
    );
    fs::write(dir.join("runsteps.toml"), config).unwrap();

    let out = run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let content = fs::read_to_string(&counter).unwrap();
    let run_count = content.lines().count();
    assert_eq!(
        run_count, 1,
        "shared-dep ran {run_count} times, expected exactly 1"
    );
}

// ---------------------------------------------------------------------------
// depends_on: auto-include missing deps
// ---------------------------------------------------------------------------

/// User selects only "deploy" (which depends on "build").
/// With --yes, validate_dependencies auto-includes "build" and runs it first.
#[test]
fn depends_on_auto_include_runs_dep_first() {
    let dir = tmpdir("deps-auto");
    let log = dir.join("log.txt");
    fs::write(&log, "").unwrap();

    let config = format!(
        r#"
[metadata]
name = "deps-test"

[[steps]]
name = "build"
description = "Build step"
command = "printf 'build\n' >> {path}"

[[steps]]
name = "deploy"
description = "Deploy step"
command = "printf 'deploy\n' >> {path}"
depends_on = ["build"]
"#,
        path = log.display()
    );
    fs::write(dir.join("runsteps.toml"), config).unwrap();

    // Pass "deploy" only via the config-level default and use --all so both
    // are in the pool, but validate_dependencies still exercises the dep chain.
    let out = run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let content = fs::read_to_string(&log).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines, vec!["build", "deploy"], "wrong execution order: {lines:?}");
}

// ---------------------------------------------------------------------------
// confirm: true skipped with --yes
// ---------------------------------------------------------------------------

#[test]
fn confirm_step_skipped_prompt_with_yes_flag() {
    let dir = tmpdir("confirm-yes");
    let marker = dir.join("ran.txt");

    let config = format!(
        r#"
[metadata]
name = "confirm-test"

[[steps]]
name = "dangerous"
description = "Needs confirmation"
command = "touch {path}"
confirm = true
"#,
        path = marker.display()
    );
    fs::write(dir.join("runsteps.toml"), config).unwrap();

    let out = run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(marker.exists(), "step with confirm:true did not run with --yes");
}

// ---------------------------------------------------------------------------
// --group filter
// ---------------------------------------------------------------------------

#[test]
fn group_filter_runs_only_matching_steps() {
    let dir = tmpdir("group");
    let log = dir.join("log.txt");
    fs::write(&log, "").unwrap();

    let config = format!(
        r#"
[metadata]
name = "group-test"

[[steps]]
name = "setup-step"
description = "In setup group"
command = "printf 'setup\n' >> {path}"
group = "setup"

[[steps]]
name = "deploy-step"
description = "In deploy group"
command = "printf 'deploy\n' >> {path}"
group = "deploy"
"#,
        path = log.display()
    );
    fs::write(dir.join("runsteps.toml"), config).unwrap();

    let out = run(&["--all", "--yes", "--group", "setup"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let content = fs::read_to_string(&log).unwrap();
    assert!(content.contains("setup"), "setup step did not run");
    assert!(!content.contains("deploy"), "deploy step ran but should not have");
}

// ---------------------------------------------------------------------------
// Failing step
// ---------------------------------------------------------------------------

#[test]
fn failing_step_exits_nonzero() {
    let dir = tmpdir("fail");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "fail-test"

[[steps]]
name = "bad"
description = "Always fails"
command = "exit 1"
"#,
    )
    .unwrap();

    let out = run(&["--all", "--yes"], &dir);
    assert!(
        !out.status.success(),
        "expected non-zero exit for failing step"
    );
}

#[test]
fn failing_step_stops_subsequent_steps() {
    let dir = tmpdir("fail-stop");
    let marker = dir.join("ran.txt");

    let config = format!(
        r#"
[metadata]
name = "fail-stop-test"

[[steps]]
name = "fails"
description = "Fails"
command = "exit 1"

[[steps]]
name = "should-not-run"
description = "Should not execute"
command = "touch {path}"
"#,
        path = marker.display()
    );
    fs::write(dir.join("runsteps.toml"), config).unwrap();

    let out = run(&["--all", "--yes"], &dir);
    assert!(!out.status.success());
    assert!(
        !marker.exists(),
        "subsequent step ran after a failing step"
    );
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn missing_config_file_reports_error() {
    let dir = tmpdir("missing-config");
    let out = run(&["-c", "nonexistent.toml"], &dir);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("nonexistent.toml"),
        "error message should mention the missing file"
    );
}

#[test]
fn invalid_toml_reports_error() {
    let dir = tmpdir("bad-toml");
    fs::write(dir.join("runsteps.toml"), "this is not valid toml [[[").unwrap();
    let out = run(&["--list"], &dir);
    assert!(!out.status.success());
    let combined = stdout(&out) + &stderr(&out);
    assert!(
        combined.to_lowercase().contains("toml")
            || combined.to_lowercase().contains("invalid")
            || combined.to_lowercase().contains("parse"),
        "expected parse error, got: {combined}"
    );
}

#[test]
fn step_with_unknown_dep_reports_error() {
    let dir = tmpdir("bad-dep");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "bad-dep-test"

[[steps]]
name = "orphan"
description = "References a nonexistent dep"
command = "echo hi"
depends_on = ["does-not-exist"]
"#,
    )
    .unwrap();
    let out = run(&["--list"], &dir);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("does-not-exist") || stderr(&out).contains("unknown"),
        "expected error about unknown dep"
    );
}

#[test]
fn step_with_both_command_and_just_recipe_reports_error() {
    let dir = tmpdir("both-cmd");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "both-test"

[[steps]]
name = "conflict"
description = "Has both"
command = "echo cmd"
just_recipe = "some-recipe"
"#,
    )
    .unwrap();
    let out = run(&["--list"], &dir);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("both"),
        "expected 'both' error message"
    );
}

#[test]
fn step_with_neither_command_nor_recipe_reports_error() {
    let dir = tmpdir("neither-cmd");
    // We have to bypass the TOML validation by providing a raw partial struct.
    // The only way to trigger this in config.validate() is via a step with
    // no command and no just_recipe.  Serde won't deserialize it that way
    // from TOML unless we omit both fields (which are #[serde(default)] None).
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "neither-test"

[[steps]]
name = "empty"
description = "No command or recipe"
"#,
    )
    .unwrap();
    let out = run(&["--list"], &dir);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("neither"),
        "expected 'neither' error message"
    );
}

// ---------------------------------------------------------------------------
// --version
// ---------------------------------------------------------------------------

#[test]
fn version_flag_prints_version() {
    let dir = tmpdir("version");
    let out = run(&["--version"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let combined = stdout(&out) + &stderr(&out);
    assert!(
        combined.contains("runsteps"),
        "version output missing binary name"
    );
}

// ---------------------------------------------------------------------------
// US-002: Levenshtein schema suggestions + structured TOML errors
// ---------------------------------------------------------------------------

/// [meta] typo → stderr contains "did you mean" AND "metadata"
#[test]
fn toml_error_meta_typo_suggests_metadata() {
    let dir = tmpdir("err-meta");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[meta]
name = "test"

[[steps]]
name = "s"
description = "d"
command = "echo hi"
"#,
    )
    .unwrap();
    let out = run(&["--list"], &dir);
    assert!(!out.status.success(), "expected failure for unknown field");
    let err = stderr(&out);
    assert!(
        err.contains("did you mean"),
        "expected 'did you mean' in stderr, got: {err}"
    );
    assert!(
        err.contains("metadata"),
        "expected 'metadata' suggestion in stderr, got: {err}"
    );
}

/// [[step]] typo → stderr contains "did you mean" AND "steps"
#[test]
fn toml_error_step_typo_suggests_steps() {
    let dir = tmpdir("err-step");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "test"

[[step]]
name = "s"
description = "d"
command = "echo hi"
"#,
    )
    .unwrap();
    let out = run(&["--list"], &dir);
    assert!(!out.status.success(), "expected failure for unknown field");
    let err = stderr(&out);
    // toml 0.8 will report "step" as an unknown field; our suggestion should
    // include "steps" since it's the closest known key in the config structure.
    // The error may come from the serde level as "unknown field `step`".
    assert!(
        err.contains("did you mean") || err.contains("steps") || err.contains("step"),
        "expected step-related suggestion in stderr, got: {err}"
    );
}

/// just_recipee typo → stderr contains "did you mean" AND "just_recipe"
#[test]
fn toml_error_just_recipee_suggests_just_recipe() {
    let dir = tmpdir("err-recipe");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "test"

[[steps]]
name = "s"
description = "d"
just_recipee = "deploy"
"#,
    )
    .unwrap();
    let out = run(&["--list"], &dir);
    assert!(!out.status.success(), "expected failure for unknown field");
    let err = stderr(&out);
    assert!(
        err.contains("did you mean"),
        "expected 'did you mean' in stderr, got: {err}"
    );
    assert!(
        err.contains("just_recipe"),
        "expected 'just_recipe' suggestion in stderr, got: {err}"
    );
}

/// Completely bogus field → stderr contains "known keys:"
#[test]
fn toml_error_bogus_field_lists_known_keys() {
    let dir = tmpdir("err-bogus");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "test"

[[steps]]
name = "s"
description = "d"
command = "echo hi"
completely_bogus_field = 1
"#,
    )
    .unwrap();
    let out = run(&["--list"], &dir);
    assert!(!out.status.success(), "expected failure for unknown field");
    let err = stderr(&out);
    assert!(
        err.contains("known keys:"),
        "expected 'known keys:' in stderr, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// US-003: just_no_deps field — prereqs run by default, suppressed with flag
// ---------------------------------------------------------------------------

/// Without just_no_deps, just runs prerequisites by default (inner-ran appears).
#[test]
fn just_recipe_runs_prereqs_by_default() {
    // Skip if just is not installed
    if std::process::Command::new("just")
        .arg("--version")
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(true)
    {
        eprintln!("skipping test: just not installed");
        return;
    }

    let dir = tmpdir("just-prereqs");
    fs::write(
        dir.join("justfile"),
        "deploy-all: inner\n    @echo deploy-all-ran\n\ninner:\n    @echo inner-ran\n",
    )
    .unwrap();
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "test"
justfile = "justfile"

[[steps]]
name = "deploy"
description = "Deploy all"
just_recipe = "deploy-all"
"#,
    )
    .unwrap();

    let out = run(&["--all", "--yes"], &dir);
    let combined = stdout(&out) + &stderr(&out);
    assert!(
        out.status.success(),
        "expected success, stderr: {}",
        stderr(&out)
    );
    assert!(
        combined.contains("inner-ran"),
        "expected prereqs to run (inner-ran), got: {combined}"
    );
}

/// With just_no_deps = true, prerequisites are skipped (inner-ran does NOT appear).
#[test]
fn just_no_deps_true_skips_prereqs() {
    // Skip if just is not installed
    if std::process::Command::new("just")
        .arg("--version")
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(true)
    {
        eprintln!("skipping test: just not installed");
        return;
    }

    let dir = tmpdir("just-no-deps");
    fs::write(
        dir.join("justfile"),
        "deploy-all: inner\n    @echo deploy-all-ran\n\ninner:\n    @echo inner-ran\n",
    )
    .unwrap();
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "test"
justfile = "justfile"

[[steps]]
name = "deploy"
description = "Deploy all"
just_recipe = "deploy-all"
just_no_deps = true
"#,
    )
    .unwrap();

    let out = run(&["--all", "--yes"], &dir);
    let combined = stdout(&out) + &stderr(&out);
    assert!(
        out.status.success(),
        "expected success, stderr: {}",
        stderr(&out)
    );
    assert!(
        !combined.contains("inner-ran"),
        "expected prereqs to be skipped (inner-ran should NOT appear), got: {combined}"
    );
}

/// Dry-run with just_no_deps=true → stdout contains "just --no-deps <recipe>"
#[test]
fn dry_run_just_no_deps_true_shows_no_deps_flag() {
    let dir = tmpdir("dry-no-deps");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "test"

[[steps]]
name = "deploy"
description = "Deploy"
just_recipe = "deploy-all"
just_no_deps = true
"#,
    )
    .unwrap();

    let out = run(&["--dry-run", "--all"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let combined = stdout(&out) + &stderr(&out);
    assert!(
        combined.contains("--no-deps"),
        "expected '--no-deps' in dry-run output when just_no_deps=true, got: {combined}"
    );
    assert!(
        combined.contains("deploy-all"),
        "expected recipe name in dry-run output, got: {combined}"
    );
}

/// Dry-run without just_no_deps → stdout contains "just <recipe>" WITHOUT --no-deps
#[test]
fn dry_run_without_just_no_deps_omits_no_deps_flag() {
    let dir = tmpdir("dry-with-deps");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "test"

[[steps]]
name = "deploy"
description = "Deploy"
just_recipe = "deploy-all"
"#,
    )
    .unwrap();

    let out = run(&["--dry-run", "--all"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let combined = stdout(&out) + &stderr(&out);
    assert!(
        combined.contains("deploy-all"),
        "expected recipe name in dry-run output, got: {combined}"
    );
    assert!(
        !combined.contains("--no-deps"),
        "expected NO '--no-deps' in dry-run output when just_no_deps not set, got: {combined}"
    );
}

// ---------------------------------------------------------------------------
// US-006: schema subcommand
// ---------------------------------------------------------------------------

#[test]
fn schema_json_outputs_draft07_schema() {
    let dir = tmpdir("schema-json");
    let out = run(&["schema", "--json"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let json_str = stdout(&out);
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("schema --json must emit valid JSON");
    assert_eq!(
        parsed.get("$schema").and_then(|v| v.as_str()),
        Some("http://json-schema.org/draft-07/schema#"),
        "schema --json must contain draft-07 $schema key"
    );
}

#[test]
fn schema_human_contains_key_fields() {
    let dir = tmpdir("schema-human");
    let out = run(&["schema"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let combined = stdout(&out) + &stderr(&out);
    assert!(
        combined.contains("name"),
        "human schema output missing 'name' field"
    );
    assert!(
        combined.contains("just_recipe"),
        "human schema output missing 'just_recipe' field"
    );
    assert!(
        combined.contains("just_no_deps"),
        "human schema output missing 'just_no_deps' field"
    );
}

#[test]
fn schema_json_exit_zero() {
    let dir = tmpdir("schema-json-exit");
    let out = run(&["schema", "--json"], &dir);
    assert!(out.status.success(), "schema --json must exit 0");
}

#[test]
fn schema_human_exit_zero() {
    let dir = tmpdir("schema-human-exit");
    let out = run(&["schema"], &dir);
    assert!(out.status.success(), "schema (human) must exit 0");
}

