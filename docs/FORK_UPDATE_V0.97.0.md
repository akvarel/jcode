# Fork update to v0.97.0

Date: 2026-10-14.

## Integration

- Upstream release tag: `v0.97.0`, commit `94392e100`.
- Starting installed fork: `b6672147a` (v0.95.63-dev).
- Branch: `update/jcode-v0.97.0-20261014`. Merge preserves both the starting
  fork and release ancestry. No master writes or force pushes are required.
- Upstream transport-aware MCP recovery supersedes the fork's process-only
  recovery. Keep its dead-handle regression, preserve session reference counts,
  and retain upstream's lost-notification regression.
- Use upstream pinned-todo rendering and its separate cache identity.
- Keep release-publishing workflow removed. Existing CI configuration is retained.
- Correct strict Clippy findings without suppression, box large pending reset
  state, and declare Tokio macros for production `select!` usage. Reset tests
  use synthetic state only; no account limits or credentials were changed.

## Validation before installation

`scripts/check_guardrails.sh` passed all gates: module resolution, formatting,
all-target/all-feature compilation, strict Clippy, lockfile freshness, warning
budget, size/panic/error inventories, dependency boundaries, wildcard exports
and onboarding state-space invariants.

Focused regressions passed, 246 tests total:

| Command suffix after `cargo test` | Passed |
| --- | ---: |
| `-p jcode-base --lib mcp::pool::tests -- --test-threads=1` | 5 |
| `-p jcode-tui --lib pinned_todo -- --test-threads=1` | 12 |
| `-p jcode-tui --lib usage_reset -- --test-threads=1` | 16 |
| `-p jcode-voice --lib global::hotkey -- --test-threads=1` | 16 |
| `-p jcode-telemetry-core --lib oauth_account -- --test-threads=1` | 10 |
| `-p jcode-base --lib provider::tests -- --test-threads=1` | 164 |
| `-p jcode-base --lib mcp::oauth -- --test-threads=1` | 20 |
| `-p jcode-tui --lib oauth_telemetry -- --test-threads=1` | 3 |

The complete workspace test suite was not run. Cargo reports upstream profile
entries for absent packages; these do not prevent strict Clippy from passing.

## Quality inventory reconciliation

A separate pristine `git archive v0.97.0` reproduced all four stale inventory
failures. Refresh inventories for the imported release and existing fork,
including decreases, without changing scanners or thresholds (1200 LOC).
Final counts: 124 oversized source files, 48 oversized test files, 168
panic-like occurrences and 3826 swallowed-error-like occurrences. Pristine
upstream has 179 and 3884 respectively. These are static counts, not evidence
that every error path is correct.

## Installation and privacy boundaries

Use `scripts/install_release.sh --fast` from a clean commit, then verify installed
version/hash and the running server separately. Source and submodule repositories
must track the same commit. Parent ohAgent changes belong on its task branch,
excluding the pre-existing `TEAM_MEMORY/SESSION_LOG.md` edit.

Upstream v0.97.0 expands telemetry to OAuth email identity. Existing environment
and `no_telemetry` opt-outs remain effective. No telemetry preference, credential,
provider configuration or account permission was changed by this update. The
upstream notice is acknowledged only after rendering; it is not an explicit
opt-in dialog. No live model request is part of validation.

Initial guardrail execution hit its 600-second deadline during cold selfdev
compilation. The warmed complete rerun passed. Old, reproducible selfdev
incremental caches were removed to recover disk space; source, installed versions
and unrelated user files were retained.
