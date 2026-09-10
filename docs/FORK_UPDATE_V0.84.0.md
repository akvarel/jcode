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

Run against the rebased branch before installation:

- `scripts/check_guardrails.sh` for format, clippy, panic-budget and
  swallowed-error gates.
- `scripts/check_code_size_budget.py` and `scripts/check_test_size_budget.py`
  (`tracked=106` and `tracked=44`, no regressions).
- Workspace library tests for the crates touched by the imported fixes.
- `scripts/install_release.sh --fast` with refreshed Git metadata, followed by a
  version check on the installed launcher.

Update the ohAgent gitlink separately on `update/jcode-v0.84.0` and preserve
unrelated parent repository changes.
