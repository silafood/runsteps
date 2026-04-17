use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

mod config;
mod display;
mod executor;
mod graph;
mod history;
mod picker;
mod preflight;
mod resolver;
mod schema;

use config::load_config;
use display::{
    print_banner, print_dry_run_header, print_failure, print_step_header, print_step_list,
    print_success,
};
use executor::{dry_run_step, execute_parallel_group, execute_step};
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

    /// Re-run the last successful session for this config [deprecated form: use `runsteps again`]
    #[arg(long, hide = true)]
    pub again: bool,

    /// Set a placeholder value: --var key=value (repeatable)
    #[arg(long, action = clap::ArgAction::Append, num_args = 1..)]
    pub var: Vec<String>,

    /// Activate a named profile from [profiles.<name>] in the config
    #[arg(long)]
    pub profile: Option<String>,
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

    /// Path to config file
    #[arg(short, long, default_value = "runsteps.toml")]
    pub config: String,
}

#[derive(Parser, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    pub shell: Shell,
}

#[derive(Parser, Debug)]
pub struct AgainArgs {
    /// Path to config file
    #[arg(short, long, default_value = "runsteps.toml")]
    pub config: String,

    /// Skip confirmations
    #[arg(short, long)]
    pub yes: bool,

    /// Dry run — show what would execute without executing
    #[arg(long)]
    pub dry_run: bool,

    /// Set a placeholder value: --var key=value (repeatable)
    #[arg(long, action = clap::ArgAction::Append, num_args = 1..)]
    pub var: Vec<String>,

    /// Activate a named profile from [profiles.<name>] in the config
    #[arg(long)]
    pub profile: Option<String>,
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

fn run_list(config_path: &str, group: Option<&str>, json_mode: bool) -> Result<()> {
    let config = load_config(config_path)?;
    config.validate()?;
    let steps: Vec<config::Step> = if let Some(g) = group {
        filter_by_group(&config.steps, g)
            .into_iter()
            .cloned()
            .collect()
    } else {
        config.steps.clone()
    };

    if json_mode {
        let step_values: Vec<serde_json::Value> = steps
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "description": s.description,
                    "group": s.group,
                    "depends_on": s.depends_on,
                    "type": if s.just_recipe.is_some() { "just_recipe" } else { "command" },
                    "confirm": s.confirm,
                })
            })
            .collect();
        let output = serde_json::json!({
            "version": 1,
            "steps": step_values,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    print_banner(&config.metadata);
    print_step_list(&steps);
    Ok(())
}

fn run_completions(args: &CompletionsArgs) -> Result<()> {
    let mut cmd = Cli::command();
    clap_complete::generate(args.shell, &mut cmd, "runsteps", &mut std::io::stdout());
    Ok(())
}

fn run_graph(args: &GraphArgs) -> Result<()> {
    let config = load_config(&args.config)?;
    config.validate()?;
    match graph::render_ascii(&config, args.group.as_deref()) {
        Ok(output) => {
            print!("{}", output);
            Ok(())
        }
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(2);
        }
    }
}

/// Resolve an optional profile name and return the profile (or default empty).
/// Exits with code 2 if the profile name is not found.
fn resolve_profile<'a>(
    config: &'a config::Config,
    profile_name: Option<&str>,
) -> &'a config::Profile {
    static DEFAULT_PROFILE: std::sync::OnceLock<config::Profile> = std::sync::OnceLock::new();
    let default = DEFAULT_PROFILE.get_or_init(config::Profile::default);

    match profile_name {
        None => default,
        Some(name) => match config.profiles.get(name) {
            Some(p) => p,
            None => {
                let available: Vec<&str> = config.profiles.keys().map(|k| k.as_str()).collect();
                let mut sorted = available.clone();
                sorted.sort();
                eprintln!(
                    "error: unknown profile '{}'; available: {}",
                    name,
                    if sorted.is_empty() {
                        "(none)".to_string()
                    } else {
                        sorted.join(", ")
                    }
                );
                std::process::exit(2);
            }
        },
    }
}

