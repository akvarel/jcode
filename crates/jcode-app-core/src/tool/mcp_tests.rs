use super::*;
use crate::tool::Tool;
use std::fs;
use std::path::PathBuf;

fn create_test_tool() -> McpManagementTool {
    // Use an explicit empty config so tests are hermetic: McpManager::new()
    // would load the developer's real ~/.jcode/mcp.json, and list output
    // now includes configured-but-not-connected servers (issue #436).
    let manager = Arc::new(RwLock::new(McpManager::with_config(
        crate::mcp::McpConfig::default(),
    )));
    McpManagementTool::new(manager)
}

fn create_test_context() -> ToolContext {
    ToolContext {
        session_id: "test-session".to_string(),
        message_id: "test-message".to_string(),
        tool_call_id: "test-tool-call".to_string(),
        working_dir: None,
        stdin_request_tx: None,
        graceful_shutdown_signal: None,
        execution_mode: crate::tool::ToolExecutionMode::Direct,
    }
}

struct LocalMcpConfigGuard {
    path: PathBuf,
    backup: Option<String>,
    created_dir: bool,
}

impl LocalMcpConfigGuard {
    fn new(content: &str) -> std::io::Result<Self> {
        let path = PathBuf::from(".jcode/mcp.json");
        let dir = path
            .parent()
            .ok_or_else(|| std::io::Error::other("missing parent"))?;
        let created_dir = if !dir.exists() {
            fs::create_dir_all(dir)?;
            true
        } else {
            false
        };
        let backup = if path.exists() {
            Some(fs::read_to_string(&path)?)
        } else {
            None
        };
        fs::write(&path, content)?;
        Ok(Self {
            path,
            backup,
            created_dir,
        })
    }
}

impl Drop for LocalMcpConfigGuard {
    fn drop(&mut self) {
        match &self.backup {
            Some(content) => {
                let _ = fs::write(&self.path, content);
            }
            None => {
                let _ = fs::remove_file(&self.path);
                if self.created_dir
                    && let Some(dir) = self.path.parent()
                {
                    let _ = fs::remove_dir(dir);
                }
            }
        }
    }
}

#[test]
fn test_tool_name() {
    let tool = create_test_tool();
    assert_eq!(tool.name(), "mcp");
}

#[test]
fn test_tool_description() {
    let tool = create_test_tool();
    assert!(tool.description().contains("MCP"));
    assert!(tool.description().contains("Model Context Protocol"));
}

#[test]
fn test_parameters_schema() {
    let tool = create_test_tool();
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["action"].is_object());
    assert!(schema["properties"]["server"].is_object());
    assert!(schema["properties"]["command"].is_object());
    assert_eq!(schema["required"], json!(["action"]));
    assert!(
        schema["properties"]["action"]["enum"]
            .as_array()
            .unwrap()
            .contains(&json!("search"))
    );
    assert!(
        schema["properties"]["action"]["enum"]
            .as_array()
            .unwrap()
            .contains(&json!("call"))
    );
    assert_eq!(
        schema["properties"]["arguments"]["additionalProperties"],
        true
    );
}

async fn unified_registry() -> crate::tool::Registry {
    let registry = crate::tool::Registry::empty();
    let manager = Arc::clone(create_test_tool().manager());
    registry
        .register(
            "mcp".into(),
            Arc::new(McpManagementTool::new(manager.clone()).with_registry(registry.clone())),
        )
        .await;
    registry
        .register(
            "mcp_search".into(),
            Arc::new(McpSearchTool::new(manager.clone()).with_registry(registry.clone())),
        )
        .await;
    registry
        .register(
            "mcp_call".into(),
            Arc::new(McpCallTool::new(manager).with_registry(registry.clone())),
        )
        .await;
    registry
}

#[tokio::test]
async fn unified_mcp_hides_alias_schemas_but_preserves_execution() {
    let registry = unified_registry().await;
    let definitions = registry.definitions(None).await;
    assert_eq!(
        definitions
            .iter()
            .map(|d| d.name.as_str())
            .collect::<Vec<_>>(),
        vec!["mcp"]
    );
    let ctx = create_test_context();
    let unified = registry
        .execute("mcp", json!({"action":"search"}), ctx.clone())
        .await
        .unwrap();
    let legacy = registry
        .execute("mcp_search", json!({}), ctx.clone())
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&unified.output).unwrap(),
        json!({"matches": [], "total": 0, "offset": 0, "limit": 10, "has_more": false})
    );
    assert_eq!(unified.output, legacy.output);
    assert_eq!(unified.metadata, legacy.metadata);
    for (name, input) in [("mcp", json!({"action":"call"})), ("mcp_call", json!({}))] {
        let error = registry
            .execute(name, input, ctx.clone())
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("missing field `server`"),
            "{error}"
        );
    }
    let script_defs = registry.definitions_for_session(&ctx.session_id).await;
    assert_eq!(
        script_defs
            .iter()
            .map(|d| d.name.as_str())
            .collect::<Vec<_>>(),
        vec!["mcp"]
    );
}

