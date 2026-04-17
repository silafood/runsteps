# Changelog

All notable changes to `runsteps` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

<!--
  Future release sections (v0.1.6, v0.2.0, v0.3.0, v0.4.0) will be generated
  automatically from Conventional Commits by `git-cliff` during `cargo release`.
  The pre-release hook in Cargo.toml's [package.metadata.release] runs:
    git-cliff --tag {{version}} --unreleased --prepend CHANGELOG.md
  so each `cargo release X.Y.Z` prepends a new section above this comment.

  Do not hand-author future release sections here — they will duplicate with
  the generated ones. Add notes to commit messages instead (use `feat!:` or a
  `BREAKING CHANGE:` footer for breaking changes so they render correctly).
-->

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
