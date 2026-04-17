use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

mod config;
mod display;
mod executor;
mod picker;
mod preflight;
mod schema;

use config::load_config;
use display::{
    print_banner, print_dry_run_header, print_failure, print_step_header, print_step_list,
    print_success,
};
use executor::{dry_run_step, execute_step};
use picker::{filter_by_group, pick_steps, validate_dependencies};
use preflight::ensure_just_available;

// ---------------------------------------------------------------------------
// Subcommand argument structs
// ---------------------------------------------------------------------------

#[derive(Parser, Debug, Default)]
pub struct RunArgs {
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

    /// List available steps and exit [deprecated: use `runsteps list`]
    #[arg(long, hide = true)]
    pub list: bool,

    /// Generate a template config file and exit [deprecated: use `runsteps init`]
    #[arg(long, hide = true)]
    pub init: bool,

    /// Filter steps by group
    #[arg(short, long)]
    pub group: Option<String>,
}

#[derive(Parser, Debug)]
pub struct SchemaArgs {
    /// Output JSON Schema instead of human-readable format
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct InitArgs {
    /// Path for the generated config file (default: runsteps.toml)
    #[arg(default_value = "runsteps.toml")]
    pub path: String,
}

#[derive(Parser, Debug)]
pub struct ListArgs {
    /// Output steps as JSON
    #[arg(long)]
    pub json: bool,

    /// Filter steps by group
    #[arg(short, long)]
    pub group: Option<String>,

    /// Path to config file
    #[arg(short, long, default_value = "runsteps.toml")]
    pub config: String,
}

#[derive(Parser, Debug)]
pub struct GraphArgs {
    /// Filter by group
    #[arg(short, long)]
    pub group: Option<String>,
}

#[derive(Parser, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    pub shell: Shell,
}

#[derive(Parser, Debug)]
pub struct AgainArgs {
    /// Skip confirmations
    #[arg(short, long)]
    pub yes: bool,
}

// ---------------------------------------------------------------------------
// Top-level CLI
// ---------------------------------------------------------------------------

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run selected steps interactively (default when no subcommand given)
    Run(RunArgs),
    /// Print the runsteps config schema
    Schema(SchemaArgs),
    /// Generate a template config file
    Init(InitArgs),
    /// List available steps
    List(ListArgs),
    /// Show step dependency graph [coming in Phase D]
    Graph(GraphArgs),
    /// Generate shell completions
    Completions(CompletionsArgs),
    /// Re-run the last successful session [coming in Phase C]
    Again(AgainArgs),
}

#[derive(Parser)]
#[command(name = "runsteps", version, about = "Interactive config-driven task runner")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[command(flatten)]
    pub run_args: RunArgs,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Deprecation helper
// ---------------------------------------------------------------------------

fn emit_deprecation(flag: &str, replacement: &str) {
    if std::env::var("RUNSTEPS_NO_WARNINGS").as_deref() == Ok("1") {
        return;
    }
    eprintln!(
        "warning: top-level flag `{}` is deprecated; use `runsteps {}` instead. Will be removed in v0.5.0.",
        flag, replacement
    );
}

// ---------------------------------------------------------------------------
// Template config for --init / init subcommand
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Subcommand handlers
// ---------------------------------------------------------------------------

fn run_schema(args: &SchemaArgs) -> Result<()> {
    if args.json {
        println!("{}", schema::schema_json());
        return Ok(());
    }

    // Human-readable output
    use console::style;
    for table in schema::SCHEMA {
        println!();
        println!("{}", style(format!("[{}]", table.name)).bold().cyan());
        println!();
        for field in table.fields {
            let req = if field.required {
                style(" [required]").yellow().to_string()
            } else {
                String::new()
            };
            println!(
                "  {}{} : {}{}  (added {})",
                style(field.name).bold(),
                req,
                style(field.ty).dim(),
                String::new(),
                style(field.added_in).dim()
            );
            // Wrap description at ~72 chars
            let desc = field.description;
            if desc.len() <= 68 {
                println!("      {}", style(desc).dim());
            } else {
                // Simple word-wrap
                let mut line = String::new();
                for word in desc.split_whitespace() {
                    if !line.is_empty() && line.len() + 1 + word.len() > 68 {
                        println!("      {}", style(&line).dim());
                        line.clear();
                    }
                    if !line.is_empty() {
                        line.push(' ');
                    }
                    line.push_str(word);
                }
                if !line.is_empty() {
                    println!("      {}", style(&line).dim());
                }
            }
        }
    }
    println!();
    Ok(())
}

