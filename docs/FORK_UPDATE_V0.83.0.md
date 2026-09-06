# Fork update to upstream v0.83.0

The update branch is `update/external-memory-enrichment-v0.83.0`, based on the
upstream release tag rather than the development branch. The previous v0.81.7
update branch remains available for rollback. No protected branch or published
history is replaced.

## Integration decisions

- Preserve external memory enrichment, coordinator-owned team memory, MCP pooled
  reconnect and bounded deferred discovery, OrcaRouter routing, and the durable
  orchestration watchdog.
- Keep upstream per-worker model selection and policy-registration ownership.
  Retain team-memory write denial when an Agent replaces a worker registration.
- Preserve untouched-panel suppression while allowing explicit empty handoff
  snapshots, canary state, memory injections, and recorded replay work to persist.
  Tests of explicitly saved sessions opt into saved state rather than requiring
  all empty panels to create transcripts.
- Keep upstream concurrency telemetry sanitization in the fork's extracted
  transport module, including test capture of delivery modes.
- Retain upstream image-navigation visibility, session-home test isolation, and
  auto-poke regression coverage when resolving historical fork conflicts.
- Keep upstream CI publishing workflows removed as required by fork policy.

## Validation integration

The upstream release introduces SSH and authentication flows and expands existing
runtime modules and regression suites. File-size, test-size, panic, and
swallowed-error baselines are refreshed for that imported release and the
reconciled fork. Thresholds and test assertions are not relaxed.

Integration fixes include an MCP dummy-handle timeout, a duplicate swarm model
field, bounded model documentation, an environment/render-lock ordering fix in
the smoothness test, wake-mode test isolation, and strict Rust lint fixes.
Environment-mutating async tests hold the environment lock outside their runtime
future. The WebSocket handshake fixture retains a scoped lint expectation for
Tungstenite's required large error-response callback type.

Run the full library acceptance suite with
`cargo test --workspace --lib -- --test-threads=1`. Parallel TUI execution can
still deadlock on process-global test state. Reconnect-only fixtures explicitly
clear their binary timestamp so a newly installed build does not redirect them
into client replacement while they are testing history and queued work.

Use `scripts/install_release.sh --fast` for installation with refreshed Git
metadata. Update the ohAgent gitlink separately and preserve unrelated parent
repository changes.
