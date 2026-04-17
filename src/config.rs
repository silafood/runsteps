use anyhow::{bail, Result};
use serde::Deserialize;
use std::collections::HashSet;

/// All field names across Metadata and Step structs that exist after Phase A (US-002, US-003).
///
/// MAINTENANCE: If you add a field to Metadata or Step, you MUST add it here too.
/// The drift tests `known_keys_covers_struct_fields` and `known_keys_only_contains_real_fields`
/// enforce this in both directions.
pub const KNOWN_KEYS: &[&str] = &[
    // Metadata fields
    "name",
    "description",
    "justfile",
    "working_directory",
    // Step fields
    "command",
    "confirm",
    "depends_on",
    "group",
    "just_no_deps",
    "just_recipe",
];

/// Top-level table names in runsteps.toml (used for suggestion when an unknown
/// key appears at the Config level rather than inside a table).
const KNOWN_TOP_LEVEL_KEYS: &[&str] = &["metadata", "steps"];

/// Suggest the closest key from a key list for an unknown field name.
/// Returns Some(suggestion) if Damerau-Levenshtein distance <= max(2, ceil(target_len / 3)).
fn suggest_from(unknown: &str, keys: &[&'static str]) -> Option<&'static str> {
    let threshold = std::cmp::max(2, unknown.len().div_ceil(3));
    keys.iter()
        .map(|&k| (k, strsim::damerau_levenshtein(unknown, k)))
        .filter(|&(_, d)| d <= threshold)
        .min_by_key(|&(_, d)| d)
        .map(|(k, _)| k)
}

/// Suggest the closest key in KNOWN_KEYS (field names) for an unknown field name.
/// Used in unit tests and available for future callers (e.g. schema subcommand).
#[allow(dead_code)]
pub fn suggest_key(unknown: &str) -> Option<&'static str> {
    suggest_from(unknown, KNOWN_KEYS)
}

/// Suggest from both field names and top-level table names combined.
/// For top-level keys, also tries prefix matching (e.g. "meta" → "metadata").
fn suggest_any_key(unknown: &str) -> Option<&'static str> {
    // Prefix match against top-level keys (handles "meta" → "metadata", "step" → "steps")
    let prefix_match = KNOWN_TOP_LEVEL_KEYS
        .iter()
        .find(|&&k| k.starts_with(unknown) || unknown.starts_with(k))
        .copied();
    if prefix_match.is_some() {
        return prefix_match;
    }
    // Distance-based match against top-level keys then field-level keys
    suggest_from(unknown, KNOWN_TOP_LEVEL_KEYS)
        .or_else(|| suggest_from(unknown, KNOWN_KEYS))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub metadata: Metadata,
    pub steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub name: String,
    pub description: Option<String>,
    pub justfile: Option<String>,
    pub working_directory: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub just_recipe: Option<String>,
    /// When `true`, passes `--no-deps` to `just`, skipping the recipe's prerequisites.
    /// Set this to restore v0.1.x isolation behavior for a specific step.
    /// Has no effect on `command` steps.
    #[serde(default)]
    pub just_no_deps: Option<bool>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub confirm: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

impl std::fmt::Display for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.description)
    }
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.steps.is_empty() {
            bail!("Config has no steps");
        }

        let mut seen_names = HashSet::new();
        let all_names: HashSet<&str> = self.steps.iter().map(|s| s.name.as_str()).collect();

        for step in &self.steps {
            // Exactly one of command/just_recipe
            match (&step.command, &step.just_recipe) {
                (None, None) => bail!(
                    "Step '{}' has neither command nor just_recipe",
                    step.name
                ),
                (Some(_), Some(_)) => bail!(
                    "Step '{}' has both command and just_recipe; use exactly one",
                    step.name
                ),
                _ => {}
            }

            // Unique names
            if !seen_names.insert(step.name.as_str()) {
                bail!("Duplicate step name: '{}'", step.name);
            }

            // Valid depends_on refs
            for dep in &step.depends_on {
                if !all_names.contains(dep.as_str()) {
                    bail!(
                        "Step '{}' depends on unknown step '{}'",
                        step.name,
                        dep
                    );
                }
            }
        }

        Ok(())
    }
}

