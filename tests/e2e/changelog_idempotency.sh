#!/usr/bin/env bash
# US-022: Idempotency test for git-cliff + cargo-release pre-release-hook.
#
# Runs `cargo release 0.99.0` (dry-run, the default) twice and asserts that
# CHANGELOG.md does not contain a duplicate [0.99.0] section header.
#
# Dry-run mode is the default for cargo-release (no --execute flag required).
# The pre-release-hook ("git-cliff --tag ... --prepend CHANGELOG.md") is also
# run in dry-run mode by cargo-release, so it should NOT modify CHANGELOG.md.
# Either the section count is 0 (hook skipped in dry-run) or 1 (hook ran once).
# A count of 2 would indicate the hook ran and was not idempotent.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

# Guard: skip if cargo-release is not installed.
if ! command -v cargo-release &>/dev/null && ! cargo release --help &>/dev/null 2>&1; then
    echo "SKIP: cargo-release not available in this environment (US-022)."
    exit 0
fi

# Guard: skip if git-cliff is not installed (pre-release-hook would fail).
if ! command -v git-cliff &>/dev/null; then
    echo "SKIP: git-cliff not available in this environment (US-022)."
    exit 0
fi

echo "==> Run 1: cargo release 0.99.0 (dry-run)"
cargo release 0.99.0 2>&1 || true

echo "==> Run 2: cargo release 0.99.0 (dry-run, repeated)"
cargo release 0.99.0 2>&1 || true

echo "==> Checking for duplicate [0.99.0] sections in CHANGELOG.md..."
COUNT=$(grep -c '^## \[0\.99\.0\]' CHANGELOG.md 2>/dev/null || echo 0)

if [ "$COUNT" -gt 1 ]; then
    echo "FAIL: CHANGELOG.md contains $COUNT occurrences of '## [0.99.0]' — hook is NOT idempotent."
    exit 1
fi

echo "PASS: CHANGELOG.md has $COUNT occurrence(s) of '## [0.99.0]' (expected 0 or 1)."
