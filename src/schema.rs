// Public items here are not yet consumed by main.rs (the `schema` subcommand
// is wired in Phase B / US-006). The allow attribute suppresses premature
// dead_code warnings until that phase ships.
#![allow(dead_code)]

//! Source-of-truth schema definitions for runsteps config fields.
//!
//! This module is the single authoritative record of every field that exists in
//! runsteps.toml. When a new field is added to `Metadata` or `Step` in config.rs:
//!   1. Add a `FieldSpec` entry to the appropriate `*_FIELDS` constant here.
//!   2. Add the field name to `KNOWN_KEYS` in config.rs.
//!   3. Update the hand-maintained lists in config.rs drift tests.
//!   4. Update SCHEMA.md.

use serde_json::{json, Value};

/// Describes a single field in a TOML table.
#[derive(Debug, PartialEq)]
pub struct FieldSpec {
    /// TOML field name as it appears in runsteps.toml.
    pub name: &'static str,
    /// Type description string (e.g. "string", "bool", "array<string>").
    pub ty: &'static str,
    /// Whether the field must be present.
    pub required: bool,
    /// Human-readable description of the field's purpose.
    pub description: &'static str,
    /// Version in which this field was introduced (e.g. "v0.1.0").
    pub added_in: &'static str,
}

/// Describes a TOML table (either `[metadata]` or `[[steps]]`).
pub struct TableSpec {
    /// Table name as it appears in runsteps.toml.
    pub name: &'static str,
    /// All fields belonging to this table.
    pub fields: &'static [FieldSpec],
}

pub const METADATA_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "name",
        ty: "string",
        required: true,
        description: "Human-readable project name displayed in the picker header.",
        added_in: "v0.1.0",
    },
    FieldSpec {
        name: "description",
        ty: "string",
        required: false,
        description: "Optional one-line description shown below the project name.",
        added_in: "v0.1.0",
    },
    FieldSpec {
        name: "justfile",
        ty: "string",
        required: false,
        description: "Path to the justfile used for just_recipe steps. Relative paths are resolved from working_directory.",
        added_in: "v0.1.0",
    },
    FieldSpec {
        name: "working_directory",
        ty: "string",
        required: false,
        description: "Working directory for all step execution. Relative to the location of runsteps.toml.",
        added_in: "v0.1.0",
    },
];

pub const STEP_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "name",
        ty: "string",
        required: true,
        description: "Unique identifier for the step. Used in depends_on references and history replay.",
        added_in: "v0.1.0",
    },
    FieldSpec {
        name: "description",
        ty: "string",
        required: true,
        description: "One-line description shown in the interactive picker.",
        added_in: "v0.1.0",
    },
    FieldSpec {
        name: "command",
        ty: "string",
        required: false,
        description: "Shell command executed via sh -c. Mutually exclusive with just_recipe.",
        added_in: "v0.1.0",
    },
    FieldSpec {
        name: "just_recipe",
        ty: "string",
        required: false,
        description: "Name of a just recipe to invoke. Mutually exclusive with command.",
        added_in: "v0.1.0",
    },
    FieldSpec {
        name: "just_no_deps",
        ty: "bool",
        required: false,
        description: "When true, passes --no-deps to just, skipping the recipe's prerequisites. Has no effect on command steps.",
        added_in: "v0.2.0",
    },
    FieldSpec {
        name: "group",
        ty: "string",
        required: false,
        description: "Logical grouping label. Used with --group filtering.",
        added_in: "v0.1.0",
    },
    FieldSpec {
        name: "confirm",
        ty: "bool",
        required: false,
        description: "When true, prompts for confirmation before executing the step.",
        added_in: "v0.1.0",
    },
    FieldSpec {
        name: "depends_on",
        ty: "array<string>",
        required: false,
        description: "Names of steps that must also be selected.",
        added_in: "v0.1.0",
    },
];

/// The complete schema: all tables and their fields.
pub const SCHEMA: &[TableSpec] = &[
    TableSpec {
        name: "metadata",
        fields: METADATA_FIELDS,
    },
    TableSpec {
        name: "steps",
        fields: STEP_FIELDS,
    },
];

/// Map a FieldSpec type string to a JSON Schema type value.
fn field_to_json_schema_type(field: &FieldSpec) -> Value {
    match field.ty {
        "bool" => json!({ "type": "boolean", "description": field.description }),
        "array<string>" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": field.description
        }),
        _ => json!({ "type": "string", "description": field.description }),
    }
}

