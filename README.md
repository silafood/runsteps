# runsteps

Interactive config-driven task runner for deployment and infrastructure workflows.

`runsteps` reads a TOML config that defines named steps — each either a shell command or a [just](https://github.com/casey/just) recipe — and presents an interactive multi-select picker so you choose which steps to run. It handles step ordering, dependency validation, per-step confirmation prompts, and grouped output.

## For AI assistants

- **JSON Schema**: https://runsteps.silafood.app/schema.json (draft-07, paste as `#:schema` preamble into any `runsteps.toml` for IDE LSP support via taplo/tombi/even-better-toml).
- **Documentation bundle**: https://runsteps.silafood.app/llms.txt (structured TOC) and https://runsteps.silafood.app/llms-full.txt (full docs concatenated for LLM context).
- **Machine-readable changelog**: https://runsteps.silafood.app/changelog/ (generated from CHANGELOG.md).

## Install

### Shell (macOS/Linux)

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/silafood/runsteps/releases/latest/download/runsteps-installer.sh | sh
```

### Homebrew

```sh
brew install silafood/runsteps/runsteps
```

### Cargo (from source)

```sh
cargo install --git https://github.com/silafood/runsteps.git
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
name = "test"
description = "Run tests"
command = "cargo test"
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

Use space to select steps, enter to confirm, and `runsteps` executes them in order.

## CLI Usage

```
runsteps [OPTIONS]
```

| Flag | Short | Description |
|------|-------|-------------|
| `--config <PATH>` | `-c` | Path to config file (default: `runsteps.toml`) |
| `--all` | | Run all steps without the interactive picker |
| `--yes` | `-y` | Skip all confirmations |
| `--dry-run` | | Print what would execute without running anything |
| `--list` | | List available steps and exit |
| `--group <NAME>` | `-g` | Filter steps by group name |

### Common Patterns

```sh
# Interactive mode (default)
runsteps

# Run everything, no prompts (CI mode)
runsteps --all --yes

# Preview what would run
runsteps --dry-run

# Run only setup steps
runsteps --group setup

# Use a different config
runsteps --config deploy.toml
```

## Configuration

### `[metadata]`

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `name` | yes | — | Project name shown in banner |
| `description` | no | — | Project description |
| `justfile` | no | `./justfile` | Path to justfile for `just_recipe` steps |
| `working_directory` | no | `.` | Working directory for all commands |

### `[[steps]]`

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `name` | yes | — | Unique step identifier |
| `description` | yes | — | Human-readable description shown in picker |
| `command` | no* | — | Shell command to run via `sh -c` |
| `just_recipe` | no* | — | `just` recipe name to invoke |
| `group` | no | — | Group name for `--group` filtering |
| `confirm` | no | `false` | Ask "Are you sure?" before running |
| `depends_on` | no | `[]` | Steps that should run before this one |

*Each step requires exactly one of `command` or `just_recipe`.

### Full Example

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

## Dependencies

When you select a step that depends on unselected steps, `runsteps` warns you and offers to include them:

```
Warning: the following dependencies are not selected: add-helm-repo
Include missing dependencies? [Y/n]
```

- **Y** (default): missing dependencies are added in the correct order
- **N**: proceed without them
- With `--yes`: dependencies are auto-included without prompting

## Requirements

- [just](https://github.com/casey/just) — only needed if any steps use `just_recipe`

## More Examples

See [examples](https://github.com/silafood/runsteps/blob/master/docs/examples.md) for complete configs:

1. Kubernetes deployment pipeline
2. Rust project release workflow
3. Docker Compose service management
4. Shell-only data pipeline (no `just` required)

## License

MIT — see [LICENSE](https://github.com/silafood/runsteps/blob/master/LICENSE)
