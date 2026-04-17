mod common;

use std::fs;

#[test]
fn init_creates_default_config() {
    let dir = common::tmpdir("init-default");
    let out = common::run(&["init"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    assert!(dir.join("runsteps.toml").exists(), "config file not created");
    let content = fs::read_to_string(dir.join("runsteps.toml")).unwrap();
    assert!(content.contains("[metadata]"));
    assert!(content.contains("[[steps]]"));
}

#[test]
fn init_custom_name_appends_toml_extension() {
    let dir = common::tmpdir("init-custom");
    let out = common::run(&["init", "myconfig"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    assert!(
        dir.join("myconfig.toml").exists(),
        "myconfig.toml not created"
    );
}

#[test]
fn init_refuses_to_overwrite_existing_file() {
    let dir = common::tmpdir("init-overwrite");
    fs::write(dir.join("runsteps.toml"), "[metadata]\nname=\"x\"\n").unwrap();
    let out = common::run(&["init"], &dir);
    assert!(!out.status.success());
    assert!(
        common::stderr(&out).contains("already exists"),
        "expected 'already exists' error"
    );
}

#[test]
fn init_subcommand_still_works_after_flag_removal() {
    let dir = common::tmpdir("us016-init-subcmd");
    let out = common::run(&["init"], &dir);
    assert!(out.status.success(), "runsteps init must still work, stderr: {}", common::stderr(&out));
    assert!(dir.join("runsteps.toml").exists(), "init subcommand should create runsteps.toml");
}