/// Generate a JSON Schema draft-07 document from the SCHEMA constant.
pub fn to_json_schema() -> Value {
    let mut metadata_props = serde_json::Map::new();
    let mut metadata_required: Vec<Value> = Vec::new();

    for field in METADATA_FIELDS {
        metadata_props.insert(
            field.name.to_string(),
            field_to_json_schema_type(field),
        );
        if field.required {
            metadata_required.push(json!(field.name));
        }
    }

    let mut step_props = serde_json::Map::new();
    let mut step_required: Vec<Value> = Vec::new();

    for field in STEP_FIELDS {
        step_props.insert(
            field.name.to_string(),
            field_to_json_schema_type(field),
        );
        if field.required {
            step_required.push(json!(field.name));
        }
    }

    json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "runsteps config",
        "type": "object",
        "required": ["metadata", "steps"],
        "properties": {
            "metadata": {
                "type": "object",
                "required": metadata_required,
                "properties": metadata_props
            },
            "steps": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": step_required,
                    "properties": step_props
                }
            }
        }
    })
}

/// Return the JSON Schema as a pretty-printed JSON string.
pub fn schema_json() -> String {
    serde_json::to_string_pretty(&to_json_schema()).expect("schema serialization is infallible")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-maintained list of expected step field names after Phase A.
    /// MAINTENANCE: Update this list when adding new Step fields.
    fn expected_step_fields() -> Vec<&'static str> {
        vec![
            "name",
            "description",
            "command",
            "just_recipe",
            "just_no_deps",
            "group",
            "confirm",
            "depends_on",
        ]
    }

    /// Hand-maintained list of expected metadata field names after Phase A.
    /// MAINTENANCE: Update this list when adding new Metadata fields.
    fn expected_metadata_fields() -> Vec<&'static str> {
        vec!["name", "description", "justfile", "working_directory"]
    }

    #[test]
    fn schema_json_is_valid_draft_07() {
        let schema = to_json_schema();
        let schema_url = schema
            .get("$schema")
            .and_then(|v| v.as_str())
            .expect("$schema key must exist");
        assert_eq!(
            schema_url,
            "http://json-schema.org/draft-07/schema#",
            "schema URL must be draft-07"
        );
        // Verify it round-trips through serde_json
        let json_str = schema_json();
        let parsed: Value = serde_json::from_str(&json_str).expect("schema JSON must parse");
        assert_eq!(
            parsed.get("$schema").and_then(|v| v.as_str()),
            Some("http://json-schema.org/draft-07/schema#")
        );
    }

    #[test]
    fn schema_includes_all_step_fields() {
        let steps_table = SCHEMA
            .iter()
            .find(|t| t.name == "steps")
            .expect("SCHEMA must contain 'steps' table");

        let schema_field_names: Vec<&str> = steps_table.fields.iter().map(|f| f.name).collect();
        let expected = expected_step_fields();

        for expected_field in &expected {
            assert!(
                schema_field_names.contains(expected_field),
                "SCHEMA steps table is missing field: '{}'",
                expected_field
            );
        }
    }

    #[test]
    fn schema_includes_all_metadata_fields() {
        let metadata_table = SCHEMA
            .iter()
            .find(|t| t.name == "metadata")
            .expect("SCHEMA must contain 'metadata' table");

        let schema_field_names: Vec<&str> =
            metadata_table.fields.iter().map(|f| f.name).collect();
        let expected = expected_metadata_fields();

        for expected_field in &expected {
            assert!(
                schema_field_names.contains(expected_field),
                "SCHEMA metadata table is missing field: '{}'",
                expected_field
            );
        }
    }

    #[test]
    fn schema_no_phantom_fields() {
        let all_real: std::collections::HashSet<&str> = expected_step_fields()
            .into_iter()
            .chain(expected_metadata_fields())
            .collect();

        for table in SCHEMA {
            for field in table.fields {
                assert!(
                    all_real.contains(field.name),
                    "SCHEMA table '{}' references non-existent field: '{}'",
                    table.name,
                    field.name
                );
            }
        }
    }

    #[test]
    fn schema_json_contains_steps_properties() {
        let schema = to_json_schema();
        let steps_props = schema
            .pointer("/properties/steps/items/properties")
            .expect("steps items properties must exist");

        for field_name in expected_step_fields() {
            assert!(
                steps_props.get(field_name).is_some(),
                "JSON schema missing step field: '{}'",
                field_name
            );
        }
    }

    #[test]
    fn schema_json_contains_metadata_properties() {
        let schema = to_json_schema();
        let meta_props = schema
            .pointer("/properties/metadata/properties")
            .expect("metadata properties must exist");

        for field_name in expected_metadata_fields() {
            assert!(
                meta_props.get(field_name).is_some(),
                "JSON schema missing metadata field: '{}'",
                field_name
            );
        }
    }
}
