# Scalable MCP Tool Discovery

## Purpose

Large MCP installations must not place every tool description and JSON schema
in the model context. Jcode therefore supports deferred discovery through two
fixed tools: `mcp_search` and `mcp_call`.

## Exposure modes

Configure the behavior in `~/.jcode/config.toml`:

```toml
[tools]
mcp_tools = "auto" # auto | eager | deferred
mcp_tools_token_threshold = 8000
```

- `eager` exposes every connected MCP tool directly.
- `deferred` exposes only `mcp_search` and `mcp_call`.
- `auto` uses eager exposure until the filtered MCP definitions exceed the
  configured token threshold, then switches to deferred exposure.

The searchable catalog combines live definitions with the on-disk schema cache
for configured servers. Live definitions replace cached definitions when both
exist. Calling a cached tool can connect its server on first use.

## Search contract

`mcp_search` accepts:

- `query`: optional natural-language terms matched against server names, tool
  names, and descriptions;
- `server`: optional exact server filter;
- `limit`: result count, default 10 and maximum 50;
- `offset`: deterministic pagination offset;
- `include_schema`: include input schemas after narrowing the shortlist.

Compact searches omit `input_schema`. Schema-bearing searches are capped at
five matches even when a larger limit is requested. The result envelope reports
`matches`, `total`, `offset`, `limit`, and `has_more`.

Ranking is deterministic. Exact tool and server names rank above phrase and
individual-term matches in names and descriptions. Equal scores are ordered by
server and tool name, which keeps pagination stable.

## Recommended model workflow

1. Search with a concise capability query and no schemas.
2. Inspect the top compact matches.
3. Restrict by server or a more precise query and set `include_schema: true`.
4. Invoke the selected raw server/tool pair through `mcp_call`.

Avoid empty searches for ordinary work. They intentionally support catalog
browsing, but ranking is most useful when the request contains capability terms.

## Scale and limits

The implementation scans the in-memory catalog and bounds model-visible output.
A regression test covers 10,000 tools and verifies that compact results contain
no schemas. This removes context growth as the primary scaling bottleneck.

The ranker is lexical rather than embedding-based. It does not currently provide
cross-language semantic similarity or synonym expansion. Those can be added
behind the same public `mcp_search` contract without changing model workflows.

## Security and policy

Search results are filtered through the session MCP allow/deny policy before
ranking. `mcp_call` checks the same policy again before dispatch. Deferred
discovery does not bypass tool permissions or server connection controls.