#[tokio::test]
async fn unified_mcp_respects_legacy_surface_and_management_policies() {
    use std::collections::HashSet;
    let registry = unified_registry().await;
    let mut ctx = create_test_context();
    ctx.session_id = "unified-mcp-action-policy".into();
    let _policy = crate::tool::register_session_tool_policy(
        &ctx.session_id,
        Some(HashSet::from(["mcp_search".into()])),
        HashSet::new(),
    );
    assert_eq!(
        registry
            .definitions(Some(&HashSet::from(["mcp_search".into()])))
            .await
            .len(),
        1
    );
    assert!(
        registry
            .execute("mcp", json!({"action":"search"}), ctx.clone())
            .await
            .is_ok()
    );
    for action in [
        "call",
        "list",
        "connect",
        "disconnect",
        "reload",
        "status",
        "logout",
    ] {
        let error = registry
            .execute("mcp", json!({"action":action}), ctx.clone())
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("not allowed"),
            "{action}: {error}"
        );
    }
    let _policy = crate::tool::register_session_tool_policy(
        &ctx.session_id,
        Some(HashSet::from(["mcp".into()])),
        HashSet::from(["mcp_call".into()]),
    );
    let error = registry
        .execute(
            "mcp",
            json!({"action":"call", "server":"x", "tool":"y"}),
            ctx.clone(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("disabled"), "{error}");
    assert!(
        registry
            .execute("mcp", json!({"action":"search"}), ctx.clone())
            .await
            .is_ok()
    );
    let _policy = crate::tool::register_session_tool_policy(
        &ctx.session_id,
        Some(HashSet::from(["mcp__x__y".into()])),
        HashSet::from(["mcp__x__y".into()]),
    );
    let error = registry
        .execute(
            "mcp",
            json!({"action":"call", "server":"x", "tool":"y"}),
            ctx,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("not allowed"), "{error}");
}

#[test]
fn mcp_call_allows_dynamic_argument_keys_in_provider_schemas() {
    let tool = create_test_tool();
    let schema = tool.parameters_schema();
    assert_eq!(
        schema["properties"]["arguments"]["additionalProperties"],
        true
    );

    for spec in [
        &jcode_schema_dialect::registry::OPENROUTER,
        &jcode_schema_dialect::registry::OPENAI,
        &jcode_schema_dialect::registry::ANTHROPIC,
    ] {
        let normalized = jcode_schema_dialect::dialect::apply(&schema, spec);
        let arguments = &normalized["properties"]["arguments"];
        assert_eq!(arguments["type"], "object", "{}", spec.id);
        assert_eq!(arguments["additionalProperties"], true, "{}", spec.id);
        if spec.transforms.require_properties_on_objects {
            // Empty declared properties must not close the dynamic payload (#1214).
            assert_eq!(arguments["properties"], json!({}), "{}", spec.id);
        }
        assert_eq!(normalized["required"], schema["required"], "{}", spec.id);
    }
}

#[test]
fn mcp_call_dynamic_arguments_remain_ineligible_for_openai_strict_mode() {
    let tool = create_test_tool();
    let compatible =
        jcode_provider_core::openai_schema::openai_compatible_schema(&tool.parameters_schema());
    assert!(!jcode_provider_core::openai_schema::schema_supports_strict(
        &compatible
    ));
    assert_eq!(
        compatible["properties"]["arguments"]["additionalProperties"],
        true
    );
}

// Ranked, paginated discovery is covered by mcp/search.rs tests.

