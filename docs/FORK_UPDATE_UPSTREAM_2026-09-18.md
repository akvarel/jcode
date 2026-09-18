# Fork update to current upstream development (2026-09-18)

## Source and authorization

The user explicitly approved replacing fork master with current upstream/master
using force-with-lease. The previous master ce4e78956 is preserved on remote
`backup/fork-master-20260918`. The mirror now points to upstream `0c6056425`.
The installed starting revision was ee91cd65b on the v0.84.0 release line.
This update intentionally follows current development, not just the latest tag.

Custom integration branch: `update/external-memory-enrichment-upstream-20260918`.
Rebase replayed 53 of 55 commits. Two are now upstream implementations:
`29717b09d` (preserve development builds) and `5efb5d9b0` (observer completion).
The old custom branch remains intact. No force push of a custom branch was used.

## Integration review and fixes

An independent read-only reviewer checked the conflicting patch/TEAM_MEMORY,
base module declarations, API capability ledger and observer-history translation.
Upstream edit statistics and active-turn history behavior were retained alongside
fork memory, MCP, watchdog and TEAM_MEMORY safeguards.

Executable validation uncovered and corrected:
- A duplicate grok-build login alias introduced by overlapping changes.
- Five ModelRoute test fixtures missing the upstream usage field.
- Context-only debug sessions incorrectly skipped first persistence. Existing
  create/resume/clear E2E tests reproduced missing-file failures; explicit debug
  state now persists just like explicit canary/title state.
- New browser/Desktop descriptions exceeded existing prompt budgets. Guidance
  was split into bounded parameter descriptions, keeping caller authorization,
  untrusted page restrictions, scoped exact actions, secret exclusion and private
  Desktop/Xvfb constraints. Test limits and assertions were not weakened.
- Conifer exact route metadata was bypassed by newly broader shared family
  guesses. Both catalog and runtime fallback now respect unknown grok/nemotron
  route restrictions. Live/disk metadata still wins. Shared rules for all other
  providers remain unchanged. The first proposed broad shared-rule deletion was
  rejected during root review before acceptance.

## Executed validation before installation

- Workspace compilation succeeded after fixture correction.
- Full workspace test attempt: completed binaries reported 5020 passed, 7 failed,
  31 ignored before the TUI binary stopped making useful progress. The run was
  cancelled. This is not a full-workspace green claim.
- The seven failures were reproduced and fixed: two debug-session E2E, two tool
  description budgets and three Conifer assertions. Every one was rerun and passed.
- Harness API: 37 passed; API server/observer translation: 114 passed.
- TEAM_MEMORY: 10 passed; orchestration watchdog: 11 passed; CLI commands: 53 passed.
- Shared provider core: 130 passed; session persistence/filter: 76 passed.
- Final exact debug E2E: 1+1 passed; Conifer catalog: 2 passed; runtime fallback: 1
  passed; description budgets: 1+1 passed. Zero-match exploratory invocations are
  explicitly excluded from these counts.
- Formatting, conflict/whitespace diff check, module resolution, dependency
  boundaries and wildcard re-export checks passed.

Quality ratchets had stale pre-development baselines. The independent reviewer
executed the same checks on pristine upstream and reproduced all four failures:
upstream panic count139 and swallowed-error count3362 versus integrated119/3290.
The documented update procedures refreshed exact measured baselines for imported
upstream growth, then all four checkers passed. Their checks remain enabled with
no arbitrary headroom. Strict all-target clippy and a clean completed full TUI run
are not claimed. Upstream unused-function/profile warnings remain.

Detailed local command logs are retained in ~/.jcode/jcode-upstream-20260918-*.log.
The parent ohAgent update records final installed binary and daemon observations.
