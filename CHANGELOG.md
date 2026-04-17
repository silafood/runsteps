# Changelog

All notable changes to `runsteps` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - YYYY-MM-DD

### Added
- Profiles: `[profiles.<name>]` tables with `skip_confirms`, `excluded_steps`, `groups` fields. Invoke with `runsteps --profile <name>`.
- Parallel execution: `parallel = true` per step runs independent sibling steps concurrently with per-step output prefixes.

### Removed
- BREAKING: top-level flags `--list` and `--init` removed. Use `runsteps list` and `runsteps init` subcommands instead.

### Migration
- Scripts using `runsteps --list` must change to `runsteps list`.
- Scripts using `runsteps --init` must change to `runsteps init`.

## [0.3.0] - YYYY-MM-DD

### Added
- Recipe arguments: `args` and `prompts` tables in `[[steps]]` with `{{placeholder}}` interpolation. New `--var key=value` flag for CLI override (repeatable). See [ADR-002](docs/adr/002-placeholder-syntax.md).
- `raw = true` per step to opt out of shell-escaping for `command` steps.
- Per-step `[env]` table: step-scoped environment variables merged into the child process environment.
- `runsteps list --json`: versioned machine-readable step list (`{"version": 1, "steps": [...]}`).
- `runsteps graph`: ASCII DAG visualization of the `depends_on` graph with cycle detection. Exits nonzero with an explicit cycle path on detection.
- `runsteps completions <shell>`: bash, zsh, and fish shell completions via `clap_complete`.
- AI-friendly homepage: `/llms.txt`, `/llms-full.txt`, and JSON-LD `SoftwareApplication` metadata published at `runsteps.silafood.app`.
- README and SCHEMA.md: TOML preamble example using `#:schema https://runsteps.silafood.app/schema.json` for IDE LSP integration (`taplo`, `tombi`, `even-better-toml`).
- `git-cliff` integration for automated changelog generation on release via `cargo-release` pre-release hook.

### Deprecated
- Top-level `--list` and `--init` flags emit deprecation warnings to stderr. Use `runsteps list` and `runsteps init` subcommands instead. These flags will be removed in v0.4.0.

## [0.2.0] - YYYY-MM-DD

### Added
- `runsteps schema` subcommand: human-readable output by default; `--json` emits a JSON-Schema draft-07 document.
- `SCHEMA.md` canonical schema reference at repo root, linked from README and the homepage.
- `/schema.json` published at `https://runsteps.silafood.app/schema.json` at release time via CI.
- Clap subcommand structure: `run` (default), `schema`, `init`, `list`. Legacy top-level flags continue to work in v0.2.0.
- `--again` replay: re-runs the last interactive selection. History stored in `~/.cache/runsteps/history.json` (up to 10 entries per config path, keyed by config SHA-256). Config-hash mismatch triggers a warning.
- Levenshtein schema suggestions on unknown TOML keys: `[meta]` suggests `metadata`, `[[step]]` suggests `[[steps]]`, `just_recipee` suggests `just_recipe`, etc.
- Structured TOML error reporting: file path, line, column, and span included in error output.
- `RUNSTEPS_*` environment variable prefix convention established. Recognized variables: `RUNSTEPS_NO_WARNINGS`, `RUNSTEPS_DEBUG`.
- [ADR-001](docs/adr/001-standalone-task-runner.md): documents the decision that `runsteps` owns the dependency graph and treats `just` as a step executor, not a dep resolver.
- [ADR-002](docs/adr/002-placeholder-syntax.md): placeholder syntax design document (implementation ships in v0.3.0).
- [Migration guide](docs/migrations/0.2-no-deps.md) for the `just_no_deps` default flip, including a version-pin escape hatch (`cargo install runsteps --version "~0.1"`).

### Changed
- BREAKING: `just` recipes are no longer invoked with `--no-deps` by default. `runsteps` now delegates to `just`'s own prerequisite system unless you explicitly set `just_no_deps = true` on a step. To restore the old per-step isolation, add `just_no_deps = true` to the relevant step. See [docs/migrations/0.2-no-deps.md](docs/migrations/0.2-no-deps.md).
- `RUNSTEPS_*` prefix is now the convention for all environment variables read by `runsteps`. No unprefixed variables are added.
- Test suite split from a single `tests/integration_test.rs` into focused modules: `tests/init.rs`, `tests/list.rs`, `tests/executor.rs`, `tests/schema.rs`, `tests/again.rs`, `tests/errors.rs`, `tests/deps.rs`, with shared helpers in `tests/common/mod.rs`.

### Removed
- Transitional prereq-only warning introduced in v0.1.6 is removed; the default flip makes it obsolete. Refer to `SCHEMA.md` for how to restore the old isolation with `just_no_deps = true`.

## [0.1.6] - YYYY-MM-DD

### Added
- Warning at config load for `just_recipe` steps that point at prerequisite-only recipes (no body lines). The warning explains that the current `--no-deps` behavior causes a silent no-op and announces the v0.2.0 default flip.
- Version gate: the warning requires `just` >= 1.15.0 (needed for `--dump --dump-format json`). Gracefully skipped when `just` is below that version, with a debug-level log entry.
- `RUNSTEPS_NO_WARNINGS=1` environment variable to silence non-fatal warnings.

### Deprecated
- Implicit `--no-deps` on all `just_recipe` invocations is deprecated. The default flips in v0.2.0. Add `just_no_deps = true` to a step to preserve the current isolation behavior after upgrading.

## [0.1.5] - 2026-04-11

### Added
- Cached cargo registry in the release `build-local-artifacts` CI job.
- CI cache migrated to `silafood/rust-cache-s3` with graceful fallback on cache miss.

## [0.1.4] - 2026-04-10

### Added
- `cargo-release` configuration via `[package.metadata.release]` in `Cargo.toml` for single-crate tag-based releases.
- `cargo-dist` release infrastructure for multi-platform binary distribution and GitHub Releases automation.

## [0.1.3] - 2026-04-10

### Added
- Duplicate step execution prevention within a single `runsteps` invocation via `HashSet` tracking.
- Integration test suite (`tests/integration_test.rs`).

### Changed
- `just` recipes now invoked with `--no-deps` to prevent double execution when `runsteps` already handles step ordering via `depends_on`. Note: this introduces a silent no-op when a recipe has only prerequisites and no body — addressed by the warning in v0.1.6 and the default flip in v0.2.0.

## [0.1.1] - 2026-04-10

### Added
- `--version` flag.
- `--init` flag: generates a template `runsteps.toml` in the current directory.
- Auto-append of `.toml` extension when `--init` is called with a bare filename (without extension).

## [0.1.0] - 2026-04-09

### Added
- Initial release.
- TOML-driven interactive task runner wrapping `just` recipes and raw shell commands behind an `inquire` MultiSelect picker.
- Dependency validation and auto-include via `depends_on`.
- `--all`, `--yes`, `--dry-run`, `--list`, `--group`, and `--config` flags.
- Step grouping via the `group` field and per-step `confirm = true` for execution gates.
