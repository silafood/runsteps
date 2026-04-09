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

fn run() -> Result<()> {
    let args = Cli::parse();

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

    // Execute
    for step in &selected {
        print_step_header(step);
        match execute_step(step, &config.metadata, args.yes) {
            Ok(()) => print_success(step),
            Err(e) => {
                print_failure(step);
                return Err(e);
            }
        }
    }

    Ok(())
}
