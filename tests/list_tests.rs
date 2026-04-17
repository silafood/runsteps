mod common;

use std::fs;

#[test]
fn list_prints_steps_and_exits() {
    let dir = common::tmpdir("list");
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

    let out = common::run(&["list"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let combined = common::stdout(&out) + &common::stderr(&out);
    assert!(combined.contains("alpha"), "alpha missing from list output");
    assert!(combined.contains("beta"), "beta missing from list output");
}

#[test]
fn list_subcommand_still_works_after_flag_removal() {
    let dir = common::tmpdir("us016-list-subcmd");
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
    let out = common::run(&["list"], &dir);
    assert!(out.status.success(), "runsteps list must still work, stderr: {}", common::stderr(&out));
    let combined = common::stdout(&out) + &common::stderr(&out);
    assert!(
        combined.contains("myalpha"),
        "expected step in list output, got: {combined}"
    );
}

#[test]
fn list_json_version_is_1() {
    let dir = common::tmpdir("list-json-ver");
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
    let out = common::run(&["list", "--json"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let parsed: serde_json::Value =
        serde_json::from_str(&common::stdout(&out)).expect("list --json must emit valid JSON");
    assert_eq!(
        parsed.get("version").and_then(|v| v.as_u64()),
        Some(1),
        "list --json version must be 1"
    );
}

#[test]
fn list_json_steps_have_name() {
    let dir = common::tmpdir("list-json-name");
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
    let out = common::run(&["list", "--json"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let parsed: serde_json::Value =
        serde_json::from_str(&common::stdout(&out)).expect("list --json must emit valid JSON");
    let first_name = parsed
        .pointer("/steps/0/name")
        .and_then(|v| v.as_str())
        .expect("steps[0].name must be a string");
    assert_eq!(first_name, "alpha");
}

#[test]
fn list_json_type_field_is_command_or_just_recipe() {
    let dir = common::tmpdir("list-json-type");
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
    let out = common::run(&["list", "--json"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let parsed: serde_json::Value = serde_json::from_str(&common::stdout(&out)).unwrap();
    let steps = parsed.get("steps").and_then(|v| v.as_array()).unwrap();
    let cmd_type = steps[0].get("type").and_then(|v| v.as_str()).unwrap();
    let recipe_type = steps[1].get("type").and_then(|v| v.as_str()).unwrap();
    assert_eq!(cmd_type, "command");
    assert_eq!(recipe_type, "just_recipe");
}

#[test]
fn list_human_output_unchanged_without_json_flag() {
    let dir = common::tmpdir("list-human");
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
    let out = common::run(&["list"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let out_str = common::stdout(&out);
    assert!(
        serde_json::from_str::<serde_json::Value>(&out_str).is_err(),
        "list without --json must not emit JSON"
    );
    let combined = out_str + &common::stderr(&out);
    assert!(
        combined.contains("myhuman"),
        "human list output should contain step name, got: {combined}"
    );
}
