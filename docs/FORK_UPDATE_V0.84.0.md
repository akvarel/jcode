# Fork update to upstream v0.84.0

The update branch is `update/external-memory-enrichment-v0.84.0`, based on the
upstream release tag rather than the development branch, matching the v0.83.0 and
earlier updates. The `update/external-memory-enrichment-v0.83.0` branch remains
available for rollback, together with
`backup/update-v0.83.0-pre-v0.84.0-20260910T134909Z`. No protected branch and no
published history of the fork was replaced except the `master` mirror, which is a
stale pure copy of upstream (it contained no fork-only commits).

## Base selection

The newest upstream release tag is `v0.84.0` (2026-09-06). Upstream `master` is
four days newer and holds nine commits after the tag: seven automated
star-history and SSH documentation updates plus two genuine fixes that the tag
does not contain. The branch therefore sits on `v0.84.0` and imports those two
fixes as explicit cherry-picks:

- `ce700ffcd` `fix(update): preserve development builds ahead of stable releases`
  adds the dev-build guard that stops a self-update from replacing a development
  build (our installed channel) with an older stable release.
- `b06ef5dab` `Preserve observer turn completion and reconnect activity in API bridge`
  keeps an observer's completed turn from being resurrected by a stale activity
  snapshot in the harness bridge.

Both keep their upstream authorship; the fork contributed no competing code in
those areas. A future rebase onto a release that contains them drops them again
through patch-identity detection.

## Integration decisions

- Preserve external memory enrichment, coordinator-owned team memory, MCP pooled
  reconnect and bounded deferred discovery, OrcaRouter routing, and the durable
  orchestration watchdog.
- `crates/jcode-harness-api-server/src/translate.rs` conflicted on one history
  snapshot region. The fork's `best_effort` helpers are kept for message and
  image parsing, while upstream's mutable `frames` vector is adopted because the
  following activity-snapshot logic pushes a status frame onto it.
- Keep upstream CI publishing and chart automation out of the fork: the
  `publish-typescript-sdk.yml` and `update-star-history.yml` workflows added by
  this release, and `.github/scripts/generate_star_history.py`, are removed. The
  fork publishes no npm package and keeps no generated star-history commits.
  Retained workflows remain `ci.yml` in its fork-compatible form,
  `discord-release.yml`, and `ios-testflight.yml`.
- File-size, test-size, panic, and swallowed-error baselines are refreshed for the
  imported release and the two cherry-picks. Thresholds and assertions are not
  relaxed.

## Validation

Executed on the rebased branch:

- `scripts/check_guardrails.sh` passes every gate: module declarations, `cargo fmt --check`,
  `cargo check --all-targets --all-features`, `cargo clippy -- -D warnings`, lockfile freshness,
  warning budget, oversized-file and oversized-test ratchets, panic-prone and swallowed-error
  ratchets, crate dependency boundaries, wildcard re-exports, and onboarding invariants.
- Ratchets after refresh: oversized files `tracked=106`, oversized test files `tracked=44`,
  panic-prone `total=87 files=29`, swallowed-error `total=3244 files=470`.
- Two `clippy::collapsible_if` sites that clippy 1.96 rejects under `-D warnings` are fixed:
  one in the imported `update.rs` guard path and one in upstream's remote-login clipboard paste.
- Focused tests: `cargo test -p jcode-app-core --lib -- update` (92 passed) and
  `cargo test -p jcode-harness-api-server --lib` (97 passed), covering the imported
  dev-build guard and the bridge history/activity frames.
- Full library suite: `cargo test --workspace --lib -- --test-threads=1`: 83 binaries,
  7382 passed, 0 failed, 33 ignored, exit 0. Parallel TUI execution is avoided because
  process-global test state can deadlock.
- `scripts/install_release.sh --fast` with `JCODE_BUILD_GIT_HASH=441f7fd5f` built and installed
  `jcode v0.84.54-dev (441f7fd5f)`, updating the `stable`, `current` and launcher symlinks and
  reloading the running server onto the new binary. The base version `0.84.0` and the commit
  hash both report correctly.

Update the ohAgent gitlink separately on `update/jcode-v0.84.0` and preserve unrelated parent
repository changes.

