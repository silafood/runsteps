# runsteps — Interactive Config-Driven Task Runner

## Project Overview

`runsteps` is a Rust CLI that reads a TOML config defining named steps (each either a `just` recipe or a raw shell command), presents an interactive multi-select picker to the user, and executes the selected steps in order. Think of it as an interactive wrapper around justfiles and shell commands for deployment/infra workflows.

## Tech Stack

- **Language:** Rust (edition 2024, MSRV 1.85)
- **CLI framework:** `clap` 4.5
- **Interactive prompts:** `inquire` 0.9.1 (MultiSelect, Select, Confirm)
- **Config:** `serde` 1.x + `toml` 0.8
- **Terminal styling:** `console` 0.15
- **Error handling:** `anyhow` 1.x
- **Build:** cargo

## Architecture

```
src/
├── main.rs           # CLI entry point (clap), loads config, runs interactive flow
├── config.rs         # TOML config types (serde deserialize)
├── preflight.rs      # Dependency checks (is `just` installed? offer to install)
├── picker.rs         # Interactive step selection (inquire MultiSelect)
├── executor.rs       # Step execution (std::process::Command for both just and raw cmds)
└── display.rs        # Terminal output formatting (console crate)
```

Single binary. No async needed — this is a synchronous CLI tool.

## Config Format (`runsteps.toml`)

```toml
[metadata]
name = "my-infra"
description = "Infrastructure deployment steps"
justfile = "./justfile"           # optional, path to justfile (default: ./justfile)
working_directory = "."           # optional, cwd for commands

[[steps]]
name = "add-helm-repo"
description = "Add Helm chart repo and update"
command = "helm repo add bitnami https://charts.bitnami.com/bitnami && helm repo update"
group = "setup"                   # optional grouping

[[steps]]
name = "install-crds"
description = "Install Custom Resource Definitions"
just_recipe = "install-crds"      # calls: just install-crds
group = "setup"

[[steps]]
name = "deploy"
description = "Deploy via Helm with values"
just_recipe = "deploy"
group = "deploy"
confirm = true                    # ask "Are you sure?" before executing
depends_on = ["add-helm-repo"]    # must also be selected, warn if not
```

## Rust Types

```rust
use serde::Deserialize;

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
```

## Just Dependency Check

Before running anything, check if any steps use `just_recipe`. If so, verify `just` is installed. If not, tell the user how to install it and bail.

```rust
use std::process::Command;

pub fn ensure_just_available(steps: &[Step]) -> anyhow::Result<()> {
    let needs_just = steps.iter().any(|s| s.just_recipe.is_some());
    if !needs_just {
        return Ok(());
    }

    match Command::new("just").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            eprintln!("  ✓ just {}", version.trim());
            Ok(())
        }
        _ => {
            eprintln!("  ✗ `just` is not installed but required by this config.");
            eprintln!();
            eprintln!("  Install it with one of:");
            eprintln!("    cargo install just");
            eprintln!("    brew install just");
            eprintln!("    curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /usr/local/bin");
            eprintln!();

            let install = inquire::Confirm::new("Attempt `cargo install just` now?")
                .with_default(false)
                .prompt()?;

            if install {
                let status = Command::new("cargo")
                    .args(["install", "just"])
                    .status()?;
                if !status.success() {
                    anyhow::bail!("Failed to install just via cargo");
                }
                eprintln!("  ✓ just installed successfully");
                Ok(())
            } else {
                anyhow::bail!("Cannot proceed without `just`. Install it and try again.");
            }
        }
    }
}
```

Call this early in `main()`, right after loading the config:

```rust
let config = load_config(&args.config)?;
ensure_just_available(&config.steps)?;
```

## Core Flow

```rust
// main.rs pseudocode
fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    let config = load_config(&args.config)?;

    // 0. Check just is available if needed
    ensure_just_available(&config.steps)?;

    // 1. Show grouped multi-select
    let selected = pick_steps(&config.steps)?;

    // 2. Validate dependencies
    validate_dependencies(&selected, &config.steps)?;

    // 3. Confirm
    let proceed = inquire::Confirm::new("Run selected steps?")
        .with_default(true)
        .prompt()?;
    if !proceed { return Ok(()); }

    // 4. Execute in order
    for step in &selected {
        execute_step(step, &config.metadata)?;
    }
    Ok(())
}
```

