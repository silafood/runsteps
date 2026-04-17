# ADR-001: runsteps is a standalone task runner with `just` as a step executor

- **Status**: Accepted
- **Date**: 2026-04-17
- **Context version**: v0.2.0

## Context

In v0.1.x, `runsteps` straddled two distinct mental models: "interactive frontend for `just`" and "standalone task runner that can invoke `just` recipes." The most visible symptom of this ambiguity was `src/executor.rs:24`, which passed `--no-deps` **unconditionally** to every `just` invocation — a design that made sense if `runsteps` owned the dependency graph (so `just` should skip its own), but was never documented as such. The result was a silent no-op footgun: steps whose `just` recipe body consisted entirely of prerequisites would succeed with exit code 0 while doing nothing. Before shipping v0.2+ features (parallelism, `env` scoping, parameterized args), the project needs a clear, stated mental model so that every subsequent design decision has a foundation to build on.

## Decision

`runsteps` owns the dependency graph. Each `[[steps]]` entry maps to **exactly one** unit of execution — either one shell command or one `just` recipe invocation. Cross-step dependencies are expressed via `runsteps.depends_on`, not `just`'s prereqs. `just` is treated as a reusable recipe executor, not a dep resolver.

## Drivers

1. The tool already behaves this way internally (HashSet dedupe in `src/main.rs:170`, explicit validator, ordering). The bug in v0.1.5 is that the design was never made explicit.
2. Mixing `just_recipe` and `command` steps with cross-type dependencies requires `runsteps` to own the graph. `just` cannot express a prereq on a raw shell step.
3. Future features (parallelism, env scoping, args) need a single graph model to be coherent.

## Alternatives considered

### Alt 1 — Frontend-for-`just`

`runsteps` is a thin interactive picker over `just --list`, with `depends_on` only as a cross-type bridge. Rejected: loses the ability to mix step types cleanly; forces users into a justfile for shell-only workflows; conflicts with the existing HashSet dedupe model.

### Alt 2 — Dual-mode per-config

`[metadata].graph_owner = "just"|"runsteps"`. Rejected: doubles the test matrix; every future feature must handle both semantics; `--no-deps` bug becomes existential rather than fixable.

## Why chosen

- Minimal code change (existing code already behaves this way).
- Fixes #2 at the design layer, not just the code layer.
- Unblocks #3, #9, #10.
- Preserves `just` interop via `just_no_deps = true` for users who want isolation.

## Counterexample for first-time readers

A user with a justfile like:

```just
deploy-all: setup deploy-infra deploy-services
```

who wants `runsteps` to run all three in sequence should express this as **three `[[steps]]`** in `runsteps.toml`, not as one step pointing at `deploy-all`:

```toml
[[steps]]
name = "setup"
just_recipe = "setup"

[[steps]]
name = "deploy-infra"
just_recipe = "deploy-infra"
depends_on = ["setup"]

[[steps]]
name = "deploy-services"
just_recipe = "deploy-services"
depends_on = ["deploy-infra"]
```

This gives `runsteps` visibility into each step for the picker, dry-run, and future parallelism. Users who insist on the one-step shape must set `just_no_deps = false` (the new default in 0.2.0) to get `just`'s prereq execution.

## Consequences

- `just_no_deps = false` is the new default in v0.2.0. Users relying on the old behavior opt in per-step.
- Documentation must state the model loudly (`SCHEMA.md` top section + README + counterexample above).
- Future versions cannot quietly resurrect `just`-as-resolver without revisiting this ADR.

## Follow-ups

- Update `CLAUDE.md` with the model.
- Reference ADR-001 in every `--no-deps` or `depends_on` diagnostic.
- Consider deprecating the `justfile` Metadata field in v1.0 if it proves redundant (most users can rely on `just`'s default discovery).

## References

- [docs/migrations/0.2-no-deps.md](../migrations/0.2-no-deps.md) — user-facing migration for the `--no-deps` default flip.
- [docs/adr/002-placeholder-syntax.md](002-placeholder-syntax.md) — related decision on `{{placeholder}}` args syntax.
- [SCHEMA.md](../../SCHEMA.md) — canonical schema reference.
- [CHANGELOG.md](../../CHANGELOG.md#020) — v0.2.0 release notes where this ADR lands.
