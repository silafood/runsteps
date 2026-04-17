use crate::config::{Metadata, Step};
use anyhow::Result;
use std::process::Command;

pub fn execute_step(step: &Step, meta: &Metadata, skip_confirm: bool) -> Result<()> {
    if step.confirm && !skip_confirm {
        let ok = inquire::Confirm::new(&format!("Confirm: {}?", step.name))
            .with_default(false)
            .prompt()?;
        if !ok {
            crate::display::print_skip(step);
            return Ok(());
        }
    }

    let status = if let Some(recipe) = &step.just_recipe {
        let mut cmd = Command::new("just");
        if let Some(jf) = &meta.justfile {
            cmd.arg("--justfile").arg(jf);
        }
        if let Some(wd) = &meta.working_directory {
            cmd.current_dir(wd);
        }
        if step.just_no_deps.unwrap_or(false) {
            cmd.arg("--no-deps");
        }
        cmd.arg(recipe).status()?
    } else if let Some(command) = &step.command {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        if let Some(wd) = &meta.working_directory {
            cmd.current_dir(wd);
        }
        cmd.status()?
    } else {
        anyhow::bail!("Step '{}' has neither command nor just_recipe", step.name);
    };

    if !status.success() {
        anyhow::bail!(
            "Step '{}' failed with exit code {:?}",
            step.name,
            status.code()
        );
    }
    Ok(())
}

pub fn dry_run_step(step: &Step, meta: &Metadata) {
    let cmd = if let Some(recipe) = &step.just_recipe {
        let justfile_arg = meta
            .justfile
            .as_deref()
            .map(|jf| format!("--justfile {} ", jf))
            .unwrap_or_default();
        if step.just_no_deps.unwrap_or(false) {
            format!("just {}--no-deps {}", justfile_arg, recipe)
        } else {
            format!("just {}{}", justfile_arg, recipe)
        }
    } else if let Some(command) = &step.command {
        format!("sh -c '{}'", command)
    } else {
        "(no command)".to_string()
    };

    let wd_note = meta
        .working_directory
        .as_deref()
        .map(|wd| format!(" [in {}]", wd))
        .unwrap_or_default();

    println!("  [dry-run] {}{}", cmd, wd_note);
}