fn run_again(args: &AgainArgs) -> Result<()> {
    let config_path = std::fs::canonicalize(&args.config)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| args.config.clone());

    let entry = match history::latest_entry_for(&config_path) {
        Some(e) => e,
        None => {
            anyhow::bail!(
                "no history found for '{}'; run without --again first",
                config_path
            );
        }
    };

    let config = load_config(&args.config)?;
    config.validate()?;

    // Check if the config has changed since the last run.
    let config_bytes = std::fs::read(&args.config)?;
    let current_sha = history::sha256_hex(&config_bytes);
    if current_sha != entry.config_sha256 {
        eprintln!("warning: config has changed since last run (SHA-256 mismatch)");
    }

    ensure_just_available(&config.steps)?;
    print_banner(&config.metadata);

    let vars = resolver::parse_var_flags(&args.var)?;

    // Reconstruct the step list from history, skipping steps no longer present.
    let step_map: std::collections::HashMap<&str, &config::Step> =
        config.steps.iter().map(|s| (s.name.as_str(), s)).collect();

    let mut selected: Vec<config::Step> = Vec::new();
    for name in &entry.selected {
        match step_map.get(name.as_str()) {
            Some(&step) => selected.push(step.clone()),
            None => eprintln!("skipping missing step {}", name),
        }
    }

    if selected.is_empty() {
        anyhow::bail!("no steps to replay (all previously selected steps are missing)");
    }

    if args.dry_run {
        println!("Dry run — the following would execute (replayed):");
        for step in &selected {
            print_dry_run_header(step);
            dry_run_step(step, &config.metadata);
        }
        return Ok(());
    }

    let mut executed: std::collections::HashSet<String> = std::collections::HashSet::new();
    for step in &selected {
        if executed.contains(&step.name) {
            continue;
        }
        print_step_header(step);
        match execute_step(step, &config.metadata, args.yes, &vars) {
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

fn run_run(args: &RunArgs) -> Result<()> {
    // --again flag at top level delegates to again logic
    if args.again {
        let again_args = AgainArgs {
            config: args.config.clone(),
            yes: args.yes,
            dry_run: args.dry_run,
            var: args.var.clone(),
            profile: args.profile.clone(),
        };
        return run_again(&again_args);
    }

    let config = load_config(&args.config)?;
    config.validate()?;

    // Resolve profile early so we can exit 2 before doing any real work.
    let profile = resolve_profile(&config, args.profile.as_deref());

    ensure_just_available(&config.steps)?;

    let vars = resolver::parse_var_flags(&args.var)?;

    print_banner(&config.metadata);

    // Apply profile group filter (intersection with --group CLI flag).
    let effective_group: Option<String> = match (args.group.as_deref(), profile.groups.is_empty()) {
        (Some(g), _) if !profile.groups.is_empty() => {
            // Both --group and profile.groups set: use --group only if it's in profile.groups.
            if profile.groups.iter().any(|pg| pg == g) {
                Some(g.to_string())
            } else {
                // Group not in profile: no steps match.
                Some("__no_match__".to_string())
            }
        }
        (Some(g), _) => Some(g.to_string()),
        (None, false) => None, // profile.groups non-empty but --group not set: handled below
        (None, true) => None,
    };

    let available_steps: Vec<config::Step> = {
        let mut steps: Vec<config::Step> = if let Some(ref group) = effective_group {
            filter_by_group(&config.steps, group)
                .into_iter()
                .cloned()
                .collect()
        } else if !profile.groups.is_empty() {
            // Profile restricts to specific groups (no --group CLI flag).
            config
                .steps
                .iter()
                .filter(|s| {
                    s.group
                        .as_deref()
                        .map(|g| profile.groups.iter().any(|pg| pg == g))
                        .unwrap_or(false)
                })
                .cloned()
                .collect()
        } else {
            config.steps.clone()
        };

        // Apply profile excluded_steps.
        if !profile.excluded_steps.is_empty() {
            steps.retain(|s| !profile.excluded_steps.contains(&s.name));
        }

        steps
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

    // Profile skip_confirms: treat as --yes for confirm steps.
    let effective_yes = args.yes || profile.skip_confirms;

    if args.dry_run {
        println!("Dry run — the following would execute:");
        for step in &selected {
            print_dry_run_header(step);
            dry_run_step(step, &config.metadata);
        }
        return Ok(());
    }

    validate_dependencies(&mut selected, &config.steps, effective_yes)?;

    if !args.all && !effective_yes {
        let proceed = inquire::Confirm::new("Run selected steps?")
            .with_default(true)
            .prompt()?;
        if !proceed {
            return Ok(());
        }
    }

    let mut executed: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut ran_names: Vec<String> = Vec::new();

    // Process steps, grouping consecutive parallel=true steps that have no
    // confirm=true (which would require interactive TTY, incompatible with
    // concurrent output).
    let mut i = 0;
    while i < selected.len() {
        let step = &selected[i];

        if executed.contains(&step.name) {
            i += 1;
            continue;
        }

        // Check if this is the start of a parallel group.
        if step.parallel && !step.confirm {
            // Collect consecutive parallel=true, confirm=false steps.
            let mut group: Vec<&config::Step> = Vec::new();
            let mut j = i;
            while j < selected.len() {
                let s = &selected[j];
                if s.parallel && !s.confirm && !executed.contains(&s.name) {
                    group.push(s);
                    j += 1;
                } else {
                    break;
                }
            }

            if group.len() == 1 {
                // Single parallel step — run sequentially to avoid overhead.
                let s = group[0];
                print_step_header(s);
                match execute_step(s, &config.metadata, effective_yes, &vars) {
                    Ok(()) => {
                        print_success(s);
                        executed.insert(s.name.clone());
                        ran_names.push(s.name.clone());
                    }
                    Err(e) => {
                        print_failure(s);
                        return Err(e);
                    }
                }
                i += 1;
            } else {
                // True parallel group.
                match execute_parallel_group(&group, &config.metadata, &vars) {
                    Ok(()) => {
                        for s in &group {
                            executed.insert(s.name.clone());
                            ran_names.push(s.name.clone());
                        }
                    }
                    Err(e) => {
                        eprintln!("error: parallel group failed");
                        return Err(e);
                    }
                }
                i = j;
            }
        } else {
            // Sequential step.
            print_step_header(step);
            match execute_step(step, &config.metadata, effective_yes, &vars) {
                Ok(()) => {
                    print_success(step);
                    executed.insert(step.name.clone());
                    ran_names.push(step.name.clone());
                }
                Err(e) => {
                    print_failure(step);
                    return Err(e);
                }
            }
            i += 1;
        }
    }

    // Persist history if any steps ran.
    if !ran_names.is_empty() {
        let config_path = std::fs::canonicalize(&args.config)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| args.config.clone());
        let config_bytes = std::fs::read(&args.config).unwrap_or_default();
        let config_sha256 = history::sha256_hex(&config_bytes);
        let entry = history::HistoryEntry {
            config_path,
            config_sha256,
            selected: ran_names,
            timestamp: history::now_rfc3339(),
        };
        // Non-fatal — history save failure should not abort the run.
        if let Err(e) = history::save_history_entry(entry) {
            eprintln!("warning: could not save history: {}", e);
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
                return run_list(
                    &cli.run_args.config,
                    cli.run_args.group.as_deref(),
                    false,
                );
            }
            run_run(&cli.run_args)
        }
        Some(Commands::Run(args)) => run_run(&args),
        Some(Commands::Schema(args)) => run_schema(&args),
        Some(Commands::Init(args)) => run_init(&args.path),
        Some(Commands::List(args)) => {
            run_list(&args.config.clone(), args.group.as_deref(), args.json)
        }
        Some(Commands::Graph(args)) => run_graph(&args),
        Some(Commands::Completions(args)) => run_completions(&args),
        Some(Commands::Again(args)) => run_again(&args),
    }
}
