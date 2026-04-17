mod common;

use std::fs;

#[test]
fn profile_skip_confirms_auto_accepts_confirm_step() {
    let dir = common::tmpdir("profile-skip-confirms");
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

    let out = common::run(&["--all", "--profile", "ci", "-c", "runsteps.toml"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    assert!(
        marker.exists(),
        "confirm=true step should run without prompt under profile with skip_confirms=true"
    );
}

#[test]
fn profile_groups_restricts_to_matching_steps() {
    let dir = common::tmpdir("profile-groups");
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

    let out = common::run(
        &["--all", "--yes", "--profile", "staging", "-c", "runsteps.toml"],
        &dir,
    );
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let content = fs::read_to_string(&log).unwrap();
    assert!(content.contains("setup"), "setup step should run under staging profile");
    assert!(content.contains("deploy"), "deploy step should run under staging profile");
    assert!(!content.contains("test"), "test step should NOT run under staging profile");
}

#[test]
fn profile_excluded_steps_removes_step_from_all() {
    let dir = common::tmpdir("profile-excluded");
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

    let out = common::run(
        &["--all", "--yes", "--profile", "safe", "-c", "runsteps.toml"],
        &dir,
    );
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let content = fs::read_to_string(&log).unwrap();
    assert!(content.contains("safe"), "safe step should run");
    assert!(!content.contains("dropped"), "drop-db should be excluded by profile");
}

#[test]
fn profile_unknown_exits_2_with_error_message() {
    let dir = common::tmpdir("profile-unknown");
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

    let out = common::run(
        &["--all", "--yes", "--profile", "nonexistent", "-c", "runsteps.toml"],
        &dir,
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit code 2 for unknown profile"
    );
    let err = common::stderr(&out);
    assert!(
        err.contains("unknown profile"),
        "expected 'unknown profile' in stderr, got: {err}"
    );
    assert!(
        err.contains("nonexistent"),
        "expected profile name in stderr, got: {err}"
    );
}
