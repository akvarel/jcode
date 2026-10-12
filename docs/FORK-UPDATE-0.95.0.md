# Jcode v0.95.0 fork integration

## Scope and history

Integrate upstream release `v0.95.0` (`6487bbd7e9a45e6f91822292d6128854d5a6c5c6`)
into fork commit `5029aa542` on `update/jcode-v0.95.0-20261012`.
The merge preserves both histories and all 61 pre-update fork-only commits.
No protected branch is modified and no published history is rewritten.
The release tag was checked against the upstream remote's peeled tag hash.
An unrelated pre-existing `v0.36.0` tag disagreement was left untouched.

## Integration decisions

- Preserve external memory enrichment, vault configuration, paginated and
  schema-bounded MCP discovery, MCP reconnect, durable orchestration watchdog,
  NotifySession, TEAM_MEMORY protections, and OrcaRouter integration.
- Combine the external-memory configuration fields with upstream's new swarm
  stall settings rather than choosing one side of the configuration conflict.
- Adopt the upstream `jcode-todo-policy` extraction. Move the fork's two
  requirement-focused continuation messages into that new shared policy crate.
- Keep telemetry's non-panicking extracted transport. Port upstream background
  retry, in-flight accounting, bounded flushing, and desktop-event circuit-breaker
  exemptions into it. Unit-test delivery remains in memory and does not send
  test events to the live telemetry service. Add exemption and flush regressions.
- Preserve the synchronous environment-lock wrapper in the browser regression
  and add upstream's automatic-update suppression.
- Adopt terminal handoff guards and updated background-widget assertions.
- Preserve previously removed release and platform-publishing workflows.
  Retain upstream regression cohorts in the existing CI workflow.
- Reconcile quality manifests from upstream and existing fork allowances.
  Narrow merged-LOC adjustments account for retained integrations and added
  regression assertions. No test or lint gate was disabled.

## Test-discovered fixes

The first application-core suite found three failures, all investigated:

1. The moved todo-policy implementation had lost the fork's stronger guidance.
   Restore the original messages in their new canonical crate, retaining the
   existing assertions rather than weakening them.
2. The new post-tool feedback test set an environment override without
   invalidating the shared configuration cache. Invalidate it after setting and
   restoring the override, as neighboring hook fixtures already do.
3. The SDK socket fixture relied on an empty session being durable. Upstream's
   empty-root omission and reconnect grace made its disconnect assertion time
   out. Give the observer fixture a visible message and explicitly assert that
   resume succeeds and binds the intended session. Preserve the five-second
   pending-callback teardown assertion and all production reconnect behavior.

The additional CLI provider suite detected a duplicate OrcaRouter mapping: the
old fork entry now overlaps upstream's canonical compatible-provider entry.
Remove only the superseded entry, preserving the upstream route and its aliases.

## Executed validation

- Locked dependency fetch and workspace metadata checks passed.
- `cargo check --locked --profile selfdev --workspace --all-targets` passed.
- Strict workspace Clippy with `--all-targets -- -D warnings` passed.
- Application core: 1,556 passed, 15 ignored after the fixes.
- Base: 1,762 passed, 5 ignored.
- Configuration types: 20 passed.
- Telemetry core: 72 passed.
- Shared todo policy: 58 passed.
- Terminal UI: 2,545 passed, 18 ignored, using `COLORTERM=truecolor` and serial
  test execution without excluding tests.
- SDK: 70 passed, 3 ignored with serial test execution. The parallel attempt
  had one subprocess-start failure (`JcodeNotFound`) in an auth fixture;
  the full serial suite, including that test, passed without code changes.
- Telemetry worker: 91 passed.
- CLI provider initialization: 24 passed after deduplicating OrcaRouter.
- Formatting, staged and unstaged whitespace, module declarations, dependency
  boundaries, wildcard exports, code-size, test-size, panic, and swallowed-error
  budget checks passed.
- An independent untouched upstream archive also passed all four budget checks.
  The comparison fixture and worker scratch script were removed.
- A separate read-only review found no required telemetry integration changes.

## Limitations

Cargo emits existing unmatched profile-package warnings for removed desktop
packages (`cosmic-text`, `swash`, `unicode-linebreak`, `yazi`). These are not Rust
or Clippy lint failures. Ignored tests were not claimed as executed. No remote
build, release publication, credential change, or production deployment was used.
The installation and ohAgent gitlink are verified separately by the update task.
