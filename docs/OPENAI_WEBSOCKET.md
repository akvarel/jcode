# OpenAI Responses WebSocket v2

Jcode's native OpenAI providers (`openai` and `openai-api`) prefer persistent
Responses WebSockets in `auto` transport mode. Every new socket, including a
prewarm socket, sends:

```text
OpenAI-Beta: responses_websockets=2026-02-06
```

This selects the WebSocket v2 protocol. It is not a `/v2/responses` endpoint.
API-key requests use `/v1/responses` (or the configured Responses API base).
ChatGPT/Codex OAuth requests use the subscription Responses backend. Custom
Chat Completions providers are unaffected.

## What prewarming does

When an idle client subscribes, Jcode snapshots its tools and static instructions
and starts preparation while the user types. This snapshot does not pin tools or
consume late MCP discovery. Idle preparation is polled once after acknowledging
the subscription. If local preparation would yield, it is abandoned rather than
holding the agent lock in front of user input. The agent also tries prewarming before local turn
context preparation. Both hooks prepare the static prefix in the background
using `response.create` with `generate: false`, `input: []`, and `store: false`.
The server returns a completed response ID without model
output. If the warmup finishes before generation is needed, Jcode continues on
that socket with `previous_response_id` and the actual conversation input.

- Warmup never executes tools or emits assistant output into the conversation.
- The foreground request does not wait for an unfinished warmup. It cancels it
  and follows the ordinary connection path instead.
- Every request setting must still match, including model, instructions, tools,
  reasoning effort, service tier, and cache policy. Credentials and endpoint
  must also match the warmup handshake.
- A successful existing conversation socket takes precedence over warmup.
- Warmup has a 5-second timeout. Unused state expires after 30 seconds. Model,
  credential, and transport resets discard speculative state. Forks do not
  inherit a parent's warmup socket.
- Expiring credentials skip warmup. Speculative work never rotates OAuth refresh
  tokens, so cancellation cannot discard newly issued credentials.
- Warmup errors do not fail the user's request or put the model into a transport
  cooldown. Ordinary WebSocket failure recovery and HTTPS fallback still apply.

The benefit depends on having preparation time to overlap with the network
work. A warmup miss is not treated as a failure. There is no guaranteed speedup
from the version header alone, and sending fewer bytes does not make earlier
context free of token charges.

## Opaque request rejection recovery

A WebSocket can return the bare error `invalid_request_error: Invalid request`
before `response.created` or any response activity, even on a fresh connection.
Jcode discards that failed socket and falls back to HTTP with the full normalized
input, without `previous_response_id`. This does not execute tools again or
change the model, tools, instructions, account, or persistent transport setting.
For a rejected continuation, the fallback skips another WebSocket attempt.

This recovery is deliberately narrow: it only handles an `error` frame with
`type: invalid_request_error`, exactly `message: Invalid request`, and no specific
error code, parameter, or associated response. Once response activity has begun,
the same error remains terminal. Schema, authentication, usage-limit, and other
specific validation errors retain their existing behavior. A non-retryable HTTP
400 is surfaced once rather than looping on the same rejected input.

Lifecycle logs identify continuation recovery with
`persistent_reuse_fallback_to_https` and reason `unaccepted_invalid_request`.
Fresh-socket recovery uses the existing `fallback_to_https` lifecycle event.
The remote cause of a bare rejection is not inferred from the error message.

Run the focused loopback and classifier regressions with:

```bash
cargo test -p jcode-provider-openai-runtime bare_invalid_request -- --test-threads=1
```

These tests exercise the public provider stream over local WebSocket and HTTP
servers. They do not claim live OpenAI incident reproduction or activation in an
already running daemon.

Validation on 2026-11-12: the three initial public-stream regressions failed
before the fix and passed afterward. All six recovery/classification regressions
pass, including both post-acceptance non-replay cases. The relocated service-tier
metadata regression also passes.

The broader command
`cargo test -p jcode-provider-openai-runtime -p jcode-provider-openai -- --test-threads=1`
passes all 51 request tests. Runtime results are 198 passed, seven failed, and
three ignored. Running the same command against unchanged `712eb30ff` gives 192
passed, the identical seven failures, and three ignored. Those existing failures
cover account-store fixture permissions (four tests), service-tier discovery,
strict tool-schema normalization, and prewarm nested-runtime handling. They are
not suppressed or rebaselined by this recovery change.

## Controls and diagnostics

```toml
[provider]
openai_transport = "auto" # auto | websocket | https
```

Prewarming is enabled by default for native OpenAI WebSockets. Set
`JCODE_OPENAI_PREWARM=0` (also `false` or `off`) in the **server process** environment
to disable speculative warmup without disabling persistent WebSockets. Setting
the transport to `https` disables both WebSockets and their warmup.

The provider's diagnostic summary includes `websocket_protocol=v2`. Lifecycle
logs include `ws_prewarm_ready`, `ws_prewarm_hit`, `ws_prewarm_miss`, and
`ws_prewarm_unavailable`. A hit uses the normal `websocket/persistent-reuse`
connection label. Logs do not include credential identities or warmup inputs.

## Verification

Run the runtime's offline regression suite:

```bash
cargo test -p jcode-provider-openai-runtime --lib -- --test-threads=1
```

The opt-in live test uses configured credentials and a few short model requests.
It checks a cold v2 connection, warmup consumption, and subsequent continuation
using the newly compiled provider, independently of the shared daemon:

```bash
cargo test -p jcode-provider-openai-runtime --lib \
  live_openai_v2_prewarm_and_continuation -- --ignored --nocapture --test-threads=1
```

Its single-sample time-to-first-text observations are not a benchmark. A real
latency comparison should measure cold and warmed requests across many turns,
report warmup hit rate, and include preparation cost when it cannot overlap
other work. See the [application-level validation report](OPENAI_WEBSOCKET_VALIDATION.md)
for a repeated enabled-versus-disabled experiment and its limitations.

Native `response.steer` and multiplexed `stream_id` support are separate features
and are not implemented by this change.

Sources: [OpenAI WebSocket guide](https://developers.openai.com/api/docs/guides/websocket-mode)
and [OpenAI Codex client](https://github.com/openai/codex/blob/main/codex-rs/core/src/client.rs).
