use anyhow::{bail, Result};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub metadata: Metadata,
    pub steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub name: String,
    pub description: Option<String>,
    pub justfile: Option<String>,
    pub working_directory: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Step {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub just_recipe: Option<String>,
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

pub fn load_config(path: &str) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Cannot read config '{}': {}", path, e))?;
    let config: Config = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Invalid TOML in '{}': {}", path, e))?;
    Ok(config)
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
                group: None,
                confirm: false,
                depends_on: vec!["nonexistent".to_string()],
            }],
        };
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("unknown step"));
    }
}