#[test]
fn mcp_search_tool_references_equal_paginated_match_names() {
    let catalog = ["alpha", "beta", "gamma"]
        .into_iter()
        .map(|name| {
            (
                "srv".to_string(),
                crate::mcp::McpToolDef {
                    name: name.to_string(),
                    description: Some(format!("{name} search tool")),
                    input_schema: json!({"type": "object"}),
                },
                format!("srv_{name}"),
            )
        })
        .collect();
    let page = search::search_tools(
        catalog,
        search::SearchOptions {
            server: None,
            query: Some("search".to_string()),
            limit: 2,
            offset: 0,
            include_schema: false,
        },
    );
    assert_eq!(page.total, 3);
    assert_eq!(page.matches.len(), 2);
    let output = mcp_search_output(&page).expect("search output renders");
    let expected: Vec<&str> = page.matches.iter().map(|m| m.name.as_str()).collect();
    let metadata = output.metadata.as_ref().expect("tool_references metadata");
    let references = metadata["tool_references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(references, expected);
}

#[tokio::test]
async fn mcp_search_times_out_when_manager_lifecycle_lock_is_stuck() {
    let manager = Arc::new(RwLock::new(McpManager::with_config(
        crate::mcp::McpConfig::default(),
    )));
    let tool = McpSearchTool::new(manager.clone());
    let held_write_lock = manager.write().await;
    let started = tokio::time::Instant::now();
    let error = tool
        .execute(json!({"query": "drive"}), create_test_context())
        .await
        .expect_err("discovery must fail fast instead of waiting forever");
    let elapsed = started.elapsed();
    drop(held_write_lock);
    assert!(
        error.to_string().contains("did not respond within 500ms"),
        "{error:#}"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "discovery timed out too late: {elapsed:?}"
    );
}

#[tokio::test]
async fn test_list_empty() {
    let tool = create_test_tool();
    let ctx = create_test_context();
    let input = json!({"action": "list"});

    let result = tool.execute(input, ctx).await.unwrap();
    assert!(result.output.contains("No MCP servers connected"));
}

#[tokio::test]
async fn test_list_shows_disabled_configured_server() {
    // Issue #436: disabled servers stay visible in the list with their
    // state, so users can see and enable them on demand.
    let mut config = crate::mcp::McpConfig::default();
    config.servers.insert(
        "off-server".to_string(),
        McpServerConfig {
            command: "some-bin".to_string(),
            args: vec![],
            env: HashMap::new(),
            shared: true,
            transport: None,
            url: None,
            headers: HashMap::new(),
            enabled: Some(false),
            disabled: None,
            timeout_secs: None,
        },
    );
    let manager = Arc::new(RwLock::new(McpManager::with_config(config)));
    let tool = McpManagementTool::new(manager);
    let ctx = create_test_context();

    let result = tool.execute(json!({"action": "list"}), ctx).await.unwrap();
    assert!(
        result.output.contains("off-server"),
        "disabled server must be listed: {}",
        result.output
    );
    assert!(
        result.output.contains("disabled in config"),
        "disabled state must be visible: {}",
        result.output
    );
}

#[tokio::test]
async fn test_connect_missing_server() {
    let tool = create_test_tool();
    let ctx = create_test_context();
    let input = json!({"action": "connect", "command": "/bin/test"});

    let result = tool.execute(input, ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("server"));
}

#[tokio::test]
async fn test_connect_missing_command() {
    let tool = create_test_tool();
    let ctx = create_test_context();
    let input = json!({"action": "connect", "server": "test"});

    let result = tool.execute(input, ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("command"));
}

#[tokio::test]
async fn test_disconnect_not_connected() {
    let tool = create_test_tool();
    let ctx = create_test_context();
    let input = json!({"action": "disconnect", "server": "nonexistent"});

    let result = tool.execute(input, ctx).await.unwrap();
    assert!(result.output.contains("not connected"));
}

#[tokio::test]
async fn test_unknown_action() {
    let tool = create_test_tool();
    let ctx = create_test_context();
    let input = json!({"action": "invalid_action"});

    let result = tool.execute(input, ctx).await.unwrap();
    assert!(result.output.contains("Unknown action"));
}

#[tokio::test]
async fn test_reload_empty_config() {
    let _guard =
        LocalMcpConfigGuard::new("{\"servers\":{}}").expect("create temporary .jcode/mcp.json");
    let tool = create_test_tool();
    let ctx = create_test_context();
    let input = json!({"action": "reload"});

    let result = tool.execute(input, ctx).await.unwrap();
    // With config merging, global config may have servers.
    // If both are empty: "No servers found in config"
    // If global has servers: "Reloaded MCP config" (may show connection failures)
    assert!(
        result.output.contains("No servers")
            || result.output.contains("Empty config")
            || result.output.contains("Connected servers: 0")
            || result.output.contains("Reloaded MCP config")
    );
}
