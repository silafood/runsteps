# runsteps

Interactive config-driven task runner for deployment and infrastructure workflows.

## What is runsteps?

`runsteps` reads a TOML config that defines named steps — each either a shell command or a `just` recipe — and presents an interactive multi-select picker so you choose which steps to run. It handles step ordering, dependency warnings, per-step confirmation prompts, and grouped output. Think of it as a scriptable, interactive wrapper around your justfiles and shell commands.

## Installation

From source:

```sh
cargo install --path .
```

From crates.io (future):

```sh
cargo install runsteps
```

## Quick Start

Create a `runsteps.toml` in your project:

```toml
[metadata]
name = "my-project"
description = "Deployment workflow"

[[steps]]
name = "build"
description = "Build the release binary"
command = "cargo build --release"
group = "ci"

[[steps]]
name = "deploy"
description = "Deploy to production"
command = "scp target/release/myapp user@server:/opt/myapp/"
group = "deploy"
confirm = true
depends_on = ["build"]
```

Then run:

```sh
runsteps
```

You will see an interactive multi-select picker. Choose steps with space, confirm with enter, and `runsteps` executes them in order.

## CLI Reference

| Flag | Short | Description |
|------|-------|-------------|
| `--config <PATH>` | `-c` | Path to config file (default: `runsteps.toml`) |
| `--all` | | Run all steps without the interactive picker |
| `--yes` | `-y` | Skip all confirmations, including per-step `confirm: true` |
| `--dry-run` | | Print what would execute without running anything |
| `--list` | | List available steps and exit |
| `--group <NAME>` | `-g` | Filter steps by group name |

### Flag combinations

| Flags | Picker | Global confirm | Per-step confirm | Execution |
|-------|--------|----------------|-----------------|-----------|
| (none) | interactive | yes | yes | yes |
| `--all` | skip (all selected) | skip | yes | yes |
| `--yes` | interactive | skip | skip | yes |
| `--all --yes` | skip | skip | skip | yes |
| `--dry-run` | interactive | no | no | print only |
| `--all --dry-run` | skip (all) | no | no | print only |
| `--list` | n/a | n/a | n/a | print + exit |

**CI usage:** `runsteps --all --yes` for zero-interaction execution.

## Configuration Reference

### `[metadata]`

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | yes | — | Project name shown in banner |
| `description` | string | no | — | Project description shown in banner |
| `justfile` | string | no | `./justfile` | Path to justfile used for `just_recipe` steps |
| `working_directory` | string | no | `.` | Working directory for all step commands |

### `[[steps]]`

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | yes | — | Unique step identifier |
| `description` | string | yes | — | Human-readable description shown in picker |
| `command` | string | no* | — | Shell command to run via `sh -c` |
| `just_recipe` | string | no* | — | `just` recipe name to invoke |
| `group` | string | no | — | Group name for filtering and display |
| `confirm` | bool | no | `false` | Prompt "Are you sure?" before running |
| `depends_on` | array of strings | no | `[]` | Step names that should run before this one |

*Exactly one of `command` or `just_recipe` is required per step.

### Full example

```toml
[metadata]
name = "my-infra"
description = "Infrastructure deployment steps"
justfile = "./justfile"
working_directory = "."

[[steps]]
name = "add-helm-repo"
description = "Add Helm chart repo and update"
command = "helm repo add bitnami https://charts.bitnami.com/bitnami && helm repo update"
group = "setup"

[[steps]]
name = "install-crds"
description = "Install Custom Resource Definitions"
just_recipe = "install-crds"
group = "setup"

[[steps]]
name = "deploy"
description = "Deploy via Helm with values"
just_recipe = "deploy"
group = "deploy"
confirm = true
depends_on = ["add-helm-repo"]
```

See [runsteps.toml](./runsteps.toml) at the repo root for a ready-to-edit example.

## Dependency Handling

When you select a step that has `depends_on` entries not in your current selection, `runsteps` warns you:

```
  Warning: the following dependencies are not selected: add-helm-repo
Include missing dependencies? [Y/n]
```

- Answer **Y** (default): missing dependencies are automatically added to the run in the correct order.
- Answer **N**: proceed without them (useful if you know they already ran).
- With `--yes`: missing dependencies are always auto-included without prompting.

Dependencies are never a hard error — `runsteps` always gives you the choice.

## Requirements

- **Rust 1.85+** (MSRV)
- **[just](https://github.com/casey/just)** — only required if any steps use `just_recipe`

Install `just`:

```sh
cargo install just
# or
brew install just
```

## Examples

See [docs/examples.md](./docs/examples.md) for complete example configs:

1. Kubernetes deployment pipeline
2. Rust project release workflow
3. Docker Compose service management
4. Shell-only data pipeline (no `just` required)

## License

MIT — see [LICENSE](./LICENSE)
