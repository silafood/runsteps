mod common;

use std::fs;

#[test]
fn depends_on_auto_include_runs_dep_first() {
    let dir = common::tmpdir("deps-auto");
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

    let out = common::run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));

    let content = fs::read_to_string(&log).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines, vec!["build", "deploy"], "wrong execution order: {lines:?}");
}

#[test]
fn step_with_unknown_dep_reports_error() {
    let dir = common::tmpdir("bad-dep");
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
    let out = common::run(&["list"], &dir);
    assert!(!out.status.success());
    assert!(
        common::stderr(&out).contains("does-not-exist") || common::stderr(&out).contains("unknown"),
        "expected error about unknown dep"
    );
}

#[test]
fn just_recipe_runs_prereqs_by_default() {
    if std::process::Command::new("just")
        .arg("--version")
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(true)
    {
        eprintln!("skipping test: just not installed");
        return;
    }

    let dir = common::tmpdir("just-prereqs");
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

    let out = common::run(&["--all", "--yes"], &dir);
    let combined = common::stdout(&out) + &common::stderr(&out);
    assert!(
        out.status.success(),
        "expected success, stderr: {}",
        common::stderr(&out)
    );
    assert!(
        combined.contains("inner-ran"),
        "expected prereqs to run (inner-ran), got: {combined}"
    );
}

#[test]
fn just_no_deps_true_skips_prereqs() {
    if std::process::Command::new("just")
        .arg("--version")
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(true)
    {
        eprintln!("skipping test: just not installed");
        return;
    }

    let dir = common::tmpdir("just-no-deps");
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

    let out = common::run(&["--all", "--yes"], &dir);
    let combined = common::stdout(&out) + &common::stderr(&out);
    assert!(
        out.status.success(),
        "expected success, stderr: {}",
        common::stderr(&out)
    );
    assert!(
        !combined.contains("inner-ran"),
        "expected prereqs to be skipped (inner-ran should NOT appear), got: {combined}"
    );
}

#[test]
fn dry_run_just_no_deps_true_shows_no_deps_flag() {
    let dir = common::tmpdir("dry-no-deps");
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

    let out = common::run(&["--dry-run", "--all"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let combined = common::stdout(&out) + &common::stderr(&out);
    assert!(
        combined.contains("--no-deps"),
        "expected '--no-deps' in dry-run output when just_no_deps=true, got: {combined}"
    );
    assert!(
        combined.contains("deploy-all"),
        "expected recipe name in dry-run output, got: {combined}"
    );
}

#[test]
fn dry_run_without_just_no_deps_omits_no_deps_flag() {
    let dir = common::tmpdir("dry-with-deps");
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

    let out = common::run(&["--dry-run", "--all"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let combined = common::stdout(&out) + &common::stderr(&out);
    assert!(
        combined.contains("deploy-all"),
        "expected recipe name in dry-run output, got: {combined}"
    );
    assert!(
        !combined.contains("--no-deps"),
        "expected NO '--no-deps' in dry-run output when just_no_deps not set, got: {combined}"
    );
}
