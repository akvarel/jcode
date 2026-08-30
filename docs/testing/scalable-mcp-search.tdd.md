# Scalable MCP Search TDD Evidence

## Source

The user requested scalable MCP discovery in Jcode and integration into ohAgent.
The journeys and acceptance criteria were derived during this implementation.

## User journeys

1. As an agent using thousands of MCP tools, I want a ranked bounded shortlist
   so that irrelevant schemas do not consume the context window.
2. As an agent preparing a tool call, I want schemas only for a small explicit
   shortlist so that I can construct valid arguments without expanding the
   entire catalog.
3. As an ohAgent operator, I want this behavior inherited from the pinned Jcode
   runtime rather than reimplemented in the product layer.

## RED/GREEN checkpoints

| Behavior | RED evidence | GREEN evidence |
|---|---|---|
| Ranked, bounded, paginated, schema-lazy search | `3cef50cd0`: compile-time RED because `SearchOptions` and `search_tools` did not exist | `b6df4d6d2`: focused tests passed 6/6 |
| Schema expansion limited to five matches | `df3d547da`: runtime RED, expected limit 5 but observed 50 | `bc923f27c`: focused tests passed 7/7 |

## Test specification

| # | Guarantee | Test | Type | Result |
|---|---|---|---|---|
| 1 | Exact tool-name matches outrank description-only matches | `ranks_exact_names_before_description_only_matches` | Unit | PASS |
| 2 | Pagination is bounded and stable | `returns_a_bounded_page_with_stable_pagination_metadata` | Unit | PASS |
| 3 | Compact pages are capped at 50 | `clamps_requested_page_size_to_the_public_maximum` | Unit | PASS |
| 4 | Schemas are omitted until explicitly requested | `omits_schemas_until_the_model_explicitly_requests_them` | Unit | PASS |
| 5 | Schema-bearing pages are capped at five | `limits_schema_expansion_to_five_shortlisted_tools` | Unit | PASS |
| 6 | Exact server filtering happens before ranking | `applies_an_exact_server_filter_before_ranking` | Unit | PASS |
| 7 | A 10,000-tool catalog returns only the relevant compact result without schema content | `searches_ten_thousand_tools_without_expanding_their_schemas` | Scale regression | PASS |
| 8 | Auto/eager/deferred MCP exposure remains compatible | `mcp_exposure_modes_select_eager_or_fixed_definitions`, `auto_mode_rechecks_late_mcp_definitions_before_deferring` | Integration | PASS |

## Commands executed

```text
cargo test -p jcode-app-core tool::mcp::search::tests --lib
cargo test -p jcode-app-core agent::tests::mcp_exposure_modes_select_eager_or_fixed_definitions --lib
cargo test -p jcode-app-core agent::tests::auto_mode_rechecks_late_mcp_definitions_before_deferring --lib
cargo clippy -p jcode-app-core --lib -- -D warnings
```

## Coverage and known gaps

All new ranking, pagination, schema deferral, server filtering, and 10,000-tool
paths have direct tests. The implementation intentionally uses deterministic
lexical ranking. Embedding-based synonym or multilingual retrieval is not part
of this change.
