<div align="center">

<p>
<a href="https://trendshift.io/repositories/25042?utm_source=repository-badge&amp;utm_medium=badge&amp;utm_campaign=badge-repository-25042" target="_blank" rel="noopener noreferrer"><img src="https://trendshift.io/api/badge/repositories/25042" alt="1jehuang/jcode | Trendshift" width="250" height="55"></a>
</p>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/brand/jcode-mark-inverse.svg">
  <img src="assets/brand/jcode-mark.svg" alt="jcode halftone donut logo" width="160">
</picture>

# jcode

<p>
  <strong>High-performance coding agents.</strong><br>
  Run dozens of agents in parallel without melting your machine. More agents, less RAM, no lag.
</p>

[![Last Commit](https://img.shields.io/github/last-commit/1jehuang/jcode/master?style=flat-square&logo=github)](https://github.com/1jehuang/jcode/commits/master)
[![GitHub Stars](https://img.shields.io/github/stars/1jehuang/jcode?style=flat-square&logo=github)](https://github.com/1jehuang/jcode/stargazers)
[![GitHub Downloads](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2F1jehuang%2Fjcode%2Fmaster%2Fdocs%2Fbadges%2Fdownloads.json&style=flat-square&logo=github)](https://github.com/1jehuang/jcode/releases)

[![Discord](https://img.shields.io/badge/Discord-Join%20Community-5865F2?style=flat-square&logo=discord&logoColor=white)](https://discord.gg/nBe9vGyK9a)
[![Y Combinator](https://img.shields.io/badge/Y%20Combinator-S26-F26625?style=flat-square&logo=ycombinator&logoColor=white)](https://www.ycombinator.com/companies/jcode)
[![X (Twitter)](https://img.shields.io/badge/follow-%401jehuang-000000?style=flat-square&logo=x&logoColor=white)](https://x.com/1jehuang)

</div>

<div align="center">

  <a href="https://github.com/1jehuang/jcode/releases/download/readme-assets/jcode-performance-demo-2026-10.mp4">
    <img src="https://github.com/1jehuang/jcode/releases/download/readme-assets/jcode-performance-demo-2026-10-30fps.webp" alt="jcode performance demonstration" width="900">
  </a>

  <p><em>jcode performance demonstration (1x speed)</em></p>

<a href="https://github.com/1jehuang/jcode/releases/download/readme-assets/jcode-yc-launch.mp4">
  <img src="https://github.com/1jehuang/jcode/releases/download/readme-assets/jcode-yc-launch.webp" alt="jcode YC launch video" width="800">
</a>

</div>

<div align="center">

## Installation

</div>

```bash
# macOS & Linux
curl -fsSL https://jcode.sh/install | bash
```

<div align="center">

## Benches

</div>

### RAM per additional session (headless)

| Tool | First session | Each additional session | Comparison (additional) |
|---|---|---|---|
| **jcode** | **71.3 MB** | **~3.3 MB** | baseline |
| **OpenCode** | 637.1 MB | ~5.9 MB | **1.8× more RAM** |
| **pi** | 84.0 MB | ~79.6 MB | **24.1× more RAM** |
| **Claude Code** | 272.9 MB | ~169.3 MB | **51.3× more RAM** |
| **Codex CLI** | 398.6 MB | ~194.1 MB | **58.8× more RAM** |

[Reproduce](scripts/bench_headless_memory.py).

### Time to first input

Time until typed text appears on screen, across 10 interactive launches.

| Tool | Time to first input | Comparison |
|---|---|---|
| **jcode** | **19.1 ms** | baseline |
| **pi** | 596.4 ms | **31.2× slower** |
| **Codex CLI** | 905.8 ms | **47.4× slower** |
| **OpenCode** | 1047.9 ms | **54.9× slower** |
| **Claude Code** | 3512.8 ms | **183.9× slower** |

Full numbers on [jcode.sh](https://jcode.sh).

<div align="center">

## Screenshots

[Browse the interactive gallery on jcode.sh](https://jcode.sh/#screenshots).

<table>
  <tr>
    <td align="center"><a href="https://jcode.sh/screenshots/inline-image-rendering.webp"><img src="https://jcode.sh/screenshots/thumbs/inline-image-rendering.webp" alt="Jcode rendering screenshots inline after reading image files" width="440" /></a><br>Inline images</td>
    <td align="center"><a href="https://jcode.sh/screenshots/resume-menu.webp"><img src="https://jcode.sh/screenshots/thumbs/resume-menu.webp" alt="Jcode resume menu with a session list and preview" width="440" /></a><br>Resume menu</td>
  </tr>
  <tr>
    <td align="center"><a href="https://jcode.sh/screenshots/latex-math-rendering.webp"><img src="https://jcode.sh/screenshots/thumbs/latex-math-rendering.webp" alt="Jcode rendering LaTeX mathematics inline" width="440" /></a><br>Inline LaTeX</td>
    <td align="center"><a href="https://jcode.sh/screenshots/cache-cold-notification.webp"><img src="https://jcode.sh/screenshots/thumbs/cache-cold-notification.webp" alt="Jcode warning that the prompt cache went cold" width="440" /></a><br>Cache cold notification</td>
  </tr>
</table>

</div>

## Swarm

Spawn two or more agents in the same repo, and they will automatically be managed by the server to allow native collaboration. When agent A edits a file that agent B has read (code shifting under its feet), the server notifies agent B. Agent B can ignore it if it is not relevant, or it can check the diff to make sure that it doesn't conflict. Each agent has messaging abilities, capable of DMing just one agent, broadcasting to all other agents hosted by the server, or just agents working in that repo. This allows you to spawn multiple sessions in the same repo, and have all conflicts automatically resolved.

Agents are also able to spawn their own swarms autonomously. They have a swarm tool which allows them to spawn in their own teamates to accomplish tasks in parallel. Doing so turns the main agent into a coordinator and the spawned agents into workers. Groups of agents, their messaging channels, their completion statuses, etc are all automatically managed. This can be done headlessly or headed.


## OAuth and Providers

jcode works with subscription-backed OAuth flows and many provider integrations, so you can use the models you already pay for and still fall back to direct API providers when needed.

### Supported built-in login flows

- **Claude** (`jcode login --provider claude`)
- **OpenAI / ChatGPT / Codex** (`jcode login --provider openai`)
- **Google Gemini** (`jcode login --provider gemini`)
- **GitHub Copilot** (`jcode login --provider copilot`)
- **Azure OpenAI** (`jcode login --provider azure`)
- **Alibaba Cloud Coding Plan** (`jcode login --provider alibaba-coding-plan`)
- **Fireworks** (`jcode login --provider fireworks`)
- **Novita AI** (`jcode login --provider novita`, API key)
- **MiniMax** (`jcode login --provider minimax`)
- **Meta Model API / Muse** (`jcode login --provider meta-muse`)
- **LM Studio** (`jcode login --provider lmstudio`)
- **Ollama** (`jcode login --provider ollama`)
- **Custom OpenAI-compatible endpoint** (`jcode login --provider openai-compatible`)

For custom OpenAI-compatible endpoints, jcode now prompts for the API base and supports local localhost servers without requiring an API key.

Azure OpenAI accepts a resource URL or a base URL ending in `/openai/v1`, a
deployment name, and either an API key or Microsoft Entra ID. Agent turns use
`/openai/v1/responses`, including tool calls with reasoning enabled. The
Responses route carries encrypted reasoning and tool results between turns;
`/chat/completions` cannot combine function tools with reasoning on GPT-5.6
and later Azure deployments. The saved Azure credentials remain in
`~/.config/jcode/azure-openai.env`. Azure omits reasoning effort unless you
set `AZURE_OPENAI_REASONING_EFFORT` for a deployment that supports it; this
avoids sending the global OpenAI default to non-reasoning deployments.
See the [Azure reasoning model guide](https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/reasoning#tool-calling-with-reasoning-models).

The native OpenAI providers use Responses WebSocket v2 with opportunistic
background prewarming and HTTPS fallback. See [OpenAI WebSocket transport](docs/OPENAI_WEBSOCKET.md)
for behavior, controls, and verification.

### Config-file setup for self-hosted endpoints and MCP

If you prefer to configure things by editing files instead of using the login UI, jcode supports both a custom OpenAI-compatible endpoint config and MCP config files.

#### OpenAI-compatible providers

Many hosted services speak the standard OpenAI `/v1/chat/completions` API. jcode talks to them through one shared OpenAI-compatible provider, so you can use almost any such endpoint without waiting for a dedicated integration.

There are two ways to set one up:

- **Built-in named profiles** — jcode ships ready-made profiles for several popular OpenAI-compatible services. Log in by id and jcode fills in the base URL and key environment variable for you:

  ```bash
  jcode login --provider <profile-id>
  # for example:
  jcode login --provider openrouter
  jcode login --provider orcarouter
  jcode login --provider deepseek
  jcode login --provider opencode      # OpenCode Zen
  jcode login --provider moonshotai
  jcode login --provider meta-muse     # Meta Model API / Muse Spark
  jcode login --provider yolo-auto     # Yolo-Auto
  ```

  Built-in OpenAI-compatible profile ids include: `openrouter`, `orcarouter`, `deepseek`, `zai`, `kimi`, `moonshotai`, `meta-muse` (Meta Model API / Muse Spark), `yolo-auto` (Yolo-Auto), `opencode` (OpenCode Zen), `opencode-go`, `302ai`, `baseten`, `cortecs`, `huggingface`, `nebius`, `scaleway`, `stackit`, and `firmware`. Each profile only sets the endpoint and key variable; you still pick the model with `/model` (or `--model`). Run `jcode login` with no provider to see the interactive list.

- **Any other endpoint** — point jcode at an arbitrary OpenAI-compatible API (hosted or local) with `jcode login --provider openai-compatible` or the scriptable `jcode provider add` command described below.

Useful environment overrides for these endpoints:

- `JCODE_STREAM_IDLE_TIMEOUT_SECS` — raise the base streaming idle timeout (default 180s) for slow reasoning models that think silently before emitting tokens. High reasoning efforts scale this automatically (high 2x, xhigh 3x, max 4x). Also settable as `[provider] stream_idle_timeout_secs` in `config.toml`.
- Per-model `context_window` (alias `context_limit`) in a `[[providers.<name>.models]]` entry — set the context window when the endpoint has no usable `/v1/models` response, so jcode does not fall back to the generic 200k default.
- `extra_body` — inject non-standard top-level fields into every chat/completions request body for backends that require them. See [Extra request-body fields](#extra-request-body-fields-extra_body) below.

For details on self-hosting, local runtimes, and the exact config file shape, see below.

#### Self-hosted OpenAI-compatible endpoints, including vLLM

For agents and scripts, the preferred path is the one-shot provider profile command. It writes a named profile to `~/.jcode/config.toml`, stores secrets in jcode's private app config directory when requested, and prints exact run/validation commands:

```bash
# Secret-safe setup for a hosted OpenAI-compatible API.
printf '%s' "$MY_API_KEY" | jcode provider add my-api \
  --base-url https://llm.example.com/v1 \
  --model my-model-id \
  --api-key-stdin \
  --set-default \
  --json

# Smoke test the profile.
jcode --provider-profile my-api auth-test --prompt 'Reply exactly JCODE_PROVIDER_SETUP_OK'

# Use it directly.
jcode --provider-profile my-api run 'hello'
```

For local servers that do not require auth:

```bash
jcode provider add local-vllm \
  --base-url http://localhost:8000/v1 \
  --model Qwen/Qwen3-Coder-30B-A3B-Instruct \
  --no-api-key \
  --set-default
```

Built-in local profiles are available for the common desktop/local runtimes:

```bash
# Ollama: start the local server and install a model first.
ollama pull llama3.2
jcode login --provider ollama
jcode --provider ollama --model llama3.2 run 'hello'

# LM Studio: start the Local Server, load a chat model, then use the exact
# model identifier shown by LM Studio or by curl http://localhost:1234/v1/models.
jcode login --provider lmstudio
jcode --provider lmstudio --model '<model-id>' run 'hello'
```

Ollama and LM Studio both expose OpenAI-compatible `/v1/models` and `/v1/chat/completions` endpoints. jcode uses streaming chat completions, function/tool calling, and OpenAI-style image content for vision-capable local models. If a local server requires a token, enter it during `jcode login` or create a named profile with `--api-key-stdin`.

Useful flags:

- `--api-key-env NAME`: reference an existing environment variable instead of storing a key.
- `--api-key-stdin`: read and store a key without putting it in shell history.
- `--context-window TOKENS`: persist the model context window for model selection and routing.
- `--overwrite`: replace an existing profile of the same name.
- `--model-catalog`: use the endpoint's `/models` response in addition to configured models.

The generated profile can also be edited manually in `~/.jcode/config.toml`:

```toml
[provider]
default_provider = "my-api"
default_model = "my-model-id"

[providers.my-api]
type = "openai-compatible"
base_url = "https://llm.example.com/v1"
api_key_env = "JCODE_PROVIDER_MY_API_API_KEY"
env_file = "provider-my-api.env"
default_model = "my-model-id"
# Optional: prevent model names such as `gpt-5-*` from automatically enabling
# `reasoning_effort` on gateways that reject it.
disable_reasoning_heuristics = true

[[providers.my-api.models]]
id = "my-model-id"
context_window = 128000
# Explicitly enable `/effort` and select this model's initial effort. Set
# `reasoning = false` on an individual model to disable it instead.
reasoning = true
reasoning_effort = "high"
```

Anthropic Messages-compatible gateways use the same named-profile surface with
`type = "anthropic-compatible"`. The profile can select bearer, custom-header,
or no authentication and attach gateway-specific headers to every request:

```toml
[provider]
default_provider = "corp-claude"
default_model = "claude-sonnet-4-6"

[providers.corp-claude]
type = "anthropic-compatible"
base_url = "https://gateway.example.com/anthropic/v1"
auth = "bearer"
api_key_env = "CORP_CLAUDE_TOKEN"
default_model = "claude-sonnet-4-6"

[providers.corp-claude.headers]
x-tenant-id = "tenant-42"

[[providers.corp-claude.models]]
id = "claude-sonnet-4-6"
context_window = 200000
```

For direct environment-based configuration, `ANTHROPIC_BASE_URL` overrides the
non-OAuth Messages endpoint and `ANTHROPIC_AUTH_TOKEN` is sent as a bearer token.
Claude OAuth traffic always continues to use Anthropic's official endpoints.

##### Extra request-body fields (`extra_body`)

Some OpenAI-compatible backends require non-standard top-level request fields. For example, NVIDIA NIM DeepSeek-V4 reasoning models (`deepseek-ai/deepseek-v4-flash`, `deepseek-ai/deepseek-v4-pro`) only enable thinking when the request includes `chat_template_kwargs`; without it they reply without reasoning (or, for some deployments, hang). jcode lets you inject arbitrary top-level fields two ways.

1. Per named profile, via `extra_body` in `config.toml` (a TOML table merged verbatim into the JSON body):

   ```toml
   [providers.my-nim]
   type = "openai-compatible"
   base_url = "https://integrate.api.nvidia.com/v1"
   api_key_env = "NVIDIA_API_KEY"
   default_model = "deepseek-ai/deepseek-v4-flash"

   [providers.my-nim.extra_body.chat_template_kwargs]
   thinking = true
   reasoning_effort = "high"
   ```

2. For built-in profiles (e.g. `nvidia-nim`) or any endpoint, via the `JCODE_OPENAI_EXTRA_BODY` environment variable (a JSON object string). It can live in the provider's env file (`~/.config/jcode/nvidia-nim.env`) next to the API key:

   ```bash
   JCODE_OPENAI_EXTRA_BODY={"chat_template_kwargs":{"thinking":true,"reasoning_effort":"high"}}
   ```

Keys from `extra_body` are merged last and override any jcode-generated body field with the same name (`JCODE_OPENAI_EXTRA_BODY` wins over the config `extra_body` on key collisions). Invalid values are logged and ignored rather than failing the request.

The custom OpenAI-compatible provider reads overrides from environment variables or from an env file in jcode's app config directory. On Linux this is usually `~/.config/jcode/`, so the default file is usually:

```text
~/.config/jcode/openai-compatible.env
```

Example for a local or LAN vLLM server:

```bash
JCODE_OPENAI_COMPAT_API_BASE=http://192.168.1.50:8000/v1
JCODE_OPENAI_COMPAT_DEFAULT_MODEL=Qwen/Qwen3-Coder-30B-A3B-Instruct
# Optional if your server expects auth
OPENAI_COMPAT_API_KEY=your-token-here
```

Notes:

- `jcode login --provider openai-compatible` can create or update this for you.
- Plain `http://` is accepted for `localhost` and private LAN IPs. Public remote HTTP is still rejected.
- HTTPS endpoints work as usual.

#### MCP config files

MCP config is separate from `config.toml`.

Primary config files:

- `~/.jcode/mcp.json` for global MCP servers
- `.jcode/mcp.json` for project-local MCP servers

Claude Code compatibility:

- `~/.claude.json` (Claude Code's user config): top-level `mcpServers`, plus per-project servers under `projects.<abs_path>.mcpServers` for the current directory
- `.mcp.json` at the repo root (Claude Code's project config)
- `.claude/mcp.json` (legacy fallback)

Claude Code config is read live on every load rather than copied into jcode's
global config. Additions, edits, and deletions therefore take effect without
leaving a stale snapshot (and inline environment values are not duplicated).
For migration from Codex CLI, jcode still performs a one-time import from
`~/.codex/config.toml` into `~/.jcode/mcp.json` when the latter does not exist.
That imported file is then jcode-owned; later Codex changes are not synced
automatically. Imported environment values are copied too and may contain
secrets.

Both the canonical `mcpServers` key and jcode's historical `servers` key are accepted. jcode currently supports stdio (command-based) servers only; HTTP/SSE entries (`"type": "http"`/`"sse"`) are recognized and skipped with a log line.

Example MCP config:

```json
{
  "mcpServers": {
    "filesystem": {
      "command": "/path/to/mcp-server",
      "args": ["--root", "/workspace"],
      "env": {},
      "shared": true
    },
    "websearch": {
      "command": "/path/to/slow-mcp-server",
      "timeout_secs": 120
    }
  }
}
```

Each request to an MCP server (`tools/call`, `tools/list`, `initialize`) times out after 30 seconds by default. Set `timeout_secs` on a server whose tools legitimately run longer.

### Scalable MCP tool discovery

Jcode avoids sending a large MCP catalog to the model on every turn. Under
`[tools]`, `mcp_tools = "auto"` and `"deferred"` keep the prompt tool list stable
when servers connect or register late. Supported Claude and OpenAI models load
matched tools through native deferred references. Other providers use the fixed
`mcp_search` and `mcp_call` surface. `mcp_tools = "eager"` exposes every tool
directly. The legacy `mcp_tools_token_threshold` setting is ignored.

Deferred discovery is two-stage:

1. `mcp_search` ranks tools by server, name, and description and returns 10
   compact matches by default. Results are deterministically paginated and the
   public maximum is 50 matches.
2. The model narrows the candidates and repeats the search with
   `include_schema: true`. Schema-bearing pages are capped at five tools before
   one is invoked through `mcp_call`.

This keeps full JSON schemas out of the model context until they are needed. A
10,000-tool regression test covers bounded discovery. The current ranker is a
deterministic in-process lexical ranker; it does not require an embedding model
or external search service. See [`docs/MCP_TOOL_DISCOVERY.md`](docs/MCP_TOOL_DISCOVERY.md).

For headless or SSH sessions, OAuth-style providers support `jcode login --provider <provider> --no-browser` (alias: `--headless`) so jcode prints the auth URL/QR and falls back to manual code or callback paste instead of trying to launch a local browser.

For more scriptable remote flows, `claude`, `openai`, `gemini`, and `antigravity` also support a two-step pattern:

```bash
# Step 1: print a resumable auth URL
jcode login --provider openai --print-auth-url --json

# Step 2: complete later with the callback URL or auth code
jcode login --provider openai --callback-url 'http://localhost:1455/auth/callback?...'
jcode login --provider gemini --auth-code '...'
```

Additional scriptable cases:

```bash
# Copilot device flow: print URL + user code, then complete later
jcode login --provider copilot --print-auth-url --json
jcode login --provider copilot --complete

# Gmail/Google OAuth after credentials are already configured
jcode login --provider google --print-auth-url --google-access-tier readonly
jcode login --provider google --callback-url 'http://127.0.0.1:8456?...'
```

Pending scriptable login state is stored under `~/.jcode/pending-login/`, automatically expires, and stale entries are cleaned up when new scriptable logins start or resume.

For the built-in OpenAI login flow, jcode opens a local callback on
`http://localhost:1455/auth/callback` by default.

<img width="2877" height="1762" alt="Screenshot from 2026-04-02 14-28-51" src="https://github.com/user-attachments/assets/530684c0-9d12-4363-aa0e-1b39a0d4e1be" />
The above image is the first page of provider logins

### Supported provider

- **Native / first-party style providers:** `claude`, `openai`, `copilot`, `gemini`, `azure`, `alibaba-coding-plan`
- **Aggregator / compatibility providers:** `openrouter`, `orcarouter`, `yolo-auto`, `openai-compatible`
- **Additional provider integrations:** `opencode`, `opencode-go`, `zai` / `kimi`, `302ai`, `baseten`, `cortecs`, `deepseek`, `firmware`, `huggingface`, `moonshotai`, `nebius`, `scaleway`, `stackit`, `groq`, `mistral`, `perplexity`, `togetherai`, `deepinfra`, `fireworks`, `novita`, `minimax`, `xai`, `lmstudio`, `ollama`, `chutes`, `cerebras`, `cursor`, `antigravity`, `google`

Jcode also supports easy multi-account switching. Ran out of tokens on your first ChatGPT Pro subscription? /account and quickly switch to your second. 

---

## Customizability / Self-Dev

Jcode is inventing a new form of customizability. One that doesn't limit you to what a plugin or extension can do. Tell your jcode agent to enter self dev mode, and it will start modifying its own source code. Jcode is optimized to iterate on itself. There is significant infrastructure around self development, which allows it to edit, build, and test its own source code, then reload its own binary and continue work in your (potentially many) sessions, fully automatically.


<!-- Add self-dev demo thumbnail/video and fuller writeup here. -->

## Other

- [Token optimization](docs/HARNESS.md#token-optimization)
- [KV cache](docs/HARNESS.md#kv-cache)
- [Hill-climbable measurements](docs/HARNESS.md#hill-climbable-measurements)
