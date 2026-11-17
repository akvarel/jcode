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

## Acceptance findings

- Nine upstream opaque websocket recovery regressions passed, covering both connection modes, bounded recovery, post-acceptance exclusions, and cancellation.
- The first app-core run passed 2026 tests with 23 ignored and failed one extracted schema test that still required pre-fork continuation wording. Align that assertion with the retained stronger requirement-inventory message, including its explicit prohibition on asking the user and its instruction to update the todo.
- Strict Clippy found nonminimal predicates in the preserved startup usage snapshot, harness interrupted-turn fence, todo lane validation, and four TUI display/recovery guards. Simplify each equivalently without suppressing lints.
- The initial combined guardrail run reached its 600-second deadline during onboarding validation after sharing the Cargo build lock with other acceptance jobs. The lint issue is fixed and the entire script is rerun, not bypassed.
- ohAgent's direct rusqlite 0.32 dependency conflicts with Jcode v1's rusqlite 0.41 native SQLite link. Align the parent workspace and its two independently pinned crates to the same dependency, explicitly retain fallible unsigned-integer conversion, regenerate its lockfile, and adapt message construction to Jcode's `SharedContent` through lossless `.into()` conversions.
- Full base-library validation exposed leaked daemon-only quota mode from test cleanup and generation inspection. Track activation explicitly, keep reconnect/empty/expired responses authoritative, and restore prior state with a test-only guard. Add a regression that reading the generation alone does not disable local cache access.
- Make the rotation fixture explicitly same-tier instead of reading real saved subscription tiers. Correct older-catalog Bedrock fixtures to use Opus 5 rather than Opus 6, which is newer than the built-in Opus 5.5. Preserve a separate regression that genuinely newer catalog routes may be promoted.
- A later app run encountered an unsafe writable account-store fixture directory under the inherited `0002` umask. Run final acceptance with `umask 077`, consistent with the earlier successful credential fixtures. Do not relax production ownership or permission checks.

## Verification and installation

Core all-target compilation passed for `jcode-base`, `jcode-config-types`, and `jcode-swarm-core`. Final acceptance passed:

- `scripts/check_guardrails.sh`: all available gates passed, including all-target/all-feature compilation, strict Clippy, formatting, lockfile consistency, all inventories, dependency boundaries, re-export checks, and onboarding state-space invariants. The optional cargo-machete gate was skipped because that tool is not installed.
- App-core library suite: 2027 passed, 23 ignored.
- Base library suite: 2315 passed, 27 ignored. Config types: 32 passed. Swarm core: 17 passed. Todo policy: 111 passed. These suites used `umask 077` and serial test execution.
- OpenAI opaque recovery: nine regressions passed.
- ohAgent: full workspace all-target compilation and formatting passed. Isolated SQLite consumer library suites passed with 19 tests, no failures.
- Fast release prebuild completed successfully. The installed binary is rebuilt from the clean commit with its exact Git identity.

Final review found no whitespace errors or unintended source changes. Preserve unchanged scanners and thresholds. Full parent E2E/runtime suites and live paid-provider tests were not run.

Install only from a clean integration commit using `scripts/install_release.sh --fast`. Verify the installed immutable binary separately from the running shared daemon. Use `JCODE_SKIP_SERVER_RELOAD=1` to preserve active sessions until a supported graceful activation boundary. Update the ohAgent submodule and tracking branch on a task branch while preserving its pre-existing `TEAM_MEMORY/SESSION_LOG.md` edit.
