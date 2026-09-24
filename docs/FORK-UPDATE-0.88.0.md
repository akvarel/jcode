# Jcode v0.88.0 fork integration

## Scope and ancestry

Merge upstream release `v0.88.0` (`ee4cd3db3`) into the custom fork at
`ec815264a1e12efa2d44aab38fa144a439fb6bc0`, on
`update/jcode-v0.88.0-20260924`. Preserve both histories, do not force-push.
The release is intentionally distinguished from newer upstream master.

## Resolution decisions

- Keep upstream Jev memory retrieval and selection. Feed fork external memory
  enrichment (Graphify, Vault, pgvector) into that same fail-closed selection
  pipeline rather than retaining the obsolete parallel sidecar path.
- Keep fork ranked, bounded, paginated MCP search. Compute upstream collision-safe
  dispatch names over the complete catalog before search/permission filtering.
  Preserve canonical and legacy permission checks.
- Port existing TEAM_MEMORY content/worker checks into upstream edit and replace
  tools. Keep foreground shell snapshot validation and rollback. Remove obsolete
  multiedit implementation in favor of upstream replacement tools.
- Keep dynamic fork account labels, watchdog/orchestration extensions, isolated
  desktop guidance and factored telemetry transport.
- Keep upstream lifecycle, persistence, SDK and UI interface changes where the
  prior fork implementation is superseded by the new interface.
- Remove duplicate auto-merged Orcarouter enum variant and dispatch arms. The
  first workspace check exposed this compiler error; the second passed.
- Preserve the fork's removed Windows smoke workflow. Existing unrelated CI is
  not removed or disabled.

## Observed validation so far

- `cargo check --workspace --all-targets --locked`: passed after duplicate fix.
- `cargo test --locked -p jcode-app-core --lib mcp`: 41 passed, including real
  stdio collision lifecycle and alias permission checks.
- `cargo test --locked -p jcode-app-core --lib team_memory`: 10 passed.
- Module-file resolution, dependency boundaries, wildcard re-export checks: passed.
- Formatting and whitespace checks passed at conflict closure.
- Memory regression: 112 passed, 1 ignored.
- Complete application-core regression: 1440 passed, 13 ignored. The first run
  exposed two wording assertions mismatched with retained upstream discovery and
  stronger fork requirement-level guidance. Assertions now check those actual
  contracts, including non-testable requirements; production behavior unchanged.
- Independent read-only review using `openai:gpt-6-sol` found no confirmed merge
  regression in memory, MCP, or guard call sites. It did not run cargo.
- Release build, installed runtime and Figma authentication: not yet verified.

## Known limitations, not waived checks

Code-size, test-size, panic-usage and swallowed-error ratchets fail on this tree.
All four also fail when executed against an independently extracted, untouched
`v0.88.0` archive. Their baselines were not reset and checks were not disabled.
This does not imply every merged-tree difference is upstream debt.
Compiler warnings include unused goal implementation, a test import and stale
Cargo profile package entries; a warning-free build is not claimed.

Independent review identified two preexisting TEAM_MEMORY limitations, confirmed
against the pre-merge fork: suffix validation falls back to whole-content checks
when old content is not a prefix, and the shell snapshot trigger is a command-text
heuristic that can miss computed paths. The retained guard is not a comprehensive
filesystem sandbox or a proof of append-only enforcement.

MCP unit and real-stdio tests are not proof of Figma OAuth success. The earlier
remote registration attempt received HTTP 403 before interactive login. Updating
Jcode does not by itself establish that the remote registration is now allowed.
