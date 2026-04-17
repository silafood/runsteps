mod common;

use std::fs;

#[test]
fn again_replays_last_run_in_dry_run() {
    let dir = common::tmpdir("again-dry");
    let cache = common::tmpdir("again-dry-cache");
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

    let out = common::run_with_cache(&["--all", "--yes"], &dir, &cache);
    assert!(out.status.success(), "first run failed: {}", common::stderr(&out));

    let content_before = fs::read_to_string(&log).unwrap();
    let out2 = common::run_with_cache(&["--again", "--dry-run"], &dir, &cache);
    assert!(out2.status.success(), "--again --dry-run failed: {}", common::stderr(&out2));
    let content_after = fs::read_to_string(&log).unwrap();
    assert_eq!(
        content_before, content_after,
        "--again --dry-run must not re-execute steps"
    );
    let combined = common::stdout(&out2) + &common::stderr(&out2);
    assert!(
        combined.contains("alpha") || combined.contains("dry"),
        "--again --dry-run output should mention steps, got: {combined}"
    );
}

#[test]
fn again_warns_on_config_change() {
    let dir = common::tmpdir("again-change");
    let cache = common::tmpdir("again-change-cache");
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

    let out = common::run_with_cache(&["--all", "--yes"], &dir, &cache);
    assert!(out.status.success(), "first run: {}", common::stderr(&out));

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

    let out2 = common::run_with_cache(&["--again", "--yes"], &dir, &cache);
    let err = common::stderr(&out2);
    assert!(
        err.contains("config has changed"),
        "expected 'config has changed' warning, got stderr: {err}"
    );
}

#[test]
fn again_no_history_exits_error() {
    let dir = common::tmpdir("again-nohist");
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
    let fake_cache = dir.join("fake_cache");
    fs::create_dir_all(&fake_cache).unwrap();
    let out = std::process::Command::new(common::bin())
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
