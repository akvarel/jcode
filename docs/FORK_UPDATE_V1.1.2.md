# Fork update to v1.1.2

Date: 2026-11-23.

## Source and boundaries

- GitHub's latest-release API returned stable `v1.1.2`, published at `2026-11-23T09:10:12Z`. The release page independently identifies the same tag and commit `4f874c9ea6b7c3e228045fa8269e4d84bec66a6b`.
- Starting source and installed revision: `1513367cd`, on `update/jcode-v1.0.0-20261117`.
- Integrate the stable tag on `update/jcode-v1.1.2-20261123` with a merge preserving both ancestries. Do not mirror protected branches, force-push, publish releases, change credentials or deploy production services.
- Fetch preserved the existing historical `v0.36.0` tag rather than overwriting it. The new stable tags and upstream master fetched successfully.

## Integration decisions

- Adopt upstream's extracted authentication lifecycle tests and Antigravity streaming implementation. The conflicting fork provider changes only explicitly discarded channel-send results and do not represent separate provider behavior.
- Adopt upstream's device-login card, clipboard and QR behavior instead of the superseded plain-text progress implementation.
- Retain the durable orchestration watchdog, atomic status persistence, once-only completion delivery, external memory enrichment, OrcaRouter integration, deferred MCP discovery and pooled-client reconnect behavior.
- Register detached tasks with both the durable watchdog and upstream elapsed-review mechanism. Adopt upstream's string-based task timestamp suffix, avoiding numeric formatting ambiguity.
- Keep the release-publishing workflow removed. Preserve CI and upstream repository-gated development workflows.

## Inventory reconciliation

Run unchanged scanners against an independently extracted pristine archive of `v1.1.2`. Upstream test-size and panic budgets pass. The upstream code-size budget fails on `commands_cloud.rs` (1209 LOC), `remote/server_events.rs` (3494 versus 3490) and `ui_input.rs` (2266 versus 2236). Its swallowed-error budget fails because `commands_cloud.rs` adds one occurrence, increasing the upstream total from 3885 to 3886.

Reconcile the combined fork and upstream inventories with the existing scanner update commands, including decreases. Do not change scanner implementations or thresholds. These inventory reconciliations do not certify behavior. Use compilation, strict linting and regression suites for acceptance.

## Acceptance findings

- The full guardrail script passed, including all-target/all-feature compilation, strict Clippy, formatting, lockfile checks, inventories, dependency boundaries, wildcard re-export checks and onboarding invariants. Optional cargo-machete was not installed.
- Default-feature core test compilation exposed three newly imported account-failover tests referencing upstream's removed `RemoteQuotaCleanup` helper. Use the retained `TestRemoteUsageGuard::new()` in those tests, consistent with existing fork tests. This preserves scoped restoration of quota state rather than reintroducing cleanup that leaks daemon-only quota mode.
- The first executable app-core suite passed 2091 tests, ignored 26 and failed the new native-deferral integration test because it inherited the developer's disabled browser policy. Initialize that fixture through the existing base constructor with an explicit unrestricted tool policy, retaining every deferral assertion and leaving production configuration unchanged.
- Final whitespace review removed one upstream trailing blank line from the extracted legacy search fixture. No test assertions or production contracts were weakened.
- One app-suite rerun intermittently failed the socket callback-owner assertion. Retain the assertion and include the actual event in its diagnostic. The isolated socket workflow and subsequent complete app suite passed without a production routing change. This remains a known nondeterministic test observation, not a claimed production bug fix.

## Final acceptance before installation

- Final full `scripts/check_guardrails.sh`: passed. Optional cargo-machete was unavailable.
- Core library suites, serial execution with `umask 077`: app-core 2092 passed / 26 ignored, base 2387 passed / 42 ignored, config types 34 passed, swarm core 17 passed.
- TUI library: 3787 passed / 45 ignored. Antigravity runtime library: 37 passed. These exercise upstream UI and provider changes, including the adopted streaming implementation.
- ohAgent `cargo check --workspace --all-targets` and workspace formatting passed after synchronizing the repaired Jcode revision. Its lockfile adds the session-types crate's new libc dependency. Existing compiler warnings were not suppressed.
- Review source and staged whitespace, preserve the pre-existing parent `TEAM_MEMORY/SESSION_LOG.md` edit, install from a clean committed revision using `scripts/install_release.sh --fast`, and verify the immutable installed binary separately from the shared daemon.
- Live paid-provider calls and full parent E2E suites were not run. No production service was deployed or restarted.
