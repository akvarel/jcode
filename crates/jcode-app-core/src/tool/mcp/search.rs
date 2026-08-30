use crate::mcp::{McpToolDef, dispatch_name};
use serde::Serialize;
use serde_json::Value;
use std::cmp::Reverse;
use std::collections::HashSet;

const DEFAULT_LIMIT: usize = 10;
const MAX_LIMIT: usize = 50;
const MAX_SCHEMA_LIMIT: usize = 5;

#[derive(Debug, Default)]
pub(super) struct SearchOptions {
    pub server: Option<String>,
    pub query: Option<String>,
    pub limit: usize,
    pub offset: usize,
    pub include_schema: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct SearchPage {
    pub matches: Vec<SearchMatch>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct SearchMatch {
    pub name: String,
    pub server: String,
    pub tool: String,
    pub description: String,
    pub score: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<Value>,
}

pub(super) fn search_tools(
    catalog: Vec<(String, McpToolDef)>,
    options: SearchOptions,
) -> SearchPage {
    let requested_limit = if options.limit == 0 {
        DEFAULT_LIMIT
    } else {
        options.limit
    };
    let max_limit = if options.include_schema {
        MAX_SCHEMA_LIMIT
    } else {
        MAX_LIMIT
    };
    let limit = requested_limit.min(max_limit);
    let query = options.query.as_deref().map_or_else(String::new, normalize);
    let query_terms = terms(&query);

    let mut matches: Vec<SearchMatch> = catalog
        .into_iter()
        .filter(|(server, _)| {
            options
                .server
                .as_deref()
                .is_none_or(|wanted| wanted == server)
        })
        .filter_map(|(server, tool)| {
            let description = tool.description.unwrap_or_else(|| "MCP tool".to_string());
            let score = relevance_score(&query, &query_terms, &server, &tool.name, &description);
            if !query.is_empty() && score == 0 {
                return None;
            }
            Some(SearchMatch {
                name: dispatch_name(&server, &tool.name),
                server,
                tool: tool.name,
                description,
                score,
                input_schema: options.include_schema.then_some(tool.input_schema),
            })
        })
        .collect();

    matches.sort_by_key(|item| {
        (
            Reverse(item.score),
            item.server.to_ascii_lowercase(),
            item.tool.to_ascii_lowercase(),
        )
    });

    let total = matches.len();
    let offset = options.offset.min(total);
    let matches = matches.into_iter().skip(offset).take(limit).collect();
    SearchPage {
        matches,
        total,
        offset,
        limit,
        has_more: offset.saturating_add(limit) < total,
    }
}

fn relevance_score(
    query: &str,
    query_terms: &[String],
    server: &str,
    tool: &str,
    description: &str,
) -> u32 {
    if query.is_empty() {
        return 0;
    }

    let server = normalize(server);
    let tool = normalize(tool);
    let description = normalize(description);
    let server_terms: HashSet<String> = terms(&server).into_iter().collect();
    let tool_terms: HashSet<String> = terms(&tool).into_iter().collect();
    let description_terms: HashSet<String> = terms(&description).into_iter().collect();

    let mut score = 0_u32;
    if tool == query {
        score += 1_000;
    } else if tool.contains(query) {
        score += 500;
    }
    if server == query {
        score += 700;
    } else if server.contains(query) {
        score += 350;
    }
    if description.contains(query) {
        score += 200;
    }

    for term in query_terms {
        if tool_terms.contains(term) {
            score += 120;
        } else if tool_terms
            .iter()
            .any(|candidate| candidate.starts_with(term))
        {
            score += 60;
        }
        if server_terms.contains(term) {
            score += 80;
        } else if server_terms
            .iter()
            .any(|candidate| candidate.starts_with(term))
        {
            score += 40;
        }
        if description_terms.contains(term) {
            score += 30;
        } else if description_terms
            .iter()
            .any(|candidate| candidate.starts_with(term))
        {
            score += 15;
        }
    }
    score
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn terms(value: &str) -> Vec<String> {
    value.split_whitespace().map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::{SearchOptions, search_tools};
    use crate::mcp::McpToolDef;
    use serde_json::json;

    fn tool(server: &str, name: &str, description: &str) -> (String, McpToolDef) {
        (
            server.to_string(),
            McpToolDef {
                name: name.to_string(),
                description: Some(description.to_string()),
                input_schema: json!({
                    "type": "object",
                    "properties": {"query": {"type": "string"}}
                }),
            },
        )
    }

    #[test]
    fn ranks_exact_names_before_description_only_matches() {
        let catalog = vec![
            tool(
                "analytics",
                "inspect_query",
                "Analyze PostgreSQL query performance",
            ),
            tool(
                "postgres",
                "query_performance",
                "Run a generic database operation",
            ),
            tool(
                "files",
                "search_files",
                "Find reports about query performance",
            ),
        ];

        let page = search_tools(
            catalog,
            SearchOptions {
                query: Some("query performance".to_string()),
                ..SearchOptions::default()
            },
        );

        assert_eq!(page.matches[0].server, "postgres");
        assert_eq!(page.matches[0].tool, "query_performance");
        assert!(page.matches[0].score > page.matches[1].score);
    }

    #[test]
    fn returns_a_bounded_page_with_stable_pagination_metadata() {
        let catalog = (0..100)
            .map(|index| tool("bulk", &format!("tool_{index:03}"), "bulk operation"))
            .collect();

        let page = search_tools(
            catalog,
            SearchOptions {
                query: Some("bulk".to_string()),
                limit: 7,
                offset: 14,
                ..SearchOptions::default()
            },
        );

        assert_eq!(page.total, 100);
        assert_eq!(page.offset, 14);
        assert_eq!(page.limit, 7);
        assert_eq!(page.matches.len(), 7);
        assert!(page.has_more);
        assert_eq!(page.matches[0].tool, "tool_014");
    }

    #[test]
    fn clamps_requested_page_size_to_the_public_maximum() {
        let catalog = (0..100)
            .map(|index| tool("bulk", &format!("tool_{index:03}"), "bulk operation"))
            .collect();

        let page = search_tools(
            catalog,
            SearchOptions {
                limit: usize::MAX,
                ..SearchOptions::default()
            },
        );

        assert_eq!(page.limit, 50);
        assert_eq!(page.matches.len(), 50);
    }

    #[test]
    fn limits_schema_expansion_to_five_shortlisted_tools() {
        let catalog = (0..20)
            .map(|index| tool("bulk", &format!("tool_{index:03}"), "bulk operation"))
            .collect();

        let page = search_tools(
            catalog,
            SearchOptions {
                limit: 50,
                include_schema: true,
                ..SearchOptions::default()
            },
        );

        assert_eq!(page.limit, 5);
        assert_eq!(page.matches.len(), 5);
        assert!(page.matches.iter().all(|item| item.input_schema.is_some()));
    }

    #[test]
    fn omits_schemas_until_the_model_explicitly_requests_them() {
        let catalog = vec![tool(
            "postgres",
            "explain_query",
            "Explain a SQL query plan",
        )];

        let compact = search_tools(catalog.clone(), SearchOptions::default());
        assert!(compact.matches[0].input_schema.is_none());

        let detailed = search_tools(
            catalog,
            SearchOptions {
                include_schema: true,
                ..SearchOptions::default()
            },
        );
        assert_eq!(
            detailed.matches[0].input_schema,
            Some(json!({
                "type": "object",
                "properties": {"query": {"type": "string"}}
            }))
        );
    }

    #[test]
    fn applies_an_exact_server_filter_before_ranking() {
        let catalog = vec![
            tool("postgres-primary", "search", "Search PostgreSQL records"),
            tool("postgres-archive", "search", "Search PostgreSQL records"),
        ];

        let page = search_tools(
            catalog,
            SearchOptions {
                server: Some("postgres-archive".to_string()),
                query: Some("search".to_string()),
                ..SearchOptions::default()
            },
        );

        assert_eq!(page.total, 1);
        assert_eq!(page.matches[0].server, "postgres-archive");
    }

    #[test]
    fn searches_ten_thousand_tools_without_expanding_their_schemas() {
        let catalog = (0..10_000)
            .map(|index| {
                let description = if index == 7_777 {
                    "Diagnose PostgreSQL query latency and execution plans"
                } else {
                    "Generic enterprise operation"
                };
                tool(
                    &format!("server_{:04}", index / 10),
                    &format!("tool_{index:05}"),
                    description,
                )
            })
            .collect();

        let page = search_tools(
            catalog,
            SearchOptions {
                query: Some("postgresql query latency".to_string()),
                ..SearchOptions::default()
            },
        );

        assert_eq!(page.matches.len(), 1);
        assert_eq!(page.matches[0].tool, "tool_07777");
        assert!(page.matches[0].input_schema.is_none());
        assert!(!serde_json::to_string(&page).unwrap().contains("properties"));
    }
}
