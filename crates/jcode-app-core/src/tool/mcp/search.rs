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
            tool("analytics", "inspect_query", "Analyze PostgreSQL query performance"),
            tool("postgres", "query_performance", "Run a generic database operation"),
            tool("files", "search_files", "Find reports about query performance"),
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
    fn omits_schemas_until_the_model_explicitly_requests_them() {
        let catalog = vec![tool("postgres", "explain_query", "Explain a SQL query plan")];

        let compact = search_tools(catalog.clone(), SearchOptions::default());
        assert!(compact.matches[0].input_schema.is_none());

        let detailed = search_tools(
            catalog,
            SearchOptions {
                include_schema: true,
                ..SearchOptions::default()
            },
        );
        assert_eq!(detailed.matches[0].input_schema, Some(json!({
            "type": "object",
            "properties": {"query": {"type": "string"}}
        })));
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
}