/// Parse a toml::de::Error to extract an unknown field name if the error message
/// matches the toml 0.8 format: `unknown field `X`, expected one of `...``
fn extract_unknown_field(msg: &str) -> Option<&str> {
    // toml 0.8 format: "unknown field `fieldname`, ..."
    let after = msg.strip_prefix("unknown field `")?;
    let end = after.find('`')?;
    Some(&after[..end])
}

pub fn load_config(path: &str) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Cannot read config '{}': {}", path, e))?;

    toml::from_str::<Config>(&content).map_err(|e| {
        let msg = e.message();
        let span_info = e
            .span()
            .map(|s| {
                // Compute line:col from byte span start
                let before = &content[..s.start.min(content.len())];
                let line = before.lines().count();
                let col = before.rfind('\n').map(|p| s.start - p - 1).unwrap_or(s.start) + 1;
                format!("{}:{}:{}", path, line, col)
            })
            .unwrap_or_else(|| path.to_string());

        if let Some(unknown) = extract_unknown_field(msg) {
            match suggest_any_key(unknown) {
                Some(suggestion) => {
                    eprintln!("error: unknown field `{}` in {}", unknown, span_info);
                    eprintln!("  did you mean `{}`?", suggestion);
                }
                None => {
                    let keys = KNOWN_KEYS.join(", ");
                    eprintln!("error: unknown field `{}` in {}", unknown, span_info);
                    eprintln!("  known keys: {}", keys);
                }
            }
        } else {
            eprintln!("error: invalid TOML in {}: {}", span_info, msg);
        }

        anyhow::anyhow!("Failed to parse config '{}'", path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_step(name: &str, command: Option<&str>, just_recipe: Option<&str>) -> Step {
        Step {
            name: name.to_string(),
            description: format!("{} desc", name),
            command: command.map(String::from),
            just_recipe: just_recipe.map(String::from),
            just_no_deps: None,
            group: None,
            confirm: false,
            depends_on: vec![],
        }
    }

    #[test]
    fn test_deserialize_full() {
        let toml = r#"
[metadata]
name = "test"
description = "A test config"
justfile = "./justfile"
working_directory = "."

[[steps]]
name = "build"
description = "Build the project"
command = "cargo build"
group = "ci"
confirm = false
depends_on = []
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.metadata.name, "test");
        assert_eq!(config.steps.len(), 1);
        assert_eq!(config.steps[0].name, "build");
    }

    #[test]
    fn test_deserialize_minimal() {
        let toml = r#"
[metadata]
name = "minimal"

[[steps]]
name = "hello"
description = "Say hello"
command = "echo hello"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.metadata.description, None);
        assert!(!config.steps[0].confirm);
        assert!(config.steps[0].depends_on.is_empty());
    }

    #[test]
    fn test_deserialize_just_no_deps() {
        let toml = r#"
[metadata]
name = "test"

[[steps]]
name = "isolated"
description = "Run without prereqs"
just_recipe = "deploy"
just_no_deps = true
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.steps[0].just_no_deps, Some(true));
    }

    #[test]
    fn test_deserialize_just_no_deps_default() {
        let toml = r#"
[metadata]
name = "test"

[[steps]]
name = "normal"
description = "Normal recipe"
just_recipe = "deploy"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.steps[0].just_no_deps, None);
    }

    #[test]
    fn test_step_display() {
        let step = make_step("deploy", Some("make deploy"), None);
        assert_eq!(format!("{}", step), "deploy: deploy desc");
    }

    #[test]
    fn test_validate_passes() {
        let config = Config {
            metadata: Metadata {
                name: "ok".to_string(),
                description: None,
                justfile: None,
                working_directory: None,
            },
            steps: vec![make_step("a", Some("echo a"), None)],
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_steps() {
        let config = Config {
            metadata: Metadata {
                name: "empty".to_string(),
                description: None,
                justfile: None,
                working_directory: None,
            },
            steps: vec![],
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_neither() {
        let config = Config {
            metadata: Metadata {
                name: "x".to_string(),
                description: None,
                justfile: None,
                working_directory: None,
            },
            steps: vec![Step {
                name: "bad".to_string(),
                description: "bad".to_string(),
                command: None,
                just_recipe: None,
                just_no_deps: None,
                group: None,
                confirm: false,
                depends_on: vec![],
            }],
        };
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("neither"));
    }

    #[test]
    fn test_validate_both() {
        let config = Config {
            metadata: Metadata {
                name: "x".to_string(),
                description: None,
                justfile: None,
                working_directory: None,
            },
            steps: vec![Step {
                name: "bad".to_string(),
                description: "bad".to_string(),
                command: Some("echo".to_string()),
                just_recipe: Some("recipe".to_string()),
                just_no_deps: None,
                group: None,
                confirm: false,
                depends_on: vec![],
            }],
        };
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("both"));
    }

    #[test]
    fn test_validate_duplicate_names() {
        let config = Config {
            metadata: Metadata {
                name: "x".to_string(),
                description: None,
                justfile: None,
                working_directory: None,
            },
            steps: vec![
                make_step("dup", Some("echo 1"), None),
                make_step("dup", Some("echo 2"), None),
            ],
        };
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("Duplicate"));
    }

    #[test]
    fn test_validate_bad_depends_on() {
        let config = Config {
            metadata: Metadata {
                name: "x".to_string(),
                description: None,
                justfile: None,
                working_directory: None,
            },
            steps: vec![Step {
                name: "b".to_string(),
                description: "b".to_string(),
                command: Some("echo b".to_string()),
                just_recipe: None,
                just_no_deps: None,
                group: None,
                confirm: false,
                depends_on: vec!["nonexistent".to_string()],
            }],
        };
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("unknown step"));
    }

    // -----------------------------------------------------------------------
    // KNOWN_KEYS drift tests
    // -----------------------------------------------------------------------

    /// Hand-maintained list of all Metadata struct fields.
    /// MAINTENANCE: If you add a field to Metadata, update this list AND KNOWN_KEYS.
    fn metadata_fields() -> Vec<&'static str> {
        vec!["name", "description", "justfile", "working_directory"]
    }

    /// Hand-maintained list of all Step struct fields.
    /// MAINTENANCE: If you add a field to Step, update this list AND KNOWN_KEYS.
    fn step_fields() -> Vec<&'static str> {
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

    /// Every struct field must appear in KNOWN_KEYS (forward drift check).
    #[test]
    fn known_keys_covers_struct_fields() {
        let all_fields: Vec<&str> = metadata_fields()
            .into_iter()
            .chain(step_fields())
            .collect();

        let key_set: HashSet<&str> = KNOWN_KEYS.iter().copied().collect();

        let missing: Vec<&str> = all_fields
            .into_iter()
            .filter(|f| !key_set.contains(f))
            .collect();

        assert!(
            missing.is_empty(),
            "These struct fields are missing from KNOWN_KEYS: {:?}",
            missing
        );
    }

    /// Every KNOWN_KEYS entry must correspond to a real struct field (reciprocal drift check).
    #[test]
    fn known_keys_only_contains_real_fields() {
        let all_fields: HashSet<&str> = metadata_fields()
            .into_iter()
            .chain(step_fields())
            .collect();

        let phantom: Vec<&str> = KNOWN_KEYS
            .iter()
            .copied()
            .filter(|k| !all_fields.contains(k))
            .collect();

        assert!(
            phantom.is_empty(),
            "KNOWN_KEYS references non-existent fields: {:?}",
            phantom
        );
    }

    // -----------------------------------------------------------------------
    // suggest_key tests
    // -----------------------------------------------------------------------

    #[test]
    fn suggest_key_metadata_typo() {
        // "meta" is not in KNOWN_KEYS (metadata is not a field name — it's a table name).
        // "meta" has distance 4 from "name", 3 from "group" — threshold for len=4 is max(2,2)=2.
        // No key is close enough, so None is correct.
        assert_eq!(suggest_key("meta"), None);
        // Close typos of actual KNOWN_KEYS entries
        assert_eq!(suggest_key("just_recipee"), Some("just_recipe"));
        assert_eq!(suggest_key("comand"), Some("command"));
        assert_eq!(suggest_key("confirn"), Some("confirm"));
    }

    #[test]
    fn suggest_key_no_match_returns_none() {
        assert_eq!(suggest_key("completely_bogus_field_xyz"), None);
    }

    #[test]
    fn extract_unknown_field_parses_toml_error() {
        let msg = "unknown field `meta`, expected one of `name`, `description`";
        assert_eq!(extract_unknown_field(msg), Some("meta"));
    }
}
