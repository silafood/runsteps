# runsteps Config Schema Reference

## Overview

A `runsteps.toml` file describes the steps that `runsteps` presents to the user at
runtime. Every config file contains a `[metadata]` table (project-level settings), one
or more `[[steps]]` entries (the selectable units of work), and optionally one or more
`[profiles.<name>]` tables (pre-configured step sets). `runsteps` owns the dependency
graph: each step is exactly one unit of execution, and cross-step ordering is expressed
via `depends_on`, not via `just` prerequisites. For the rationale behind this model, see
[docs/adr/001-standalone-task-runner.md](docs/adr/001-standalone-task-runner.md).

## Schema URL

Place this comment at the top of any `runsteps.toml` to enable IDE completion via
[taplo](https://taplo.tamasfe.dev/), [tombi](https://tombi-toml.github.io/tombi/), or
the [Even Better TOML](https://marketplace.visualstudio.com/items?itemName=tamasfe.even-better-toml)
VS Code extension:

```toml
#:schema https://runsteps.silafood.app/schema.json

[metadata]
name = "my-project"
```

The schema JSON is regenerated at every release via `runsteps schema --json` and
published at the stable URL `https://runsteps.silafood.app/schema.json`.

## Top-level Structure

```toml
#:schema https://runsteps.silafood.app/schema.json

[metadata]
name          = "my-project"
description   = "Optional description shown in the header"
justfile      = "./justfile"
working_directory = "."

[[steps]]
name        = "step-a"
description = "First step"
command     = "echo hello"

[[steps]]
name        = "step-b"
description = "Second step"
just_recipe = "deploy"
depends_on  = ["step-a"]

[profiles.prod]
skip_confirms   = false
excluded_steps  = []
groups          = ["deploy"]
```

## `[metadata]` Fields

| Field | Type | Required | Default | Added in | Description |
|---|---|---|---|---|---|
| `name` | string | yes | — | v0.1.0 | Human-readable project name displayed in the picker header. |
| `description` | string | no | `null` | v0.1.0 | Optional one-line description shown below the project name. |
| `justfile` | string | no | `"./justfile"` | v0.1.0 | Path to the justfile used for `just_recipe` steps. Relative paths are resolved from `working_directory`. |
| `working_directory` | string | no | `"."` | v0.1.0 | Working directory for all step execution. Relative to the location of `runsteps.toml`. |

## `[[steps]]` Fields

Each `[[steps]]` entry must contain exactly one of `command` or `just_recipe`. Providing
both or neither is a hard error at config load time.

| Field | Type | Required | Default | Added in | Description |
|---|---|---|---|---|---|
| `name` | string | yes | — | v0.1.0 | Unique identifier for the step. Used in `depends_on` references and history replay. Must be unique within the config file. |
| `description` | string | yes | — | v0.1.0 | One-line description shown in the interactive picker. |
| `command` | string | no | `null` | v0.1.0 | Shell command executed via `sh -c`. Mutually exclusive with `just_recipe`. |
| `just_recipe` | string | no | `null` | v0.1.0 | Name of a `just` recipe to invoke. Mutually exclusive with `command`. |
| `just_no_deps` | bool | no | `false` | v0.2.0 | When `true`, passes `--no-deps` to `just`, skipping the recipe's prerequisites. Set this to restore v0.1.x isolation behavior for a specific step. Has no effect on `command` steps. |
| `group` | string | no | `null` | v0.1.0 | Logical grouping label. Used with `--group` filtering and profile `groups` selection. |
| `confirm` | bool | no | `false` | v0.1.0 | When `true`, prompts "Confirm: \<name\>?" before executing the step. The user can decline to skip the step without aborting the run. |
| `depends_on` | array of string | no | `[]` | v0.1.0 | Names of steps that must also be selected. `runsteps` warns at validation time if a dependency is missing from the selection. |
| `args` | array of string | no | `[]` | v0.3.0 | Extra arguments appended to the step invocation. May contain `{{placeholder}}` tokens resolved at execution time (see [Placeholders](#placeholders-name)). |
| `prompts` | table | no | `{}` | v0.3.0 | Map of placeholder name to prompt label. When a placeholder in `args` is not provided via `--var`, the user is prompted interactively using the label as the prompt text. |
| `raw` | bool | no | `false` | v0.3.0 | When `true`, substituted placeholder values in `command` steps are inserted verbatim (not shell-escaped). Has no effect on `just_recipe` steps. Use only when the command constructs the shell invocation itself. |
| `env` | table | no | `{}` | v0.3.0 | Map of environment variable name to value, set for this step's process only. Values may contain `{{placeholder}}` tokens. Does not affect the parent process environment. |
| `parallel` | bool | no | `false` | v0.4.0 | When `true`, this step may execute concurrently with other `parallel = true` steps that have no dependency relationship. Steps with `depends_on` links are still sequenced correctly. |

## `[profiles.<name>]` Fields

Profiles are defined in v0.4.0. Each profile is a named preset that adjusts which steps
are run and how confirmations behave. Activate a profile with `runsteps --profile <name>`.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `skip_confirms` | bool | no | `false` | When `true`, all `confirm = true` steps skip the per-step confirmation prompt within this profile. Equivalent to passing `--yes` for the affected steps only. |
| `excluded_steps` | array of string | no | `[]` | Step names to exclude from selection when this profile is active. Excluded steps cannot be manually selected even if the user edits the multi-select. |
| `groups` | array of string | no | `[]` | When non-empty, restricts the picker to steps whose `group` matches one of the listed values. Equivalent to `--group` but persisted in the config. |

Example:

```toml
[profiles.prod]
skip_confirms  = false
excluded_steps = ["seed-db"]
groups         = ["build", "deploy"]

[profiles.ci]
skip_confirms  = true
groups         = ["build", "test"]
```

## Environment Variables

Every environment variable that `runsteps` reads directly must start with the
`RUNSTEPS_` prefix. `runsteps` does not read unprefixed environment variables such as
`DEBUG` or `NO_COLOR` through its own code. `NO_COLOR` is honored automatically by the
[`console`](https://docs.rs/console) crate without any `runsteps`-specific handling.

| Variable | Description |
|---|---|
| `RUNSTEPS_NO_WARNINGS=1` | Silences non-fatal runtime warnings (e.g., the prereq-only step warning). Fatal errors are not suppressed. |
| `RUNSTEPS_DEBUG=1` | Enables verbose internal logging. The output format is unstable and subject to change between releases without notice. |

Any future environment variable recognized by `runsteps` requires a SCHEMA.md entry in
the same pull request that introduces it.

## Placeholders (`{{name}}`)

Added in v0.3.0. Implemented per
[docs/adr/002-placeholder-syntax.md](docs/adr/002-placeholder-syntax.md).

Placeholders are tokens in the form `{{name}}` embedded in `args` values or `env`
values. They are resolved at execution time before the step is spawned.

### Resolution Order

1. `--var name=value` CLI flag (highest priority).
2. `prompts[name]` — an interactive [inquire](https://docs.rs/inquire) Text prompt is
   shown to the user using the value from `prompts` as the prompt label.
3. Hard error — exits nonzero. A placeholder with no resolution path is always an error.

### Rules

- **Multiple placeholders per element** are supported.
  `"--pod={{pod}}-{{env}}"` resolves both tokens in the same string.
- **Literal `{{`** is escaped as `{{{{`. The output is `{{` verbatim.
- **Shell escaping** — for `command` steps, substituted values are shell-escaped via the
  [`shell-escape`](https://crates.io/crates/shell-escape) crate before being inserted.
  This prevents injection when values contain spaces or shell metacharacters.
- **`raw = true`** disables shell escaping for a step. Use only when the command itself
  handles quoting or constructs the invocation programmatically.
- **`--yes` mode** — if `--yes` is passed and a placeholder has no `--var` value and no
  `prompts` entry, `runsteps` exits nonzero with an explicit error message. It does not
  fall back to an empty string.
- **Orphan prompt** — a `prompts` entry whose key does not appear in any `args` element
  emits a warning at config load time but does not abort execution.
- **`env` values** — placeholders in `[env]` values follow the same resolution order and
  escaping rules as `args`.

### Example

```toml
[[steps]]
name        = "seal-secret"
description = "Seal a Kubernetes secret with kubeseal"
command     = "kubeseal --namespace {{namespace}} --name {{secret_name}}"
group       = "secrets"
args        = ["--namespace", "{{namespace}}", "--name", "{{secret_name}}"]
prompts     = { namespace = "Kubernetes namespace", secret_name = "Secret name" }
```

Run with explicit values:

```
runsteps --var namespace=production --var secret_name=db-password
```

Run interactively (both values are prompted):

```
runsteps
```

## Examples

### Kubernetes Deployment with `confirm` and `depends_on`

```toml
#:schema https://runsteps.silafood.app/schema.json

[metadata]
name             = "k8s-deploy"
description      = "Kubernetes deployment workflow"
justfile         = "./justfile"
working_directory = "."

[[steps]]
name        = "add-helm-repo"
description = "Add Helm chart repo and update index"
command     = "helm repo add bitnami https://charts.bitnami.com/bitnami && helm repo update"
group       = "setup"

[[steps]]
name        = "install-crds"
description = "Install Custom Resource Definitions"
just_recipe = "install-crds"
group       = "setup"
depends_on  = ["add-helm-repo"]

[[steps]]
name        = "deploy"
description = "Deploy application via Helm"
just_recipe = "deploy"
group       = "deploy"
confirm     = true
depends_on  = ["install-crds"]
```

### Parameterized Secret Sealing with `args` and `prompts`

```toml
#:schema https://runsteps.silafood.app/schema.json

[metadata]
name = "secrets-workflow"

[[steps]]
name        = "seal-secret"
description = "Seal a Kubernetes secret with kubeseal"
just_recipe = "seal"
group       = "secrets"
args        = ["{{namespace}}", "{{secret_name}}"]
prompts     = { namespace = "Target namespace", secret_name = "Secret resource name" }

[[steps]]
name        = "apply-secret"
description = "Apply the sealed secret to the cluster"
command     = "kubectl apply -f sealed-secret.yaml"
group       = "secrets"
confirm     = true
depends_on  = ["seal-secret"]
env         = { KUBECONFIG = "/etc/kubeconfig/{{cluster}}.yaml" }
prompts     = { cluster = "Cluster name (matches kubeconfig filename)" }
```

### Parallel Independent Steps

```toml
#:schema https://runsteps.silafood.app/schema.json

[metadata]
name = "ci-pipeline"

[[steps]]
name        = "lint"
description = "Run clippy lints"
command     = "cargo clippy -- -D warnings"
group       = "check"
parallel    = true

[[steps]]
name        = "test"
description = "Run unit and integration tests"
command     = "cargo test"
group       = "check"
parallel    = true

[[steps]]
name        = "audit"
description = "Run cargo audit for known vulnerabilities"
command     = "cargo audit"
group       = "check"
parallel    = true

[[steps]]
name        = "build-release"
description = "Build release binary after checks pass"
command     = "cargo build --release"
group       = "build"
depends_on  = ["lint", "test", "audit"]
```

## Versioning Policy

`runsteps` follows [Semantic Versioning](https://semver.org/) while pre-1.0. Breaking
schema changes land at **minor** version boundaries (e.g., v0.2.0, v0.3.0). Each
breaking change is announced with a deprecation warning in the **patch release
immediately before** the minor that removes or alters the behavior, giving users one
release cycle to update their configs.

The v0.2.0 default flip of `just_no_deps` is an example: the prereq-only warning was
shipped in v0.1.6 (patch), and the behavioral change landed in v0.2.0 (minor).

See [CHANGELOG.md](CHANGELOG.md) for the full history of schema additions by version.

## References

- [docs/adr/001-standalone-task-runner.md](docs/adr/001-standalone-task-runner.md) — ADR-001: runsteps is a standalone task runner with `just` as a step executor.
- [docs/adr/002-placeholder-syntax.md](docs/adr/002-placeholder-syntax.md) — ADR-002: Placeholder syntax design (`{{name}}`, `--var`, `prompts`, shell-escape rules).
- [docs/migrations/0.2-no-deps.md](docs/migrations/0.2-no-deps.md) — Migration guide for the v0.2.0 `just_no_deps` default flip.
- [CHANGELOG.md](CHANGELOG.md) — Full release history including schema additions per version.
