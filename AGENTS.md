# Repository Guidelines

## Repository Scope

- Jcode Desktop is in a separate repository.

## TUI style

- UI chrome must not wrap: startup headers, account rows, status labels, and
  other compact UI rows should fit their available terminal-cell width or
  truncate with an ellipsis. Count Unicode display cells, not characters.
  Preserve one logical row per visual row, including at narrow widths.
  Chat prose and other intentionally multiline content may wrap normally.
- Reuse the info widget's limit-bar renderer for usage indicators instead of
  introducing a different bar glyph, color, or used/remaining convention.

## Development Workflow

- **Use the user's Git identity** - Create commits with the configured
  `user.name` and `user.email`. Do not override them with `Jcode`, `Jcode agent`,
  or a fabricated agent email. Preserve existing contributor attribution when
  integrating work. If no identity is configured, ask rather than inventing one.
- **Welcome pull requests from everyone** - Review contributions on their merits,
  regardless of whether the author is a maintainer, an existing contributor, a
  first-time contributor, or an agent. Good PRs can be merged directly after review
  and validation. Do not require a maintainer-authored rewrite merely because of
  who submitted the change. See `CONTRIBUTING.md` for the contribution policy.
- **Prefer master/main** - Work directly on the repository's primary branch
  (`master` or `main`) by default. Do not create a branch or worktree merely to
  start a task. Only do so when the user explicitly requests it or isolation is
  genuinely necessary, and explain the concrete reason first.
- **Keep work scoped** - Preserve unrelated work. If already on a branch with
  ongoing changes, finish and integrate safely rather than switching blindly.
  When the user asks you to review or integrate a PR or branch, you may inspect,
  test, and integrate that contribution regardless of author status. Do not pull
  in unrelated branches or merge a PR without user authorization.

## Fork Strategy (akvarel/jcode)

This is a **fork** of [1jehuang/jcode](https://github.com/1jehuang/jcode) maintained as
`akvarel/jcode`. Integrate stable upstream releases on versioned `update/jcode-*`
branches using a merge that preserves fork and upstream ancestry. Never force-push
or write a protected branch as part of an ordinary update request.

### CI/CD

The release-publishing workflow remains removed. Retain existing CI and run
`scripts/check_guardrails.sh` locally before installing. This fork does not publish
releases to Homebrew, AUR, or GitHub Releases as part of local updates.

Build and install with `scripts/install_release.sh --fast` from a clean commit.
The legacy `~/.jcode/scripts/update-jcode-fork.sh` is not the update procedure:
it performs protected-branch/force-push operations and uses obsolete install paths.

### Upstream alignment (2026-09 review)

Prefer upstream implementations when they cover the same pain; do not carry
competing fork code:

- **Dropped** (superseded upstream): `Expose headless session API`,
  `Make state module public` — nothing consumed them, and programmatic use
  should go through `crates/jcode-sdk` (session launch, `wake_mode`,
  externally managed wake requests, session context forking).
- **Keep** (unique value, no upstream equivalent): external memory enrichment
  (graphify/vault/pgvector, `JCODE_MEMORY_VAULT_ROOT`) — `ohagent-memory`
  depends on `jcode-base` directly; durable orchestration watchdog;
  scalable deferred MCP discovery; OrcaRouter wiring (complements the
  upstream catalog profile).
- **Keep** (complementary healing): MCP pooled-client reconnect after a dead
  process — upstream prevents dead clients in the pool, ours reconnects the
  surviving ones.
- **Keep** (still required): `NotifySession` as lightweight control request —
  upstream's externally managed wake requests target SDK-launched sessions,
  not the ambient scheduler's internal `Client::connect()` path.
- Before adding a fork-only fix, check whether upstream already solved the
  same pain (grep `git log upstream/master`).

### Updating from upstream

Follow the integration evidence in `docs/FORK_UPDATE_V0.97.0.md` and subsequent
versioned update records. Verify the latest stable release, preserve custom
behavior with regressions, and update the ohAgent submodule on a task branch.
Use `JCODE_SKIP_SERVER_RELOAD=1` when installation must not interrupt active work,
then activate through the supported graceful reload path at a safe boundary.

### Submodule in ohAgent

This repo is a git submodule of `orangehat/ohAgent` at `ohAgent/jcode/`.
After updating the fork, update the submodule pointer:
```bash
cd /sharedssd/git/orangehat/ohAgent
git add jcode
git commit -m "chore: update jcode submodule to <hash>"
```

## Install Notes
- `~/.local/bin/jcode` is the launcher symlink used from `PATH`.
- `~/.jcode/builds/current/jcode` is the active local/source-build channel; self-dev builds and `scripts/install_release.sh` point the launcher here.
- `~/.jcode/builds/stable/jcode` is the stable release channel; `scripts/install.sh` installs this and points the launcher here.
- `~/.jcode/builds/versions/<version>/jcode` stores immutable binaries.
- `~/.jcode/builds/canary/jcode` still exists for canary/testing flows, but it is not the primary self-dev install path.
- On Windows, the equivalents are `%LOCALAPPDATA%\\jcode\\bin\\jcode.exe` for the launcher, `%LOCALAPPDATA%\\jcode\\builds\\stable\\jcode.exe` for stable, and `%LOCALAPPDATA%\\jcode\\builds\\versions\\<version>\\jcode.exe` for immutable installs; `scripts/install.ps1` currently installs the stable channel.
- Ensure `~/.local/bin` is **before** `~/.cargo/bin` in `PATH`.

## Verifying a change at runtime

`cargo build` alone proves nothing about behavior. `jcode run` and interactive
sessions are served by the long-lived daemon at
`~/.jcode/builds/shared-server/jcode`, which is a symlink into
`~/.jcode/builds/versions/<version>/`. Until that symlink is repointed and the
daemon restarted (`jcode self-dev --build`), a freshly built binary is inert and
every runtime check silently measures the old code.

To test a change without disturbing the shared daemon or the caller's session,
run your build against its own socket:

```bash
cargo build --profile selfdev
./target/selfdev/jcode run --no-update --socket /run/user/1000/jcode-mytest.sock '<prompt>'
```

Two things that waste time otherwise:

- `crate::logging::info` writes to a log file, not stderr, so instrumenting a
  code path with it produces no visible output under `--trace`. Use `eprintln!`
  for throwaway diagnostics and delete it before committing.
- Confirm which binary you are actually inspecting. `strings` on
  `builds/shared-server/jcode` reads a 70-byte symlink, not a program; resolve it
  with `readlink -f` first.
