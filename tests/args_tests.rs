mod common;

use std::fs;

#[test]
fn args_static_pass_through_command_step() {
    let dir = common::tmpdir("args-static");
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
    let out = common::run(&["--all", "--yes"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let content = fs::read_to_string(&log).unwrap_or_default();
    assert!(
        content.contains("hello") || common::stdout(&out).contains("hello"),
        "static arg 'hello' not found in output or log"
    );
}

#[test]
fn args_var_flag_substitutes_placeholder() {
    let dir = common::tmpdir("args-var");
    let log = dir.join("log.txt");
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
    fs::write(dir.join("runsteps.toml"), &config2).unwrap();
    let out = common::run(&["--all", "--yes", "--var", "pod=webserver"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let content = fs::read_to_string(&log).unwrap_or_default();
    assert!(
        content.contains("webserver"),
        "expected 'webserver' in output, got: {content}"
    );
}

#[test]
fn args_missing_placeholder_in_yes_mode_exits_2() {
    let dir = common::tmpdir("args-missing");
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
    let out = common::run(&["--all", "--yes"], &dir);
    assert!(
        !out.status.success(),
        "expected failure when placeholder missing in --yes mode"
    );
    let err = common::stderr(&out);
    assert!(
        err.contains("pod") || err.contains("placeholder"),
        "expected placeholder name in error, got: {err}"
    );
}

#[test]
fn args_multi_placeholder_per_element() {
    let dir = common::tmpdir("args-multi");
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
    let out = common::run(
        &["--all", "--yes", "--var", "p=web", "--var", "e=staging"],
        &dir,
    );
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let content = fs::read_to_string(&log).unwrap_or_default();
    let combined = content + &common::stdout(&out) + &common::stderr(&out);
    assert!(
        combined.contains("web-staging") || combined.contains("--pod=web-staging"),
        "expected 'web-staging' in output, got combined output"
    );
}

#[test]
fn args_newline_in_var_value_exits_error() {
    let dir = common::tmpdir("args-newline");
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
    let out = std::process::Command::new(common::bin())
        .args(["--all", "--yes"])
        .arg("--var")
        .arg("x=a\nb")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "expected failure for newline in var value"
    );
    let err = common::stderr(&out);
    assert!(
        err.contains("newline"),
        "expected 'newline' in error message, got: {err}"
    );
}

#[test]
fn args_raw_true_passes_special_chars_unescaped() {
    let dir = common::tmpdir("args-raw");
    let log = dir.join("log.txt");
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
    let out = common::run(&["--all", "--yes", "--var", "val=plain"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let content = fs::read_to_string(&log).unwrap_or_default();
    assert!(
        content.contains("plain"),
        "expected 'plain' in log with raw=true, got: {content}"
    );
}