fn run_init(path: &str) -> Result<()> {
    let config_path = if !path.ends_with(".toml") {
        format!("{}.toml", path)
    } else {
        path.to_string()
    };
    let p = std::path::Path::new(&config_path);
    if p.exists() {
        anyhow::bail!(
            "{} already exists. Remove it first or use a different path.",
            config_path
        );
    }
    std::fs::write(p, TEMPLATE_CONFIG)?;
    println!(
        "Created {} — edit it with your steps, then run `runsteps`.",
        config_path
    );
    Ok(())
}

fn run_list(config_path: &str, group: Option<&str>) -> Result<()> {
    let config = load_config(config_path)?;
    config.validate()?;
    print_banner(&config.metadata);
    let steps: Vec<config::Step> = if let Some(g) = group {
        filter_by_group(&config.steps, g)
            .into_iter()
            .cloned()
            .collect()
    } else {
        config.steps.clone()
    };
    print_step_list(&steps);
    Ok(())
}

fn run_completions(args: &CompletionsArgs) -> Result<()> {
    let mut cmd = Cli::command();
    clap_complete::generate(args.shell, &mut cmd, "runsteps", &mut std::io::stdout());
    Ok(())
}

fn run_graph(_args: &GraphArgs) -> Result<()> {
    eprintln!("graph subcommand is not yet implemented (coming in Phase D)");
    std::process::exit(2);
}

fn run_again(_args: &AgainArgs) -> Result<()> {
    eprintln!("again subcommand is not yet implemented (coming in Phase C)");
    std::process::exit(2);
}

fn run_run(args: &RunArgs) -> Result<()> {
    let config = load_config(&args.config)?;
    config.validate()?;

    ensure_just_available(&config.steps)?;

    print_banner(&config.metadata);

    let available_steps: Vec<config::Step> = if let Some(ref group) = args.group {
        filter_by_group(&config.steps, group)
            .into_iter()
            .cloned()
            .collect()
    } else {
        config.steps.clone()
    };

    if args.list {
        emit_deprecation("--list", "list");
        print_step_list(&available_steps);
        return Ok(());
    }

    if args.init {
        emit_deprecation("--init", "init");
        return run_init(&args.config);
    }

    let mut selected = if args.all {
        available_steps.clone()
    } else {
        pick_steps(&available_steps)?
    };

    if args.dry_run {
        println!("Dry run — the following would execute:");
        for step in &selected {
            print_dry_run_header(step);
            dry_run_step(step, &config.metadata);
        }
        return Ok(());
    }

    validate_dependencies(&mut selected, &config.steps, args.yes)?;

    if !args.all && !args.yes {
        let proceed = inquire::Confirm::new("Run selected steps?")
            .with_default(true)
            .prompt()?;
        if !proceed {
            return Ok(());
        }
    }

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

// ---------------------------------------------------------------------------
// Main dispatch
// ---------------------------------------------------------------------------

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => {
            // Legacy flag dual-path: --list and --init handled inside run_run
            // with deprecation warnings when the flags are set.
            // Pure run flow when neither flag is set.
            if cli.run_args.init {
                emit_deprecation("--init", "init");
                return run_init(&cli.run_args.config);
            }
            if cli.run_args.list {
                emit_deprecation("--list", "list");
                let config = load_config(&cli.run_args.config)?;
                config.validate()?;
                print_banner(&config.metadata);
                let steps: Vec<config::Step> = if let Some(ref group) = cli.run_args.group {
                    filter_by_group(&config.steps, group)
                        .into_iter()
                        .cloned()
                        .collect()
                } else {
                    config.steps.clone()
                };
                print_step_list(&steps);
                return Ok(());
            }
            run_run(&cli.run_args)
        }
        Some(Commands::Run(args)) => run_run(&args),
        Some(Commands::Schema(args)) => run_schema(&args),
        Some(Commands::Init(args)) => run_init(&args.path),
        Some(Commands::List(args)) => run_list(&args.config.clone(), args.group.as_deref()),
        Some(Commands::Graph(args)) => run_graph(&args),
        Some(Commands::Completions(args)) => run_completions(&args),
        Some(Commands::Again(args)) => run_again(&args),
    }
}
