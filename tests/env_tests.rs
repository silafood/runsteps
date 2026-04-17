mod common;

use std::fs;

#[test]
fn env_table_sets_env_for_child_process() {
    let dir = common::tmpdir("env-basic");
    let log = dir.join("log.txt");
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
    fs::write(dir.join("runsteps.toml"), &config2).unwrap();
    let out = common::run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let content = fs::read_to_string(&log).unwrap_or_default();
    assert!(
        content.trim() == "bar" || content.contains("bar"),
        "expected env FOO=bar to produce 'bar', got: {content}"
    );
}

#[test]
fn env_table_with_placeholder_resolved_by_var() {
    let dir = common::tmpdir("env-var");
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
    let out = common::run(&["--all", "--yes", "--var", "val=zz"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let content = fs::read_to_string(&log).unwrap_or_default();
    assert!(
        content.contains("zz"),
        "expected env X=zz to produce 'zz', got: {content}"
    );
}

#[test]
fn env_dry_run_prints_env_lines() {
    let dir = common::tmpdir("env-dry");
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
    let out = common::run(&["--dry-run", "--all"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let combined = common::stdout(&out) + &common::stderr(&out);
    assert!(
        combined.contains("FOO") || combined.contains("env:"),
        "dry-run should print env lines, got: {combined}"
    );
}
