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

## Definition-of-Done traceability

The table below maps every explicit requirement from
`orangehat-context-compression-task.md` to an observed check. “Reused” means the
capability already existed in Jcode and was verified rather than duplicated.

| Requirement | Implementation/evidence | Check and observed result |
| --- | --- | --- |
| Inspect and reuse Graphify | `memory_external.rs` invokes the installed `graphify query`; the compiler consumes compact `NODE` rows instead of building another index | `graphify update .` completed with 39,718 nodes and 97,395 edges; the evaluation example executed the real Graphify CLI |
| Context Compiler integrated into LLM execution | `memory::get_relevant_* -> memory_external::enrich_context -> query_graphify -> compile_graphify_context`; the resulting `MemoryEntry` is injected by the existing prompting path | Production call path was inspected and compiler tests passed. A paid/provider-backed `jcode run` was not executed, so final provider acceptance remains blocked |
| Graph-aware selection | Lexical task roots are ranked over Graphify nodes, with source/community metadata retained | `compiler_ranks_task_roots_and_deduplicates` passed |
| Multiple representation levels | `ContextRepresentationLevel::{L0,L1}` | Compiler serialization tests passed and emitted both levels where budget allowed |
| Intelligent token budgets | Hard token/item limits, task-root preservation, and effective budget capped to the baseline Graphify response | `compiler_enforces_budget_and_preserves_a_root` passed; a real case that initially regressed from 922 to 1,198 estimated tokens was fixed and re-ran at 882 to 856 tokens (1.030x) |
| Structured agent-to-agent state | Reused typed swarm `HandoffArtifact` and DAG artifact propagation | `e2e_complete_flows_artifact_to_downstream_assignment` and `deep_turn_without_artifact_requeues_then_fails` passed |
| Structured outputs validated | Reused strict swarm artifact deserialization/validation and failure/requeue behavior | The two swarm tests above passed, including rejection of missing artifacts |
| Safe references/deduplication | SHA-256 content/revision references, exact identity deduplication, collision-content verification | Compiler reference and raw-evidence tests passed |
| Symbol-level context preferred to full files | Graphify compact symbol nodes are the compiler input and emitted unit | Real Graphify evaluation emitted 14 selected nodes from 20 candidates for the traced runtime task |
| Tool output compression with retrievable evidence | GraphCompact stores oversized raw output in `JCODE_HOME/context-store` and emits a bounded reference | `raw_tool_output_is_retrievable_after_compaction` plus both app-core tool-history tests passed |
| Token/cost/latency telemetry | Compiler emits estimated context tokens and selection counts; reused session/provider telemetry carries billed token/cost and tool latency fields | Existing telemetry serialization path was inspected. Live billed cost/latency was not collected |
| Baseline/compressed comparison | `context_compression_eval` and the eight-case JSON artifact | Eight-case aggregate: 12,951 baseline vs 9,308 compressed estimated tokens, 1.391x. A newly checked smaller result produced 882 vs 856, 1.030x |
| Automated core tests | Compiler, configuration, tool-history, compaction, and swarm artifact tests | Compiler 4/4, config 2/2, tool integration 3/3, compaction 35/35, mapped swarm tests 2/2 passed |
| Existing project tests pass | Full `jcode-base` + `jcode-app-core` suite | 1,142 passed, 6 ignored, 2 failed. Both failures are pre-existing tool-schema description token caps, so this requirement is not fully satisfied |
| Benchmark/evaluation produced | `docs/context-compression-evaluation.json` | Artifact contains eight categories and aggregate metrics |
| Documentation | This document plus generated evaluation JSON | Configuration, flow, storage, telemetry, evaluation, limitations, and traceability are documented |
| No unsupported savings claims | Results are labelled byte/4 estimates and live provider metrics are explicitly unavailable | Documentation review and `git diff --check` passed |

### Public-interface acceptance

The committed project built the actual `jcode` executable. `jcode --help`,
`jcode run --help`, and `jcode memory --help` completed successfully. The binary
also started with all four `JCODE_CONTEXT_*` environment variables set. The
end-to-end provider boundary was not invoked because it requires configured live
provider credentials and can incur usage. Therefore this change is integrated
and representative, but not claimed as provider-acceptance-complete.
