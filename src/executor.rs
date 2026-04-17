use crate::config::{Metadata, Step};
use crate::resolver;
use anyhow::Result;
use std::collections::HashMap;
use std::process::Command;

pub fn execute_step(
    step: &Step,
    meta: &Metadata,
    skip_confirm: bool,
    vars: &HashMap<String, String>,
) -> Result<()> {
    if step.confirm && !skip_confirm {
        let ok = inquire::Confirm::new(&format!("Confirm: {}?", step.name))
            .with_default(false)
            .prompt()?;
        if !ok {
            crate::display::print_skip(step);
            return Ok(());
        }
    }

    // Warn about orphan prompts entries.
    resolver::warn_orphan_prompts(&step.name, &step.args, &step.prompts);

    // Resolve env values (no shell escaping; newline rejection applies).
    let resolved_env = resolver::resolve_env(
        &step.name,
        &step.env,
        &step.prompts,
        vars,
        skip_confirm,
    )?;

    let status = if let Some(recipe) = &step.just_recipe {
        // Resolve args (no shell escaping — passed as argv to just).
        let resolved_args = resolver::resolve_args(
            &step.name,
            &step.args,
            &step.prompts,
            vars,
            skip_confirm,
            false, // just_recipe: no shell escape
        )?;

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
        cmd.arg(recipe);
        cmd.args(&resolved_args);
        cmd.envs(&resolved_env);
        cmd.status()?
    } else if let Some(command) = &step.command {
        // For command steps, resolve placeholders in the command string itself (no shell escape
        // for the base command — it is a shell template). Then resolve args with shell escaping.
        let resolved_command = resolver::resolve_placeholders(
            command,
            &step.name,
            vars,
            &step.prompts,
            skip_confirm,
            false, // the command string is already shell syntax; don't double-escape
        )?;

        let shell_escape = !step.raw;
        let resolved_args = resolver::resolve_args(
            &step.name,
            &step.args,
            &step.prompts,
            vars,
            skip_confirm,
            shell_escape,
        )?;

        // Build the full command string: base command + resolved args appended.
        let full_command = if resolved_args.is_empty() {
            resolved_command
        } else {
            format!("{} {}", resolved_command, resolved_args.join(" "))
        };

        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&full_command);
        if let Some(wd) = &meta.working_directory {
            cmd.current_dir(wd);
        }
        cmd.envs(&resolved_env);
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
    // Print env if present.
    let mut env_keys: Vec<&String> = step.env.keys().collect();
    env_keys.sort();
    for k in &env_keys {
        println!("  env: {}={}", k, step.env[*k]);
    }

    let cmd = if let Some(recipe) = &step.just_recipe {
        let justfile_arg = meta
            .justfile
            .as_deref()
            .map(|jf| format!("--justfile {} ", jf))
            .unwrap_or_default();
        let args_str = if step.args.is_empty() {
            String::new()
        } else {
            format!(" {}", step.args.join(" "))
        };
        if step.just_no_deps.unwrap_or(false) {
            format!("just {}--no-deps {}{}", justfile_arg, recipe, args_str)
        } else {
            format!("just {}{}{}", justfile_arg, recipe, args_str)
        }
    } else if let Some(command) = &step.command {
        let args_str = if step.args.is_empty() {
            String::new()
        } else {
            format!(" {}", step.args.join(" "))
        };
        format!("sh -c '{}{}'", command, args_str)
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
