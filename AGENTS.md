# Repository Guidelines

## Development Workflow

- **Stay on your own branch** - Do not take, cherry-pick, merge, or copy code from other
  people's or other agents' branches unless the source branch belongs to a repository
  maintainer and the user explicitly asks you to integrate it. Only work from your branch
  and its base (e.g. `main`) otherwise. Never integrate branches owned by non-maintainers
  or other agents yourself; tell the user and let them decide how to proceed.

## Fork Strategy (akvarel/jcode)

This is a **fork** of [1jehuang/jcode](https://github.com/1jehuang/jcode) maintained as
`akvarel/jcode`. Our fork carries custom commits on `master` (never rebased onto upstream
— updated via rebase of `feature/external-memory-enrichment`).

### CI/CD

**All upstream CI workflows are removed** (`ci.yml`, `release.yml`, `freebsd-smoke.yml`,
`windows-smoke.yml`, `require-issue.yml` and helper scripts) — see commit `1f93d52c`.

Reasoning:
- We do **not** publish releases to Homebrew, AUR, GitHub Releases, or any public channel.
- jcode is **built locally** via `~/.jcode/scripts/update-jcode-fork.sh` and used directly.
- The upstream CI matrix (10+ platforms, signing, packaging) is pure overhead for us.
- Quality guardrails (clippy, fmt, audits) are run **manually** via `scripts/check_guardrails.sh`.

Retained upstream workflows:
- `discord-release.yml` — Discord release announcements (still useful if we tag a release).
- `ios-testflight.yml` — iOS TestFlight build/upload (Apple signing pipeline, platform-specific).

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

See `~/.jcode/scripts/update-jcode-fork.sh` — fetches upstream, rebases the
current `update/external-memory-enrichment-v<X.Y.Z>` branch onto the latest
release tag (branch is versioned per upstream release), rebuilds binary.

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
