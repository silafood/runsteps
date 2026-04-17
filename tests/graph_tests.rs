mod common;

use std::fs;

#[test]
fn graph_exits_zero_and_contains_step_names() {
    let dir = common::tmpdir("graph-basic");
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

    let out = common::run(&["graph", "-c", "runsteps.toml"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let combined = common::stdout(&out) + &common::stderr(&out);
    assert!(combined.contains("setup"), "graph output missing 'setup'");
    assert!(combined.contains("build"), "graph output missing 'build'");
    assert!(combined.contains("deploy"), "graph output missing 'deploy'");
}

#[test]
fn graph_cycle_exits_2_with_cycle_message() {
    let dir = common::tmpdir("graph-cycle");
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

    let out = common::run(&["graph", "-c", "runsteps.toml"], &dir);
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit code 2 for cycle, got {:?}",
        out.status.code()
    );
    let err = common::stderr(&out);
    assert!(
        err.contains("cycle detected:"),
        "expected 'cycle detected:' in stderr, got: {err}"
    );
    assert!(
        err.contains("a") && err.contains("b"),
        "expected both step names in cycle message, got: {err}"
    );
}
