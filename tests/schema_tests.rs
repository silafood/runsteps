mod common;

#[test]
fn schema_json_outputs_draft07_schema() {
    let dir = common::tmpdir("schema-json");
    let out = common::run(&["schema", "--json"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let json_str = common::stdout(&out);
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("schema --json must emit valid JSON");
    assert_eq!(
        parsed.get("$schema").and_then(|v| v.as_str()),
        Some("http://json-schema.org/draft-07/schema#"),
        "schema --json must contain draft-07 $schema key"
    );
}

#[test]
fn schema_human_contains_key_fields() {
    let dir = common::tmpdir("schema-human");
    let out = common::run(&["schema"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let combined = common::stdout(&out) + &common::stderr(&out);
    assert!(
        combined.contains("name"),
        "human schema output missing 'name' field"
    );
    assert!(
        combined.contains("just_recipe"),
        "human schema output missing 'just_recipe' field"
    );
    assert!(
        combined.contains("just_no_deps"),
        "human schema output missing 'just_no_deps' field"
    );
}

#[test]
fn schema_json_exit_zero() {
    let dir = common::tmpdir("schema-json-exit");
    let out = common::run(&["schema", "--json"], &dir);
    assert!(out.status.success(), "schema --json must exit 0");
}

#[test]
fn schema_human_exit_zero() {
    let dir = common::tmpdir("schema-human-exit");
    let out = common::run(&["schema"], &dir);
    assert!(out.status.success(), "schema (human) must exit 0");
}
