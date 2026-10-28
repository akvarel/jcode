# Fork update to v0.99.1

Date: 2026-10-28.

## Integration decisions

- Stable upstream tag: `v0.99.1`, commit `7206efd0c`. The v1.0.0 previews are not the installation target.
- Starting fork: `6495c7ef3`, installed as v0.97.67-dev.
- Task branch: `update/jcode-v0.99.1-20261028`. Merge preserves both release and fork ancestry. No protected-branch writes or force pushes.
- Preserve external-memory enrichment, durable orchestration watchdog, ranked/paginated MCP discovery, discovery timeout, exact returned-page tool references, OrcaRouter configuration, lightweight NotifySession, and coordinator-owned team-memory safeguards.
- Adopt upstream atomic unified-patch preflight. Run the team-memory guard during preflight, before any file write. A regression verifies that a rejected memory update leaves an earlier ordinary file unchanged.
- Adopt upstream extraction of provider error classification, model metadata tests, and compatible-runtime initialization. Preserve compatible-profile clones and session binding.
- Adopt upstream removal of the redundant replace tool and task-graph/deep-swarm interface. The watchdog now uses the remaining plan status fields.
- Adopt upstream worker-effort policy: agent-facing swarm calls use the configured worker effort, defaulting to low. Direct SDK/protocol overrides remain supported. No user's configuration was changed.
- Adopt the durable telemetry usage outbox while preserving non-panicking HTTP-client and worker initialization. Existing opt-outs remain unchanged.
- Use the extracted upstream MCP test module, retain management/security coverage, and port fork-specific pagination/reference and lifecycle-timeout tests. Adapt consumers' tests to the paginated result envelope instead of an unpaginated array.
- Keep publishing workflow removed and existing CI enabled. Correct the obsolete local updater instructions in AGENTS.md.

## Acceptance findings and fixes

- The initial all-workspace/all-feature check encountered missing ALSA development metadata (`alsa.pc`) for optional audio features. The normal CLI all-target/all-feature check does not require it.
- The expanded app-core suite exposed stale test expectations for MCP pagination, provider-qualified model identity, and the intentionally open todo patch payload.
- Tool description limits remain unchanged. Shorten imported swarm/todo descriptions and the fork's bash description rather than increasing their budgets.
- Rust 1.96 Clippy reports nonminimal boolean conditions in imported code. Use equivalent expressions, not lint suppression.
- An isolated account-switch regression failed because this machine's umask is 0002 and its temporary auth home was group-writable. The new upstream auth-store safety check correctly rejected it. Create this test fixture with mode 0700 independently of umask. The real Jcode home was inspected as mode 0700, with no permission or credential changes.
- The old auth-inventory rendering helper no longer exists upstream. Keep the active-credential tag assertion and use the new account-router rendering tests instead of calling the removed helper.
- Update Bedrock test fixtures for the current built-in flagship: an older cached catalog must not downgrade it, while an explicit model pin still wins.
- Serialize the subprocess test's PATH access with the shared environment lock. Auth-store tests require a secure temporary home, so run acceptance with process-local `umask 077`, without changing real credential permissions.
- App-core library acceptance: 1679 passed, 17 ignored. Telemetry library acceptance: 94 passed with one test thread. Its parallel run exposed an inherited-file-description timing failure, so parallel telemetry acceptance is not claimed.
- Fix a real background-status race exposed by completion polling: direct writes truncated JSON while readers loaded it. Route initial, terminal, cancellation, and update writes through the existing atomic-rename storage helper, with logged write failures. Add a concurrent-reader regression.
- Optional all-workspace audio validation remains blocked by missing ALSA development metadata. The guardrail script skips cargo-machete because it is not installed.

## Static inventory reconciliation

A pristine `git archive v0.99.1` passed all four inventories: 120 oversized production files, 49 oversized test files, 174 panic-like occurrences, and 3961 swallowed-error-like occurrences. The merge combines existing fork differences with these upstream inventories, so individual per-file entries require reconciliation.

Remove duplicated inline MCP tests in favor of upstream's extracted module, retaining the unique fork regressions. Refresh inventories for the combined source, including decreases. Keep scanners and the 1200-line thresholds unchanged. The integrated panic inventory is 162, and the swallowed-error inventory is 3908. These counts do not prove correctness of individual paths.

## Installation boundaries

Install from a clean commit with `scripts/install_release.sh --fast`. Verify installed identity separately from the running shared daemon. Preserve active work until a supported graceful reload is safe. No live model request, provider configuration change, credential change, telemetry preference change, or ohAgent production restart is part of validation.

Update the ohAgent submodule and its tracking branch on a task branch, preserving the pre-existing TEAM_MEMORY/SESSION_LOG.md edit.

Final source acceptance on 2026-10-28: base library 1970 passed and 7 ignored, including atomic-status concurrency coverage. `scripts/check_guardrails.sh` passed formatting, CLI all-target/all-feature compilation, strict Clippy, lockfile validation, all inventories, dependency boundaries, re-export checks, and onboarding invariants. The optional cargo-machete step was skipped as noted above. Installation uses this integration commit's immutable version directory, with automatic shared-server reload disabled to preserve active sessions.
