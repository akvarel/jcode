# Jcode v0.89.3 fork integration

## Scope and ancestry

Integrate upstream release `v0.89.3` (`de65ade33`) into fork commit
`1f8b6ba0c` on `update/jcode-v0.89.3-20260930`. Preserve both histories with
a merge. Do not rewrite published branches or update protected master branches.
The newer upstream master commit only updates the weekly stars chart.

## Resolution decisions

- Keep external memory enrichment and apply upstream relevance prefiltering to
  the combined local and external candidates before fail-closed Jev selection.
- Keep ranked, paginated MCP search, collision-safe names, permission filtering,
  schema caps and the discovery timeout. Add upstream provider-native tool
  references from the returned page, capped at 32 references.
- Adopt upstream cache-stable auto/deferred MCP exposure. Update README and
  MCP discovery documentation to remove the obsolete token-threshold behavior.
- Adopt upstream Grok HTTP transport and bearer-token retry implementation in
  place of the obsolete ACP runtime and its fake-process fixture.
- Preserve the fork telemetry transport extraction while incorporating new
  usage-report APIs and per-event circuit-breaker exemption.
- Preserve orchestration watchdog, NotifySession, MCP reconnect, TEAM_MEMORY
  checks and desktop isolation guidance.
- Keep release and FreeBSD publishing workflows removed as required by fork
  policy. Preserve upstream test improvements and fork test-home isolation.

## Validation evidence

- Locked dependency fetch succeeded.
- `cargo check --locked --profile selfdev --workspace --all-targets`: passed.
- `cargo clippy --locked --profile selfdev --workspace --all-targets -- -D warnings`:
  passed after equivalent boolean simplifications and a needless-borrow fix.
- Workspace formatting and staged/unstaged whitespace checks passed.
- Configuration-type tests: 20 passed.
- Telemetry worker tests: 90 passed.
- Module declarations, dependency boundaries and wildcard re-export checks passed.
- Core regression command: `cargo test --locked --profile selfdev -p
  jcode-app-core -p jcode-base -p jcode-provider-grok-build-runtime -p
  jcode-telemetry-core --lib`. Application core: 1,460 passed, 13 ignored.
  Base: 1,701 passed, 5 ignored. Grok runtime: 5 passed. Telemetry: 66 passed.
- Final lint-fix revalidation: 60 Jev/transcript/redaction tests passed (1 ignored)
  and 48 MCP tests passed, including real stdio lifecycle and permission checks.
- TUI conflict-specific tests and onboarding invariants: 10 passed, 1 ignored.
  The new MCP pagination/native-reference regression passed in the core suite.
- Compilation caught missing telemetry usage APIs/helper import, an upstream
  MCP closed-state field missing in the fork test fixture, and mismatched TUI
  type aliases. These were corrected and their test suites rerun successfully.

## Existing guardrail limitations

Code-size, test-size, panic-usage and swallowed-error budget checks fail on the
merged tree. All four also fail against a separately extracted, untouched
`v0.89.3` archive. No baselines were reset and no checks were disabled. This
comparison is not a claim that every merged-tree difference is upstream debt.
The disposable upstream comparison fixture was removed after checking.

The compiler still prints Cargo profile warnings for removed dependencies
(`cosmic-text`, `swash`, `unicode-linebreak`, `yazi`). These are manifest-profile
warnings rather than Rust/Clippy lint failures. No warning-free build is claimed.

The initial build filesystem had only 1.7 GiB free. `cargo clean --profile dev`
removed regenerable debug artifacts and freed approximately 70 GiB while
preserving installed versioned binaries and release/selfdev caches.
