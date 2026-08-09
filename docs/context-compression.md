# Graphify context compression

Jcode can compile Graphify's code graph into a bounded context package before
the memory prompt is sent to an LLM. Graphify remains the source of truth for
code relationships. Jcode only ranks, deduplicates, budgets, and serializes the
retrieved nodes.

## Existing architecture reused

- `CompactionManager` remains responsible for conversation-history compaction
  and hard context-window recovery.
- Provider runtimes continue to report input/output/cache tokens through the
  existing turn telemetry.
- Tool outputs already have a visible history cap in
  `agent/tools.rs`; this implementation does not create a competing log path.
- Swarm task graphs, completion artifacts, and typed protocol messages remain
  the structured agent-to-agent state layer.
- Existing `agentgrep`, `read`, patch, and edit tools provide on-demand detail
  and patch-oriented editing.

The new compiler is intentionally limited to graph-derived request context.

## Modes

```toml
[context_compression]
mode = "off" # reproducible baseline
graph_token_budget = 1200
max_graph_items = 24
max_tool_output_chars = 65536
```

Set `mode = "graph_compact"` to enable graph-aware compilation. The equivalent
environment variables are:

- `JCODE_CONTEXT_COMPRESSION_MODE`
- `JCODE_CONTEXT_GRAPH_TOKEN_BUDGET`
- `JCODE_CONTEXT_MAX_GRAPH_ITEMS`
- `JCODE_CONTEXT_MAX_TOOL_OUTPUT_CHARS`

`graph_compact` implicitly enables Graphify retrieval. The older
`agents.memory_graphify_enabled` flag remains supported.

## Request flow

1. The memory pipeline decides whether the user turn is code-related.
2. Jcode invokes `graphify query <task> --format compact --budget <tokens>`.
3. The context compiler parses Graphify nodes without creating another index.
4. Task-matching roots are ranked first, exact duplicates are removed, and the
   item/token budgets are enforced.
5. A versioned JSON package is injected through the existing memory prompt.

Each item contains a stable graph reference and a revision hash derived from
the exact Graphify node representation. L0 contains topology-only nodes. L1
adds source and location metadata. Implementation bodies remain on-demand via
existing `agentgrep`, `read`, and Graphify tools rather than being injected by
default.

## Telemetry

Every successful compilation emits a structured `CONTEXT_COMPILATION` log with
the mode/version, candidates, selected items, deduplicated items, dropped items,
estimated tokens, and budget. Provider token/cache usage continues through the
existing turn telemetry.

Oversized tool results in graph-compact mode are written once to the local
content store using a deterministic revision hash. Prompt history receives a
bounded prefix and a structured `raw_ref`, so token reduction does not destroy
diagnostic evidence. Raw artifacts use owner-only permissions on Unix and are
bounded to 64 MiB; larger outputs fall back to the existing history safety cap.

## Baseline comparison

Run the same graph query through both representations:

```bash
cargo run -q -p jcode-base --example context_compression_eval -- \
  "messages_for_provider context compaction"
```

The JSON report includes baseline and compressed bytes/tokens/items plus the
input compression ratio. Token values are estimates unless a provider exposes
actual request usage. Do not treat an estimated reduction as a quality result.
Relevant tests and task validation must still pass.

The checked-in `context-compression-evaluation.json` records an offline run over
eight representative task categories. At commit `dd592426`, it measured 12,951
baseline estimated input tokens versus 9,308 graph-compact estimated tokens, a
`1.391x` input compression ratio. These are deterministic character-based
estimates, not provider billing claims. Cost, cached-token, latency, and task
quality comparisons require controlled live model runs and are therefore left
unknown rather than fabricated.

## Current limits

- The first production slice compiles Graphify L0/L1 topology and signatures.
- L2 summaries and L3 symbol bodies remain explicit on-demand retrieval.
- Tool-output normalization and cross-agent task-state IR are separate follow-up
  integrations and are not silently applied by this mode.
- Full raw Graphify output is not discarded by Graphify and can be reproduced
  from the same repository revision and query.