## Step Execution

```rust
use std::process::Command;

pub fn execute_step(step: &Step, meta: &Metadata) -> anyhow::Result<()> {
    if step.confirm {
        let ok = inquire::Confirm::new(&format!("Confirm: {}?", step.name))
            .with_default(false)
            .prompt()?;
        if !ok {
            println!("  Skipped: {}", step.name);
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
        cmd.arg(recipe).status()?
    } else if let Some(command) = &step.command {
        Command::new("sh")
            .arg("-c")
            .arg(command)
            .status()?
    } else {
        anyhow::bail!("Step '{}' has neither command nor just_recipe", step.name);
    };

    if !status.success() {
        anyhow::bail!("Step '{}' failed with exit code {:?}", step.name, status.code());
    }
    Ok(())
}
```

## Interactive Picker

```rust
use inquire::MultiSelect;

pub fn pick_steps(steps: &[Step]) -> anyhow::Result<Vec<Step>> {
    let selected = MultiSelect::new("Select steps to run:", steps.to_vec())
        .with_page_size(15)
        .prompt()?;

    if selected.is_empty() {
        anyhow::bail!("No steps selected");
    }

    Ok(selected)
}
```

## CLI Arguments (clap)

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "runsteps", about = "Interactive config-driven task runner")]
pub struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "runsteps.toml")]
    pub config: String,

    /// Run all steps without prompting
    #[arg(long)]
    pub all: bool,

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
```

## What NOT to Do

- **Do NOT use `just` as a library.** The public API has no semver guarantees and will break.
- **Do NOT parse justfiles yourself.** If you need recipe discovery, use `just --dump --dump-format json` and parse the JSON output.
- **Do NOT use `dialoguer`** over `inquire` — `inquire`'s MultiSelect has fuzzy search and a cleaner builder API.
- **Do NOT make this async.** It's a synchronous CLI tool. `tokio` would be pure waste here.
- **Do NOT use `just-mcp-lib`.** It has 725 total downloads and is v0.1.1 — too immature.

## Implementation Order

1. **Scaffold:** `cargo init runsteps`, add deps to `Cargo.toml`
2. **Config types:** `config.rs` with serde derives, write a test config, test deserialization
3. **Just check:** `ensure_just_available()` — detect `just` binary, offer to install via cargo
4. **Executor:** `executor.rs` — run a just recipe, run a raw command, handle exit codes
4. **Picker:** `picker.rs` — MultiSelect with Display impl, group filtering
5. **CLI:** `main.rs` — clap args, wire config → picker → executor
6. **Dependency validation:** warn if a selected step depends on an unselected step
7. **Dry run:** print commands without executing
8. **Polish:** colored output with `console`, error messages, `--list` subcommand

## Testing Strategy

- **Unit tests:** config deserialization with various TOML inputs (missing fields, defaults, validation)
- **Integration tests:** write a temp justfile + config, run executor, assert exit codes
- **No need to test `inquire` itself** — test the step filtering/ordering/dependency logic around it

## Cargo.toml

```toml
[package]
name = "runsteps"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow = "1"
clap = { version = "4.5", features = ["derive"] }
console = "0.15"
inquire = "0.9"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
```

## Optional Future Features

- **Auto-discover mode:** `runsteps --discover` parses `just --dump --dump-format json` and builds the picker from justfile recipes (no config needed)
- **Step arguments:** allow steps to accept runtime parameters (`just deploy --set env=prod`)
- **Profiles:** `[profiles.dev]` / `[profiles.prod]` to pre-select different step sets
- **Progress bar:** use `indicatif` for long-running steps
- **Parallel execution:** run independent steps concurrently (only if `depends_on` graph allows)

## References

- inquire docs: https://docs.rs/inquire/0.9.1
- inquire examples: https://github.com/mikaelmello/inquire/tree/main/inquire/examples
- just JSON dump: `just --dump --dump-format json` (see https://github.com/casey/just)
- clap derive docs: https://docs.rs/clap/4.5/clap/_derive/index.html
