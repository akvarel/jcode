# Fork update to v1.0.0

Date: 2026-11-17.

## Source and boundaries

- GitHub's latest release API returned stable `v1.0.0`, published at `2026-11-17T04:01:22Z`. The annotated tag resolves to `32b28045a`.
- Starting source branch: `update/jcode-v0.99.1-20261028`, commit `712eb30ff`. Installed binary and ohAgent submodule started at `f3b027148`.
- Integration branch: `update/jcode-v1.0.0-20261117`. Merge retains upstream and fork ancestry. No protected-branch writes, force pushes, release publication, production operations, provider configuration changes, or credential changes.
- Fetch refused to overwrite an existing historical `v0.36.0` tag. Preserve that local tag. The new stable tag and upstream master fetched successfully, and the release API independently established the target.

## Integration decisions

- Adopt upstream module extractions instead of resurrecting inline helper implementations and duplicate tests.
- Preserve external memory enrichment and its opt-in defaults across the extracted configuration implementation, OrcaRouter wiring, deferred MCP discovery and pagination, pooled-client healing, lightweight NotifySession, durable orchestration watchdogs, and coordinator-owned team-memory validation.
- Keep upstream final-response and lane-progress swarm reporting. Retain coordinator memory-consolidation guidance without restoring the obsolete explicit-report injection function.
- Retain atomic background-status publication and once-only durable completion delivery. Combine them with upstream runtime activity scopes, serialized status updates, settled-task tracking, and the extracted stall watchdog.
- The installed November 12 opaque websocket rejection hotfix is superseded by upstream's native opaque recovery implementation and its fresh/persistent, bounded retry, post-acceptance exclusion, and cancellation regressions. Do not overlay the older transport implementation.
- Keep publishing workflows previously removed from the fork removed. Preserve CI, including the new TypeScript SDK checks. Imported preview workflows are upstream-repository gated.

## Inventory reconciliation

Run all four unchanged inventory scanners against a pristine archive of `v1.0.0` before reconciliation. All pass. Upstream's tracked baselines include slack: actual panic-like occurrences are 145 and swallowed-error-like occurrences are 3843.

Reconcile the combined source inventory, including decreases, without changing scanners or thresholds. The preserved fork files that exceed the upstream baseline (shell memory guard, MCP discovery, memory enrichment, authentication test coverage, and provider regression coverage) do not exceed their pre-update fork inventories. The initial integrated inventory contains 108 oversized production files, 47 oversized test files, 137 panic-like occurrences, and 3792 swallowed-error-like occurrences. These inventory counts do not certify behavior.

## Verification and installation

Core all-target compilation passed for `jcode-base`, `jcode-config-types`, and `jcode-swarm-core`. Formatting and module declaration checks passed. Further acceptance results are recorded below as they complete.

Install only from a clean integration commit using `scripts/install_release.sh --fast`. Verify the installed immutable binary separately from the running shared daemon. Use `JCODE_SKIP_SERVER_RELOAD=1` to preserve active sessions until a supported graceful activation boundary. Update the ohAgent submodule and tracking branch on a task branch while preserving its pre-existing `TEAM_MEMORY/SESSION_LOG.md` edit.
