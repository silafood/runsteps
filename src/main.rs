use anyhow::Result;
use clap::Parser;

mod config;
mod display;
mod executor;
mod picker;
mod preflight;

use config::load_config;
use display::{
    print_banner, print_dry_run_header, print_failure, print_step_header, print_step_list,
    print_success,
};
use executor::{dry_run_step, execute_step};
use picker::{filter_by_group, pick_steps, validate_dependencies};
use preflight::ensure_just_available;

#[derive(Parser)]
#[command(name = "runsteps", about = "Interactive config-driven task runner", version)]
pub struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "runsteps.toml")]
    pub config: String,

    /// Run all steps without interactive picker
    #[arg(long)]
    pub all: bool,

    /// Skip all confirmations (including per-step confirm: true)
    #[arg(short, long)]
    pub yes: bool,

    /// Dry run — show what would execute
    #[arg(long)]
    pub dry_run: bool,

    /// List available steps and exit
    #[arg(long)]
    pub list: bool,

    /// Filter steps by group
    #[arg(short, long)]
    pub group: Option<String>,

    /// Generate a template config file and exit
    #[arg(long)]
    pub init: bool,
}

fn main() {
    match run() {
        Ok(()) => {}
        Err(e) => {
            if let Some(
                inquire::InquireError::OperationCanceled
                | inquire::InquireError::OperationInterrupted,
            ) = e.downcast_ref::<inquire::InquireError>()
            {
                eprintln!("Aborted.");
                std::process::exit(0);
            }
            eprintln!("Error: {:#}", e);
            std::process::exit(1);
        }
    }
}

const TEMPLATE_CONFIG: &str = r#"[metadata]
name = "my-project"
description = "Project deployment workflow"
# justfile = "./justfile"
# working_directory = "."

[[steps]]
name = "build"
description = "Build the project"
command = "echo 'Building...'"
group = "ci"

[[steps]]
name = "test"
description = "Run tests"
command = "echo 'Testing...'"
group = "ci"

[[steps]]
name = "deploy"
description = "Deploy to production"
command = "echo 'Deploying...'"
group = "deploy"
confirm = true
depends_on = ["build", "test"]
"#;

fn run() -> Result<()> {
    let args = Cli::parse();

    if args.init {
        let config_path = if !args.config.ends_with(".toml") {
            format!("{}.toml", args.config)
        } else {
            args.config.clone()
        };
        let path = std::path::Path::new(&config_path);
        if path.exists() {
            anyhow::bail!("{} already exists. Remove it first or use -c to specify a different path.", config_path);
        }
        std::fs::write(path, TEMPLATE_CONFIG)?;
        println!("Created {} — edit it with your steps, then run `runsteps`.", config_path);
        return Ok(());
    }

    // Load and validate config
    let config = load_config(&args.config)?;
    config.validate()?;

    // Preflight: check just is available if needed
    ensure_just_available(&config.steps)?;

    print_banner(&config.metadata);

    // Apply group filter
    let available_steps: Vec<config::Step> = if let Some(ref group) = args.group {
        filter_by_group(&config.steps, group)
            .into_iter()
            .cloned()
            .collect()
    } else {
        config.steps.clone()
    };

    // --list: print and exit
    if args.list {
        print_step_list(&available_steps);
        return Ok(());
    }

    // Select steps
    let mut selected = if args.all {
        available_steps.clone()
    } else {
        pick_steps(&available_steps)?
    };

    // --dry-run: print commands and exit
    if args.dry_run {
        println!("Dry run — the following would execute:");
        for step in &selected {
            print_dry_run_header(step);
            dry_run_step(step, &config.metadata);
        }
        return Ok(());
    }

    // Validate dependencies (warn + offer-to-include)
    validate_dependencies(&mut selected, &config.steps, args.yes)?;

    // Global confirmation (skipped with --all or --yes)
    if !args.all && !args.yes {
        let proceed = inquire::Confirm::new("Run selected steps?")
            .with_default(true)
            .prompt()?;
        if !proceed {
            return Ok(());
        }
    }

    // Execute — state machine tracks which steps have run; skips duplicates
    let mut executed: std::collections::HashSet<String> = std::collections::HashSet::new();
    for step in &selected {
        if executed.contains(&step.name) {
            continue;
        }
        print_step_header(step);
        match execute_step(step, &config.metadata, args.yes) {
            Ok(()) => {
                print_success(step);
                executed.insert(step.name.clone());
            }
            Err(e) => {
                print_failure(step);
                return Err(e);
            }
        }
    }

    Ok(())
}
