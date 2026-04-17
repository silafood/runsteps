mod common;

use std::fs;

#[test]
fn missing_config_file_reports_error() {
    let dir = common::tmpdir("missing-config");
    let out = common::run(&["-c", "nonexistent.toml"], &dir);
    assert!(!out.status.success());
    assert!(
        common::stderr(&out).contains("nonexistent.toml"),
        "error message should mention the missing file"
    );
}

#[test]
fn invalid_toml_reports_error() {
    let dir = common::tmpdir("bad-toml");
    fs::write(dir.join("runsteps.toml"), "this is not valid toml [[[").unwrap();
    let out = common::run(&["list"], &dir);
    assert!(!out.status.success());
    let combined = common::stdout(&out) + &common::stderr(&out);
    assert!(
        combined.to_lowercase().contains("toml")
            || combined.to_lowercase().contains("invalid")
            || combined.to_lowercase().contains("parse"),
        "expected parse error, got: {combined}"
    );
}

#[test]
fn step_with_both_command_and_just_recipe_reports_error() {
    let dir = common::tmpdir("both-cmd");
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
    let out = common::run(&["list"], &dir);
    assert!(!out.status.success());
    assert!(
        common::stderr(&out).contains("both"),
        "expected 'both' error message"
    );
}

#[test]
fn step_with_neither_command_nor_recipe_reports_error() {
    let dir = common::tmpdir("neither-cmd");
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
    let out = common::run(&["list"], &dir);
    assert!(!out.status.success());
    assert!(
        common::stderr(&out).contains("neither"),
        "expected 'neither' error message"
    );
}

#[test]
fn toml_error_meta_typo_suggests_metadata() {
    let dir = common::tmpdir("err-meta");
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
    let out = common::run(&["list"], &dir);
    assert!(!out.status.success(), "expected failure for unknown field");
    let err = common::stderr(&out);
    assert!(
        err.contains("did you mean"),
        "expected 'did you mean' in stderr, got: {err}"
    );
    assert!(
        err.contains("metadata"),
        "expected 'metadata' suggestion in stderr, got: {err}"
    );
}

#[test]
fn toml_error_step_typo_suggests_steps() {
    let dir = common::tmpdir("err-step");
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
    let out = common::run(&["list"], &dir);
    assert!(!out.status.success(), "expected failure for unknown field");
    let err = common::stderr(&out);
    assert!(
        err.contains("did you mean") || err.contains("steps") || err.contains("step"),
        "expected step-related suggestion in stderr, got: {err}"
    );
}

#[test]
fn toml_error_just_recipee_suggests_just_recipe() {
    let dir = common::tmpdir("err-recipe");
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
    let out = common::run(&["list"], &dir);
    assert!(!out.status.success(), "expected failure for unknown field");
    let err = common::stderr(&out);
    assert!(
        err.contains("did you mean"),
        "expected 'did you mean' in stderr, got: {err}"
    );
    assert!(
        err.contains("just_recipe"),
        "expected 'just_recipe' suggestion in stderr, got: {err}"
    );
}

#[test]
fn toml_error_bogus_field_lists_known_keys() {
    let dir = common::tmpdir("err-bogus");
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
    let out = common::run(&["list"], &dir);
    assert!(!out.status.success(), "expected failure for unknown field");
    let err = common::stderr(&out);
    assert!(
        err.contains("known keys:"),
        "expected 'known keys:' in stderr, got: {err}"
    );
}
