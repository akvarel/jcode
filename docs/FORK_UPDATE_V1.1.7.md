# Fork update to v1.1.7

Date: 2026-11-28.

## Source and boundaries

- GitHub latest-release API identifies stable `v1.1.7`, published at `2026-11-28T07:11:46Z`, targeting `f04e8d7f480be22baa703440551dc5b79b1bd184`.
- Starting source and installed revision: `ae5926ca1`, on `update/jcode-v1.1.2-20261123`.
- Merge the stable release on `update/jcode-v1.1.7-20261128`, preserving fork and upstream ancestry. No protected-branch writes, force-pushes, release publication, production deployment or credential changes.
- Fetch retained the conflicting historical `v0.36.0` tag. New stable tags through `v1.1.7` and upstream master fetched successfully.

## Integration decisions

- Preserve atomic background-task status persistence and adopt upstream cancellation completion-bus publication. Exercise the upstream cancellation regression together with fork background ownership and watchdog tests.
- Adopt upstream hosted CI routing and module-resolution checks, retaining existing fork CI policy and the intentionally removed publishing workflows.
- Retain external memory enrichment, durable orchestration watchdog, OrcaRouter, deferred MCP discovery and pooled-client reconnect behavior.
- Apply rustfmt to imported upstream changes without changing contracts.

## Acceptance evidence

- Independently extracted pristine `v1.1.7` fails the unchanged code-size, test-size and swallowed-error inventory scanners. Reconcile the combined inventory with their existing `--update` commands, including decreases, without changing scanners or thresholds. The panic inventory improves upstream.
- Direct-release script tests: 5 passed. Hosted justrust wrapper tests: 4 passed.
- CI workflow contract tests report 8 assertion failures and 2 missing-publisher errors. An independently extracted pre-update `ae5926ca1` reproduces the same counts: these contracts conflict with existing fork branch triggers, advisory desktop timing policy and intentionally removed publishing workflows. Preserve that policy and report this limitation rather than reintroducing publication or weakening assertions.
- Strict Clippy exposed two upstream defects: a nested todo-work signature and the TUI launch function's eighth positional argument. Introduce a shared canonical-work alias and named `TuiLaunchOptions`, preserving serialization and all local/selfdev/SSH launch values without suppressing lints.
- Completed serial core suites: app-core 2100 passed / 26 ignored, base 2389 passed / 42 ignored, compaction-core 32 passed and todo-policy 117 passed. The aggregate invocation reached its 600-second deadline during TUI execution, so rerun TUI separately and rerun policy plus CLI regressions after the lint repairs.
- ohAgent `cargo check --locked --workspace --all-targets` passed with existing warnings. Workspace formatting fails in unrelated existing parent code, which remains untouched. Preserve all pre-existing AaaS and session-log edits and commit only the submodule pointer.
- The first complete TUI run passed 3795 tests and exposed two integration issues. Preserve the existing documented-stop exemption in the new delivery-poke path, consistent with this fork's established regression and the upstream configuration documentation. Reconcile imported policy assertions to that preserved fork contract, retaining the check that removing stopping evidence re-enables continuation. Update only the exact Escape notice assertion to upstream's intentional `Auto-poke paused` wording, retaining queue clearing and disabled-state assertions.
- Final full guardrails passed after the documented-stop repair, including strict Clippy and onboarding invariants. Optional cargo-machete was not installed. Complete reruns passed: TUI 3797 / 45 ignored, todo-policy 117, CLI 467. Debug CLI parser tests require `RUST_MIN_STACK=16777216`; the default test-thread stack overflowed in Clap parsing. One dependency-summary assertion failed intermittently, then passed in isolation and in the complete rerun. Retain both assertions and add the actual summary to failure diagnostics, with six corresponding test-inventory lines. No timing assertions were removed.
- The preliminary install build was cancelled before installation when TUI failures were reported. Install the final clean committed source with `JCODE_SKIP_SERVER_RELOAD=1 scripts/install_release.sh --fast`, verify its immutable revision and CLI, then activate through supported graceful server reload. Do not restart ohAgent production services.
