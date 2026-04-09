use crate::config::Step;
use anyhow::Result;
use inquire::MultiSelect;

pub fn pick_steps(steps: &[Step]) -> Result<Vec<Step>> {
    let selected = MultiSelect::new("Select steps to run:", steps.to_vec())
        .with_page_size(15)
        .prompt()?;

    if selected.is_empty() {
        anyhow::bail!("No steps selected");
    }

    Ok(selected)
}

pub fn filter_by_group<'a>(steps: &'a [Step], group: &str) -> Vec<&'a Step> {
    steps
        .iter()
        .filter(|s| s.group.as_deref() == Some(group))
        .collect()
}

pub fn validate_dependencies(
    selected: &mut Vec<Step>,
    all_steps: &[Step],
    skip_confirm: bool,
) -> Result<()> {
    let selected_names: std::collections::HashSet<String> =
        selected.iter().map(|s| s.name.clone()).collect();

    let mut missing: Vec<String> = vec![];
    for step in selected.iter() {
        for dep in &step.depends_on {
            if !selected_names.contains(dep) && !missing.contains(dep) {
                missing.push(dep.clone());
            }
        }
    }

    if missing.is_empty() {
        return Ok(());
    }

    eprintln!(
        "\n  Warning: the following dependencies are not selected: {}",
        missing.join(", ")
    );

    let include = if skip_confirm {
        true
    } else {
        inquire::Confirm::new("Include missing dependencies?")
            .with_default(true)
            .prompt()?
    };

    if include {
        for dep_name in &missing {
            if let Some(dep_step) = all_steps.iter().find(|s| &s.name == dep_name) {
                let insert_pos = selected
                    .iter()
                    .position(|s| s.depends_on.contains(dep_name))
                    .unwrap_or(0);
                selected.insert(insert_pos, dep_step.clone());
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_step(name: &str, group: Option<&str>, depends_on: Vec<&str>) -> Step {
        Step {
            name: name.to_string(),
            description: format!("{} desc", name),
            command: Some(format!("echo {}", name)),
            just_recipe: None,
            group: group.map(String::from),
            confirm: false,
            depends_on: depends_on.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn test_filter_by_group_matches() {
        let steps = vec![
            make_step("a", Some("setup"), vec![]),
            make_step("b", Some("deploy"), vec![]),
            make_step("c", Some("setup"), vec![]),
        ];
        let filtered = filter_by_group(&steps, "setup");
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "a");
        assert_eq!(filtered[1].name, "c");
    }

    #[test]
    fn test_filter_by_group_no_match() {
        let steps = vec![make_step("a", Some("setup"), vec![])];
        let filtered = filter_by_group(&steps, "other");
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_validate_deps_all_selected() {
        let all = vec![
            make_step("a", None, vec![]),
            make_step("b", None, vec!["a"]),
        ];
        let mut selected = all.clone();
        assert!(validate_dependencies(&mut selected, &all, false).is_ok());
    }

    #[test]
    fn test_validate_deps_auto_include() {
        let all = vec![
            make_step("a", None, vec![]),
            make_step("b", None, vec!["a"]),
        ];
        let mut selected = vec![all[1].clone()]; // only "b" selected, not "a"
        validate_dependencies(&mut selected, &all, true).unwrap(); // skip_confirm=true
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].name, "a");
        assert_eq!(selected[1].name, "b");
    }

    #[test]
    fn test_validate_deps_config_order() {
        let all = vec![
            make_step("setup", None, vec![]),
            make_step("build", None, vec!["setup"]),
            make_step("test", None, vec!["build"]),
        ];
        // Selected: test only
        let mut selected = vec![all[2].clone()];
        validate_dependencies(&mut selected, &all, true).unwrap();
        // "build" (dep of "test") should be inserted at pos 0
        assert!(selected.iter().any(|s| s.name == "build"));
    }
}
