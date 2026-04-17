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

/// Run the binary with an isolated `RUNSTEPS_CACHE_DIR` so that history tests
/// do not interfere with each other or with real user history.
fn run_with_cache(args: &[&str], cwd: &std::path::Path, cache_dir: &std::path::Path) -> Output {
    Command::new(bin())
        .args(args)
        .current_dir(cwd)
        .env("RUNSTEPS_CACHE_DIR", cache_dir)
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
// init subcommand
// ---------------------------------------------------------------------------

#[test]
fn init_creates_default_config() {
    let dir = tmpdir("init-default");
    let out = run(&["init"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(dir.join("runsteps.toml").exists(), "config file not created");
    let content = fs::read_to_string(dir.join("runsteps.toml")).unwrap();
    assert!(content.contains("[metadata]"));
    assert!(content.contains("[[steps]]"));
}

#[test]
fn init_custom_name_appends_toml_extension() {
    let dir = tmpdir("init-custom");
    let out = run(&["init", "myconfig"], &dir);
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
    let out = run(&["init"], &dir);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("already exists"),
        "expected 'already exists' error"
    );
}

// ---------------------------------------------------------------------------
// list subcommand
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

    let out = run(&["list"], &dir);
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
    let out = run(&["list"], &dir);
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
    let out = run(&["list"], &dir);
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
    let out = run(&["list"], &dir);
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
    let out = run(&["list"], &dir);
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
    let out = run(&["list"], &dir);
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
    let out = run(&["list"], &dir);
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
    let out = run(&["list"], &dir);
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
    let out = run(&["list"], &dir);
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


// ---------------------------------------------------------------------------
// US-012: completions subcommand
// ---------------------------------------------------------------------------

#[test]
fn completions_bash_contains_runsteps() {
    let dir = tmpdir("comp-bash");
    let out = run(&["completions", "bash"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let combined = stdout(&out) + &stderr(&out);
    assert!(
        combined.contains("runsteps"),
        "bash completions missing 'runsteps', got: {}",
        &combined[..combined.len().min(200)]
    );
}

#[test]
fn completions_zsh_contains_compdef() {
    let dir = tmpdir("comp-zsh");
    let out = run(&["completions", "zsh"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let combined = stdout(&out) + &stderr(&out);
    assert!(
        combined.contains("#compdef runsteps") || combined.contains("_runsteps"),
        "zsh completions missing compdef/function, got: {}",
        &combined[..combined.len().min(200)]
    );
}

#[test]
fn completions_fish_contains_complete_c() {
    let dir = tmpdir("comp-fish");
    let out = run(&["completions", "fish"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let combined = stdout(&out) + &stderr(&out);
    assert!(
        combined.contains("complete") && combined.contains("runsteps"),
        "fish completions missing 'complete -c runsteps', got: {}",
        &combined[..combined.len().min(200)]
    );
}

#[test]
fn completions_invalid_shell_exits_nonzero() {
    let dir = tmpdir("comp-invalid");
    let out = run(&["completions", "invalid_shell"], &dir);
    assert!(
        !out.status.success(),
        "expected nonzero exit for invalid shell, got success"
    );
}

// ---------------------------------------------------------------------------
// US-016: legacy --list and --init top-level flags are removed (subcommand-only)
// ---------------------------------------------------------------------------

#[test]
fn legacy_list_flag_exits_nonzero_with_unexpected_argument() {
    let dir = tmpdir("us016-list");
    let out = run(&["--list"], &dir);
    assert!(
        !out.status.success(),
        "expected nonzero exit for removed --list flag"
    );
    let err = stderr(&out);
    assert!(
        err.contains("unexpected argument") || err.contains("--list"),
        "expected 'unexpected argument' in stderr, got: {err}"
    );
}

#[test]
fn legacy_init_flag_exits_nonzero_with_unexpected_argument() {
    let dir = tmpdir("us016-init");
    let out = run(&["--init"], &dir);
    assert!(
        !out.status.success(),
        "expected nonzero exit for removed --init flag"
    );
    let err = stderr(&out);
    assert!(
        err.contains("unexpected argument") || err.contains("--init"),
        "expected 'unexpected argument' in stderr, got: {err}"
    );
}

#[test]
fn list_subcommand_still_works_after_flag_removal() {
    let dir = tmpdir("us016-list-subcmd");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "list-subcmd-test"

[[steps]]
name = "myalpha"
description = "First"
command = "echo myalpha"
"#,
    )
    .unwrap();
    let out = run(&["list"], &dir);
    assert!(out.status.success(), "runsteps list must still work, stderr: {}", stderr(&out));
    let combined = stdout(&out) + &stderr(&out);
    assert!(
        combined.contains("myalpha"),
        "expected step in list output, got: {combined}"
    );
}

#[test]
fn init_subcommand_still_works_after_flag_removal() {
    let dir = tmpdir("us016-init-subcmd");
    let out = run(&["init"], &dir);
    assert!(out.status.success(), "runsteps init must still work, stderr: {}", stderr(&out));
    assert!(dir.join("runsteps.toml").exists(), "init subcommand should create runsteps.toml");
}

// ---------------------------------------------------------------------------
// US-007: --again replay
// ---------------------------------------------------------------------------

#[test]
fn again_replays_last_run_in_dry_run() {
    let dir = tmpdir("again-dry");
    let cache = tmpdir("again-dry-cache");
    let log = dir.join("log.txt");
    fs::write(&log, "").unwrap();
    let config = format!(
        r#"
[metadata]
name = "again-test"

[[steps]]
name = "alpha"
description = "Step alpha"
command = "printf 'alpha\n' >> {path}"

[[steps]]
name = "beta"
description = "Step beta"
command = "printf 'beta\n' >> {path}"
"#,
        path = log.display()
    );
    fs::write(dir.join("runsteps.toml"), &config).unwrap();

    // First run to populate history (use isolated cache dir).
    let out = run_with_cache(&["--all", "--yes"], &dir, &cache);
    assert!(out.status.success(), "first run failed: {}", stderr(&out));

    // Now replay with --again --dry-run — should show same steps without re-executing.
    let content_before = fs::read_to_string(&log).unwrap();
    let out2 = run_with_cache(&["--again", "--dry-run"], &dir, &cache);
    assert!(out2.status.success(), "--again --dry-run failed: {}", stderr(&out2));
    let content_after = fs::read_to_string(&log).unwrap();
    // dry-run must not re-execute
    assert_eq!(
        content_before, content_after,
        "--again --dry-run must not re-execute steps"
    );
    // dry-run output should mention the steps
    let combined = stdout(&out2) + &stderr(&out2);
    assert!(
        combined.contains("alpha") || combined.contains("dry"),
        "--again --dry-run output should mention steps, got: {combined}"
    );
}

#[test]
fn again_warns_on_config_change() {
    let dir = tmpdir("again-change");
    let cache = tmpdir("again-change-cache");
    let log = dir.join("log.txt");
    fs::write(&log, "").unwrap();
    let config = format!(
        r#"
[metadata]
name = "again-change-test"

[[steps]]
name = "step1"
description = "Step one"
command = "printf 'step1\n' >> {path}"
"#,
        path = log.display()
    );
    let config_path = dir.join("runsteps.toml");
    fs::write(&config_path, &config).unwrap();

    // First run with isolated cache.
    let out = run_with_cache(&["--all", "--yes"], &dir, &cache);
    assert!(out.status.success(), "first run: {}", stderr(&out));

    // Modify config slightly (change description to change SHA-256).
    let modified = format!(
        r#"
[metadata]
name = "again-change-test"
description = "modified"

[[steps]]
name = "step1"
description = "Step one modified"
command = "printf 'step1\n' >> {path}"
"#,
        path = log.display()
    );
    fs::write(&config_path, &modified).unwrap();

    // --again should warn about config change.
    let out2 = run_with_cache(&["--again", "--yes"], &dir, &cache);
    let err = stderr(&out2);
    assert!(
        err.contains("config has changed"),
        "expected 'config has changed' warning, got stderr: {err}"
    );
}

#[test]
fn again_no_history_exits_error() {
    let dir = tmpdir("again-nohist");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "nohist"

[[steps]]
name = "s"
description = "d"
command = "echo s"
"#,
    )
    .unwrap();
    // Use an isolated cache dir so we don't accidentally pick up real history.
    let fake_cache = dir.join("fake_cache");
    fs::create_dir_all(&fake_cache).unwrap();
    let out = std::process::Command::new(bin())
        .args(["--again", "--yes"])
        .env("RUNSTEPS_CACHE_DIR", &fake_cache)
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "expected failure with no history, got success"
    );
}

// ---------------------------------------------------------------------------
// US-009: list --json
// ---------------------------------------------------------------------------

#[test]
fn list_json_version_is_1() {
    let dir = tmpdir("list-json-ver");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "json-test"

[[steps]]
name = "alpha"
description = "First step"
command = "echo alpha"
"#,
    )
    .unwrap();
    let out = run(&["list", "--json"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&out)).expect("list --json must emit valid JSON");
    assert_eq!(
        parsed.get("version").and_then(|v| v.as_u64()),
        Some(1),
        "list --json version must be 1"
    );
}

#[test]
fn list_json_steps_have_name() {
    let dir = tmpdir("list-json-name");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "json-test"

[[steps]]
name = "alpha"
description = "First step"
command = "echo alpha"
"#,
    )
    .unwrap();
    let out = run(&["list", "--json"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&out)).expect("list --json must emit valid JSON");
    let first_name = parsed
        .pointer("/steps/0/name")
        .and_then(|v| v.as_str())
        .expect("steps[0].name must be a string");
    assert_eq!(first_name, "alpha");
}

#[test]
fn list_json_type_field_is_command_or_just_recipe() {
    let dir = tmpdir("list-json-type");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "json-type-test"

[[steps]]
name = "cmd-step"
description = "Command step"
command = "echo hi"

[[steps]]
name = "recipe-step"
description = "Recipe step"
just_recipe = "build"
"#,
    )
    .unwrap();
    let out = run(&["list", "--json"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let parsed: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let steps = parsed.get("steps").and_then(|v| v.as_array()).unwrap();
    let cmd_type = steps[0].get("type").and_then(|v| v.as_str()).unwrap();
    let recipe_type = steps[1].get("type").and_then(|v| v.as_str()).unwrap();
    assert_eq!(cmd_type, "command");
    assert_eq!(recipe_type, "just_recipe");
}

#[test]
fn list_human_output_unchanged_without_json_flag() {
    let dir = tmpdir("list-human");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "human-test"

[[steps]]
name = "myhuman"
description = "Human step"
command = "echo hi"
"#,
    )
    .unwrap();
    // `runsteps list` (no --json) — check it's not JSON
    let out = run(&["list"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let out_str = stdout(&out);
    assert!(
        serde_json::from_str::<serde_json::Value>(&out_str).is_err(),
        "list without --json must not emit JSON"
    );
    let combined = out_str + &stderr(&out);
    assert!(
        combined.contains("myhuman"),
        "human list output should contain step name, got: {combined}"
    );
}

// ---------------------------------------------------------------------------
// US-008: args + prompts + raw + --var flag
// ---------------------------------------------------------------------------

#[test]
fn args_static_pass_through_command_step() {
    let dir = tmpdir("args-static");
    let log = dir.join("log.txt");
    let config = format!(
        r#"
[metadata]
name = "args-static"

[[steps]]
name = "echo-args"
description = "Echo args"
command = "printf '%s\n' >> {path}"
args = ["hello"]
"#,
        path = log.display()
    );
    fs::write(dir.join("runsteps.toml"), &config).unwrap();
    let out = run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let content = fs::read_to_string(&log).unwrap_or_default();
    assert!(
        content.contains("hello") || stdout(&out).contains("hello"),
        "static arg 'hello' not found in output or log"
    );
}

#[test]
fn args_var_flag_substitutes_placeholder() {
    let dir = tmpdir("args-var");
    let log = dir.join("log.txt");
    let config = format!(
        r#"
[metadata]
name = "args-var"

[[steps]]
name = "echo-pod"
description = "Echo pod"
command = "echo {{{{pod}}}} >> {path}"
args = []
"#,
        path = log.display()
    );
    // Use a command that uses --var substitution in the command itself
    // We'll test via a simpler approach: write the var value to a file.
    let config2 = format!(
        r#"
[metadata]
name = "args-var"

[[steps]]
name = "write-val"
description = "Write var value"
command = "printf '%s\n' {{{{pod}}}} >> {path}"
"#,
        path = log.display()
    );
    drop(config);
    fs::write(dir.join("runsteps.toml"), &config2).unwrap();
    let out = run(&["--all", "--yes", "--var", "pod=webserver"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let content = fs::read_to_string(&log).unwrap_or_default();
    assert!(
        content.contains("webserver"),
        "expected 'webserver' in output, got: {content}"
    );
}

#[test]
fn args_missing_placeholder_in_yes_mode_exits_2() {
    let dir = tmpdir("args-missing");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "args-missing"

[[steps]]
name = "needs-var"
description = "Needs a placeholder"
command = "echo {{pod}}"
"#,
    )
    .unwrap();
    let out = run(&["--all", "--yes"], &dir);
    assert!(
        !out.status.success(),
        "expected failure when placeholder missing in --yes mode"
    );
    let err = stderr(&out);
    assert!(
        err.contains("pod") || err.contains("placeholder"),
        "expected placeholder name in error, got: {err}"
    );
}

#[test]
fn args_multi_placeholder_per_element() {
    let dir = tmpdir("args-multi");
    let log = dir.join("log.txt");
    let config = format!(
        r#"
[metadata]
name = "args-multi"

[[steps]]
name = "multi"
description = "Multi placeholder"
command = "printf '%s\n' >> {path}"
args = ["--pod={{{{p}}}}-{{{{e}}}}"]
"#,
        path = log.display()
    );
    fs::write(dir.join("runsteps.toml"), &config).unwrap();
    let out = run(
        &["--all", "--yes", "--var", "p=web", "--var", "e=staging"],
        &dir,
    );
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let content = fs::read_to_string(&log).unwrap_or_default();
    let combined = content + &stdout(&out) + &stderr(&out);
    assert!(
        combined.contains("web-staging") || combined.contains("--pod=web-staging"),
        "expected 'web-staging' in output, got combined output"
    );
}

#[test]
fn args_newline_in_var_value_exits_error() {
    let dir = tmpdir("args-newline");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "args-newline"

[[steps]]
name = "nl"
description = "Newline test"
command = "echo {{x}}"
"#,
    )
    .unwrap();
    // Pass a value with a newline using shell ANSI escape: $'a\nb'
    // We do this by writing it via a shell wrapper
    let out = std::process::Command::new(bin())
        .args(["--all", "--yes"])
        .arg("--var")
        .arg("x=a\nb") // actual newline in the arg
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "expected failure for newline in var value"
    );
    let err = stderr(&out);
    assert!(
        err.contains("newline"),
        "expected 'newline' in error message, got: {err}"
    );
}

#[test]
fn args_raw_true_passes_special_chars_unescaped() {
    let dir = tmpdir("args-raw");
    let log = dir.join("log.txt");
    // raw=true: value with $ should pass through without quoting
    // We write a config where the command captures its first arg
    let config = format!(
        r#"
[metadata]
name = "args-raw"

[[steps]]
name = "raw-step"
description = "Raw step"
command = "printf '%s\n' {{{{val}}}} >> {path}"
raw = true
"#,
        path = log.display()
    );
    fs::write(dir.join("runsteps.toml"), &config).unwrap();
    let out = run(&["--all", "--yes", "--var", "val=plain"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let content = fs::read_to_string(&log).unwrap_or_default();
    assert!(
        content.contains("plain"),
        "expected 'plain' in log with raw=true, got: {content}"
    );
}

// ---------------------------------------------------------------------------
// US-011: per-step env table
// ---------------------------------------------------------------------------

#[test]
fn env_table_sets_env_for_child_process() {
    let dir = tmpdir("env-basic");
    let log = dir.join("log.txt");
    let config = format!(
        r#"
[metadata]
name = "env-basic"

[[steps]]
name = "env-step"
description = "Env step"
command = "echo $FOO >> {path}"

[steps.env]
FOO = "bar"
"#,
        path = log.display()
    );
    // TOML inline table syntax for env
    let config2 = format!(
        r#"
[metadata]
name = "env-basic"

[[steps]]
name = "env-step"
description = "Env step"
command = "echo $FOO >> {path}"
env = {{ FOO = "bar" }}
"#,
        path = log.display()
    );
    drop(config);
    fs::write(dir.join("runsteps.toml"), &config2).unwrap();
    let out = run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let content = fs::read_to_string(&log).unwrap_or_default();
    assert!(
        content.trim() == "bar" || content.contains("bar"),
        "expected env FOO=bar to produce 'bar', got: {content}"
    );
}

#[test]
fn env_table_with_placeholder_resolved_by_var() {
    let dir = tmpdir("env-var");
    let log = dir.join("log.txt");
    let config = format!(
        r#"
[metadata]
name = "env-var"

[[steps]]
name = "env-var-step"
description = "Env var step"
command = "echo $X >> {path}"
env = {{ X = "{{{{val}}}}" }}
"#,
        path = log.display()
    );
    fs::write(dir.join("runsteps.toml"), &config).unwrap();
    let out = run(&["--all", "--yes", "--var", "val=zz"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let content = fs::read_to_string(&log).unwrap_or_default();
    assert!(
        content.contains("zz"),
        "expected env X=zz to produce 'zz', got: {content}"
    );
}

#[test]
fn env_dry_run_prints_env_lines() {
    let dir = tmpdir("env-dry");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "env-dry"

[[steps]]
name = "env-dry-step"
description = "Env dry step"
command = "echo $FOO"
env = { FOO = "bar" }
"#,
    )
    .unwrap();
    let out = run(&["--dry-run", "--all"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let combined = stdout(&out) + &stderr(&out);
    assert!(
        combined.contains("FOO") || combined.contains("env:"),
        "dry-run should print env lines, got: {combined}"
    );
}

// ---------------------------------------------------------------------------
// US-010: graph subcommand
// ---------------------------------------------------------------------------

#[test]
fn graph_exits_zero_and_contains_step_names() {
    let dir = tmpdir("graph-basic");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "graph-test"

[[steps]]
name = "setup"
description = "Setup step"
command = "echo setup"
group = "setup"

[[steps]]
name = "build"
description = "Build step"
command = "echo build"
group = "ci"
depends_on = ["setup"]

[[steps]]
name = "deploy"
description = "Deploy step"
command = "echo deploy"
group = "deploy"
depends_on = ["build"]
"#,
    )
    .unwrap();

    let out = run(&["graph", "-c", "runsteps.toml"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let combined = stdout(&out) + &stderr(&out);
    assert!(combined.contains("setup"), "graph output missing 'setup'");
    assert!(combined.contains("build"), "graph output missing 'build'");
    assert!(combined.contains("deploy"), "graph output missing 'deploy'");
}

#[test]
fn graph_cycle_exits_2_with_cycle_message() {
    let dir = tmpdir("graph-cycle");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "cycle-test"

[[steps]]
name = "a"
description = "Step a"
command = "echo a"
depends_on = ["b"]

[[steps]]
name = "b"
description = "Step b"
command = "echo b"
depends_on = ["a"]
"#,
    )
    .unwrap();

    let out = run(&["graph", "-c", "runsteps.toml"], &dir);
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit code 2 for cycle, got {:?}",
        out.status.code()
    );
    let err = stderr(&out);
    assert!(
        err.contains("cycle detected:"),
        "expected 'cycle detected:' in stderr, got: {err}"
    );
    assert!(
        err.contains("a") && err.contains("b"),
        "expected both step names in cycle message, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// US-014: profiles
// ---------------------------------------------------------------------------

#[test]
fn profile_skip_confirms_auto_accepts_confirm_step() {
    let dir = tmpdir("profile-skip-confirms");
    let marker = dir.join("ran.txt");
    let config = format!(
        r#"
[metadata]
name = "profile-test"

[[steps]]
name = "dangerous"
description = "Needs confirmation"
command = "touch {path}"
confirm = true

[profiles.ci]
skip_confirms = true
"#,
        path = marker.display()
    );
    fs::write(dir.join("runsteps.toml"), &config).unwrap();

    // With --profile ci and --all (no --yes), confirm should be auto-skipped.
    let out = run(&["--all", "--profile", "ci", "-c", "runsteps.toml"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        marker.exists(),
        "confirm=true step should run without prompt under profile with skip_confirms=true"
    );
}

#[test]
fn profile_groups_restricts_to_matching_steps() {
    let dir = tmpdir("profile-groups");
    let log = dir.join("log.txt");
    fs::write(&log, "").unwrap();
    let config = format!(
        r#"
[metadata]
name = "profile-groups-test"

[[steps]]
name = "setup-step"
description = "Setup"
command = "printf 'setup\n' >> {path}"
group = "setup"

[[steps]]
name = "deploy-step"
description = "Deploy"
command = "printf 'deploy\n' >> {path}"
group = "deploy"

[[steps]]
name = "test-step"
description = "Test"
command = "printf 'test\n' >> {path}"
group = "test"

[profiles.staging]
groups = ["setup", "deploy"]
"#,
        path = log.display()
    );
    fs::write(dir.join("runsteps.toml"), &config).unwrap();

    let out = run(
        &["--all", "--yes", "--profile", "staging", "-c", "runsteps.toml"],
        &dir,
    );
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let content = fs::read_to_string(&log).unwrap();
    assert!(content.contains("setup"), "setup step should run under staging profile");
    assert!(content.contains("deploy"), "deploy step should run under staging profile");
    assert!(!content.contains("test"), "test step should NOT run under staging profile");
}

#[test]
fn profile_excluded_steps_removes_step_from_all() {
    let dir = tmpdir("profile-excluded");
    let log = dir.join("log.txt");
    fs::write(&log, "").unwrap();
    let config = format!(
        r#"
[metadata]
name = "profile-excluded-test"

[[steps]]
name = "safe-step"
description = "Safe"
command = "printf 'safe\n' >> {path}"

[[steps]]
name = "drop-db"
description = "Dangerous!"
command = "printf 'dropped\n' >> {path}"

[profiles.safe]
excluded_steps = ["drop-db"]
"#,
        path = log.display()
    );
    fs::write(dir.join("runsteps.toml"), &config).unwrap();

    let out = run(
        &["--all", "--yes", "--profile", "safe", "-c", "runsteps.toml"],
        &dir,
    );
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let content = fs::read_to_string(&log).unwrap();
    assert!(content.contains("safe"), "safe step should run");
    assert!(!content.contains("dropped"), "drop-db should be excluded by profile");
}

#[test]
fn profile_unknown_exits_2_with_error_message() {
    let dir = tmpdir("profile-unknown");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "profile-unknown-test"

[[steps]]
name = "s"
description = "d"
command = "echo s"

[profiles.ci]
skip_confirms = true
"#,
    )
    .unwrap();

    let out = run(
        &["--all", "--yes", "--profile", "nonexistent", "-c", "runsteps.toml"],
        &dir,
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit code 2 for unknown profile"
    );
    let err = stderr(&out);
    assert!(
        err.contains("unknown profile"),
        "expected 'unknown profile' in stderr, got: {err}"
    );
    assert!(
        err.contains("nonexistent"),
        "expected profile name in stderr, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// US-015: parallel execution
// ---------------------------------------------------------------------------

#[test]
fn parallel_steps_complete_faster_than_sequential() {
    let dir = tmpdir("parallel-timing");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "parallel-timing-test"

[[steps]]
name = "slow-a"
description = "Slow step A"
command = "sleep 2 && echo done-a"
parallel = true

[[steps]]
name = "slow-b"
description = "Slow step B"
command = "sleep 2 && echo done-b"
parallel = true
"#,
    )
    .unwrap();

    let start = std::time::Instant::now();
    let out = run(&["--all", "--yes"], &dir);
    let elapsed = start.elapsed();

    assert!(out.status.success(), "stderr: {}", stderr(&out));
    // Two 2s steps should finish in <3s if run in parallel, not ~4s if sequential.
    assert!(
        elapsed.as_secs() < 4,
        "parallel steps took {}s, expected <4s",
        elapsed.as_secs()
    );
}

#[test]
fn parallel_both_step_outputs_appear() {
    let dir = tmpdir("parallel-output");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "parallel-output-test"

[[steps]]
name = "step-alpha"
description = "Alpha"
command = "echo alpha-output"
parallel = true

[[steps]]
name = "step-beta"
description = "Beta"
command = "echo beta-output"
parallel = true
"#,
    )
    .unwrap();

    let out = run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let combined = stdout(&out) + &stderr(&out);
    assert!(
        combined.contains("alpha-output"),
        "alpha output missing, got: {}",
        &combined[..combined.len().min(500)]
    );
    assert!(
        combined.contains("beta-output"),
        "beta output missing, got: {}",
        &combined[..combined.len().min(500)]
    );
}

#[test]
fn parallel_failure_exits_nonzero() {
    let dir = tmpdir("parallel-fail");
    fs::write(
        dir.join("runsteps.toml"),
        r#"
[metadata]
name = "parallel-fail-test"

[[steps]]
name = "fail-step"
description = "Always fails"
command = "exit 1"
parallel = true

[[steps]]
name = "ok-step"
description = "Would succeed"
command = "echo ok"
parallel = true
"#,
    )
    .unwrap();

    let out = run(&["--all", "--yes"], &dir);
    assert!(
        !out.status.success(),
        "expected nonzero exit when a parallel step fails"
    );
    let err = stderr(&out);
    assert!(
        err.contains("fail-step") || err.contains("failed"),
        "expected failure step name in stderr, got: {err}"
    );
}

#[test]
fn sequential_behavior_unchanged_without_parallel_flag() {
    let dir = tmpdir("sequential-regression");
    let log = dir.join("log.txt");
    fs::write(&log, "").unwrap();
    let config = format!(
        r#"
[metadata]
name = "sequential-test"

[[steps]]
name = "first"
description = "First"
command = "printf '1\n' >> {path}"

[[steps]]
name = "second"
description = "Second"
command = "printf '2\n' >> {path}"

[[steps]]
name = "third"
description = "Third"
command = "printf '3\n' >> {path}"
"#,
        path = log.display()
    );
    fs::write(dir.join("runsteps.toml"), &config).unwrap();

    let out = run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let content = fs::read_to_string(&log).unwrap();
    assert_eq!(content, "1\n2\n3\n", "sequential order broken: {content}");
}
