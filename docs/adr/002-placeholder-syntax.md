# ADR-002: Placeholder syntax for recipe arguments

- **Status**: Accepted
- **Date**: 2026-04-17
- **Implementation**: v0.3.0

## Context

runsteps v0.2.x steps can invoke a `just` recipe or a shell command but cannot pass runtime arguments to either. Real-world deployment workflows constantly need this: `just logs <pod>`, `just seal-secret <src> <dst>`, `kubectl rollout restart deployment/<name>`. The v0.3.0 roadmap adds arguments as an `args: Vec<String>` field on `[[steps]]` with `{{placeholder}}` interpolation.

This ADR locks the syntax and 7 decision points before implementation begins, so `args` becomes a stable public surface.

## Decision

- Placeholders use `{{name}}` syntax inside `args` element strings and inside `[env]` values.
- Resolution order per placeholder: (1) `--var name=value` CLI flag, (2) `prompts[name]` via `inquire::Text` interactive prompt, (3) hard error (no silent default).
- `prompts` is a sibling table on `[[steps]]`: `prompts = { pod_name = "Pod name to tail" }` — map of placeholder-name to human prompt text.
- `raw = true` per step opts out of shell-escaping (for `command` steps only; `just_recipe` steps pass argv directly to `just`).
- `--var` CLI flag is repeatable: `runsteps --var pod=foo --var env=staging`.
- Literal `{{` in an argv element is written as `{{{{`.

## Drivers

1. **Shell-injection safety** — prompted or CLI-supplied values must not enable command injection in `command` steps. Shell-escape via the `shell-escape` crate by default.
2. **User mental model** — `{{name}}` is familiar (Handlebars, Jinja, GitHub Actions expressions).
3. **TOML ergonomics** — keep `args` as a homogeneous `Vec<String>`, not a mix of strings and inline tables. Prompts live in a sibling map.
4. **Discoverability** — each placeholder's prompt lives in one place (`prompts` table), easy to read.

## Alternatives considered

### Alt 1 — Inline tables in args: `args = [{value="..."}, {prompt="...", name="pod"}]`

Rejected: TOML mixed-type arrays require everything to be the same type; forcing every element to be an object is verbose for the common static-arg case.

### Alt 2 — Positional `$1 $2` (shell-style)

Rejected: no named-argument clarity; `--var` CLI form becomes unreadable (`--arg 1=foo`).

### Alt 3 — Rust format-string `{name}` (single brace)

Rejected: collides with TOML's own `{}` inline-table syntax in some contexts, and single-brace syntax is visually noisy against TOML strings.

## Resolutions (the 7 questions)

### Q1. Multiple placeholders per argv element

**Decision**: SUPPORTED. `"--pod={{pod}}-{{env}}"` resolves to e.g. `--pod=web-staging`. Resolution is simple string replacement; no nesting.

### Q2. Literal `{{` escape

**Decision**: Doubled braces — `{{{{` resolves to literal `{{`. `}}}}` similarly for closing. Rare in practice.

### Q3. Value validation

**Decision**: Empty strings are allowed (valid argv element). Newlines (`\n` or `\r`) inside values are REJECTED with a hard error to avoid argv confusion and log-injection.

### Q4. CLI flag name

**Decision**: `--var key=value`. Rationale: `--arg` collides with clap's internal "Arg" concept and reads ambiguously. `--var` is distinct, short, and extensible. Repeatable via `clap::Arg::action(ArgAction::Append)`; `num_args = 1..`. No short form (reserved).

### Q5. `--yes` mode without value

**Decision**: HARD ERROR, exit code 2. Error text: `error: step '<step-name>' requires value for placeholder '{{<name>}}'; pass --var <name>=<value>`. Never silent empty-substitute.

### Q6. Orphan `prompts` entry

**Decision**: WARNING to stderr (non-fatal): `warning: prompts.<key> in step '<step>' is not referenced in any args element`. Tolerated during refactors; `RUNSTEPS_NO_WARNINGS=1` silences.

### Q7. Placeholders in `[env]` values

**Decision**: SUPPORTED. Resolution rules identical. NOT shell-escaped (env vars don't traverse the shell; the child process receives them as-is). Newline rejection still applies.

## Examples

### Static args (no placeholders)

```toml
[[steps]]
name = "seal-secret"
just_recipe = "seal-secret"
args = ["secrets/raw/db.env", "secrets/sealed/db.yaml"]
```

### Prompted args

```toml
[[steps]]
name = "logs"
just_recipe = "logs"
args = ["{{pod}}"]
prompts = { pod = "Pod name to tail" }
```

Interactive run: runsteps prompts "Pod name to tail: " and runs `just logs <entered-value>`.

CLI override: `runsteps --var pod=web-5f9d run`.

### Multiple placeholders per element

```toml
[[steps]]
name = "deploy"
command = "kubectl --context={{ctx}} rollout restart deployment/{{svc}}"
prompts = { ctx = "kubectl context", svc = "service name" }
```

### Raw opt-out (trust the value)

```toml
[[steps]]
name = "run-query"
command = "psql -c '{{sql}}'"
prompts = { sql = "SQL query" }
raw = true  # user is responsible; shell-escape disabled
```

### Placeholders in env

```toml
[[steps]]
name = "deploy-staging"
command = "helm upgrade --install app ./chart"
env = { KUBECONFIG = "~/.kube/{{env}}", HELM_NAMESPACE = "{{env}}" }
prompts = { env = "environment (staging/prod)" }
```

## Security

- `command` steps shell-escape every substituted value via the `shell-escape` crate.
- `raw = true` opts out and is a user-accepted risk, documented in SCHEMA.md and this ADR.
- Newlines in values are rejected to defend against argv confusion and log-injection.
- `just_recipe` steps pass resolved values as separate argv elements to `just`, bypassing the shell entirely.

## Consequences

- `args`, `prompts`, `raw` become part of the public TOML schema from v0.3.0 onward — backward-compatible changes only after that.
- The `--var` CLI flag is reserved in v0.3.0; conflicts with any future use of `--var` for other purposes.
- `[env]` table becomes a first-class feature in v0.3.0 with interpolation support.

## Follow-ups

- Document `--var` in `runsteps schema` output.
- Consider a future extension for required-vs-optional placeholders with defaults (`prompts = { name = { text = "...", default = "..." } }`) — not part of v0.3.0.

## References

- [ADR-001](001-standalone-task-runner.md) — the structural decision this ADR builds on.
- [SCHEMA.md](../../SCHEMA.md) — canonical schema reference; lists all placeholder rules.
- [CHANGELOG.md](../../CHANGELOG.md#030) — v0.3.0 release notes.
- `shell-escape` crate: https://crates.io/crates/shell-escape
