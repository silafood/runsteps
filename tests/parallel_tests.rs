mod common;

use std::fs;

#[test]
fn parallel_steps_complete_faster_than_sequential() {
    let dir = common::tmpdir("parallel-timing");
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
    let out = common::run(&["--all", "--yes"], &dir);
    let elapsed = start.elapsed();

    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    assert!(
        elapsed.as_secs() < 4,
        "parallel steps took {}s, expected <4s",
        elapsed.as_secs()
    );
}

#[test]
fn parallel_both_step_outputs_appear() {
    let dir = common::tmpdir("parallel-output");
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

    let out = common::run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let combined = common::stdout(&out) + &common::stderr(&out);
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
    let dir = common::tmpdir("parallel-fail");
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

    let out = common::run(&["--all", "--yes"], &dir);
    assert!(
        !out.status.success(),
        "expected nonzero exit when a parallel step fails"
    );
    let err = common::stderr(&out);
    assert!(
        err.contains("fail-step") || err.contains("failed"),
        "expected failure step name in stderr, got: {err}"
    );
}

#[test]
fn sequential_behavior_unchanged_without_parallel_flag() {
    let dir = common::tmpdir("sequential-regression");
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

    let out = common::run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let content = fs::read_to_string(&log).unwrap();
    assert_eq!(content, "1\n2\n3\n", "sequential order broken: {content}");
}
