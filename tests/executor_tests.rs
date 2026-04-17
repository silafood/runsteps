mod common;

use std::fs;

#[test]
fn dry_run_prints_commands_without_executing() {
    let dir = common::tmpdir("dry-run");
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

    let out = common::run(&["--dry-run", "--all"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let combined = common::stdout(&out) + &common::stderr(&out);
    assert!(
        combined.contains("marker") || combined.contains("touch"),
        "dry-run output missing command info"
    );
    assert!(
        !std::path::Path::new("/tmp/runsteps-dry-run-marker-should-not-exist").exists(),
        "dry-run executed the command!"
    );
}

#[test]
fn all_yes_runs_every_step_in_order() {
    let dir = common::tmpdir("all-yes");
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

    let out = common::run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));

    let written = fs::read_to_string(&marker).unwrap();
    assert_eq!(written, "1\n2\n3\n", "steps ran out of order or duplicated");
}

#[test]
fn confirm_step_skipped_prompt_with_yes_flag() {
    let dir = common::tmpdir("confirm-yes");
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

    let out = common::run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    assert!(marker.exists(), "step with confirm:true did not run with --yes");
}

#[test]
fn group_filter_runs_only_matching_steps() {
    let dir = common::tmpdir("group");
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

    let out = common::run(&["--all", "--yes", "--group", "setup"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));

    let content = fs::read_to_string(&log).unwrap();
    assert!(content.contains("setup"), "setup step did not run");
    assert!(!content.contains("deploy"), "deploy step ran but should not have");
}

#[test]
fn failing_step_exits_nonzero() {
    let dir = common::tmpdir("fail");
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

    let out = common::run(&["--all", "--yes"], &dir);
    assert!(
        !out.status.success(),
        "expected non-zero exit for failing step"
    );
}

#[test]
fn failing_step_stops_subsequent_steps() {
    let dir = common::tmpdir("fail-stop");
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

    let out = common::run(&["--all", "--yes"], &dir);
    assert!(!out.status.success());
    assert!(
        !marker.exists(),
        "subsequent step ran after a failing step"
    );
}

#[test]
fn state_machine_dedup_shared_dependency() {
    let dir = common::tmpdir("dedup");
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

    let out = common::run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));

    let content = fs::read_to_string(&counter).unwrap();
    let run_count = content.lines().count();
    assert_eq!(
        run_count, 1,
        "shared-dep ran {run_count} times, expected exactly 1"
    );
}
