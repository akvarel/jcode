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
