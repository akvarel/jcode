use super::*;
use crate::provider::Provider;
use tokio_stream::StreamExt;

#[test]
fn parse_fetch_available_models_response_discovers_metadata_and_priority_order() {
    let response: FetchAvailableModelsResponse = serde_json::from_value(serde_json::json!({
        "defaultAgentModelId": "gemini-3.1-pro-high",
        "commandModelIds": ["gemini-3-flash"],
        "models": {
            "claude-opus-4-6-thinking": {
                "displayName": "Claude Opus 4.6 (Thinking)",
                "quotaInfo": { "remainingFraction": 1, "resetTime": "2026-04-24T20:53:26Z" },
                "recommended": true,
                "modelProvider": "MODEL_PROVIDER_ANTHROPIC"
            },
            "gemini-3.1-pro-high": {
                "displayName": "Gemini 3.1 Pro (High)",
                "quotaInfo": { "remainingFraction": 0.25 }
            },
            "gemini-3-flash": {
                "displayName": "Gemini 3 Flash",
                "quotaInfo": { "remainingFraction": 0, "resetTime": "2026-04-24T21:53:26Z" }
            },
            "gpt-oss-120b-medium": {}
        }
    }))
    .expect("parse response");

    let parsed = parse_fetch_available_models_response(&response);
    assert_eq!(
        parsed.default_model_id.as_deref(),
        Some("gemini-3.1-pro-high")
    );
    let parsed = parsed.models;
    assert_eq!(parsed[0].id, "claude-opus-4-6-thinking");
    assert_eq!(parsed[1].id, "gemini-3.1-pro-high");
    assert_eq!(parsed[2].id, "gpt-oss-120b-medium");
    assert_eq!(
        parsed[0].display_name.as_deref(),
        Some("Claude Opus 4.6 (Thinking)")
    );
    assert_eq!(parsed[1].remaining_fraction_milli, Some(250));
    let flash = parsed
        .iter()
        .find(|model| model.id == "gemini-3-flash")
        .expect("gemini flash model");
    assert!(!flash.available);
    assert_eq!(flash.remaining_fraction_milli, Some(0));
}

#[test]
fn client_metadata_uses_backend_accepted_platform() {
    assert_eq!(metadata_platform(), "PLATFORM_UNSPECIFIED");
    assert!(client_metadata_header().contains("\"platform\":\"PLATFORM_UNSPECIFIED\""));
}

#[test]
fn available_models_display_includes_dynamic_cache_and_current_override() {
    let provider = AntigravityProvider::new();
    *provider.fetched_catalog.write().expect("catalog lock") = vec![
        CatalogModel {
            id: "claude-opus-4-6-thinking".to_string(),
            display_name: None,
            reset_time: None,
            tag_title: None,
            model_provider: None,
            max_tokens: None,
            max_output_tokens: None,
            recommended: true,
            available: true,
            remaining_fraction_milli: Some(1000),
        },
        CatalogModel {
            id: "gemini-3-pro-high".to_string(),
            display_name: None,
            reset_time: None,
            tag_title: None,
            model_provider: None,
            max_tokens: None,
            max_output_tokens: None,
            recommended: false,
            available: true,
            remaining_fraction_milli: Some(1000),
        },
    ];
    provider
        .set_model("custom-antigravity-model")
        .expect("set custom model");

    let models = provider.available_models_display();

    assert!(models.contains(&"claude-opus-4-6-thinking".to_string()));
    assert!(models.contains(&"gemini-3-pro-high".to_string()));
    assert!(models.contains(&"custom-antigravity-model".to_string()));
}

#[test]
fn available_models_display_seeds_from_persisted_catalog() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::TempDir::new().expect("temp dir");
    let previous = std::env::var_os("JCODE_HOME");
    crate::env::set_var("JCODE_HOME", temp.path());

    let path = AntigravityProvider::persisted_catalog_path().expect("catalog path");
    crate::storage::write_json(
        &path,
        &PersistedCatalog {
            models: vec![CatalogModel {
                id: "claude-opus-4-6-thinking".to_string(),
                display_name: Some("Claude Opus 4.6 (Thinking)".to_string()),
                reset_time: None,
                tag_title: None,
                model_provider: None,
                max_tokens: None,
                max_output_tokens: None,
                recommended: true,
                available: true,
                remaining_fraction_milli: Some(1000),
            }],
            fetched_at_rfc3339: Utc::now().to_rfc3339(),
            default_model_id: Some("gemini-3-flash".to_string()),
        },
    )
    .expect("write persisted catalog");

    let provider = AntigravityProvider::new();
    assert!(
        provider
            .available_models_display()
            .contains(&"claude-opus-4-6-thinking".to_string())
    );
    assert_eq!(
        provider.backend_default_model().as_deref(),
        Some("gemini-3-flash")
    );

    if let Some(previous) = previous {
        crate::env::set_var("JCODE_HOME", previous);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
}

#[test]
fn catalog_detail_mentions_quota_and_reset() {
    let detail = catalog_model_detail(&CatalogModel {
        id: "claude-opus-4-6-thinking".to_string(),
        display_name: Some("Claude Opus 4.6 (Thinking)".to_string()),
        reset_time: Some("2026-04-24T20:53:26Z".to_string()),
        tag_title: Some("New".to_string()),
        model_provider: Some("MODEL_PROVIDER_ANTHROPIC".to_string()),
        max_tokens: Some(250_000),
        max_output_tokens: Some(64_000),
        recommended: true,
        available: true,
        remaining_fraction_milli: Some(1000),
    });

    assert!(detail.contains("recommended"));
    assert!(detail.contains("quota 100.0%"));
    assert!(detail.contains("resets 2026-04-24T20:53:26Z"));
}

#[test]
fn catalog_stale_handles_invalid_timestamp() {
    assert!(catalog_is_stale("not-a-time"));
}

#[test]
fn resolve_model_for_request_maps_default_alias_to_real_model() {
    let provider = AntigravityProvider::new();

    // With no backend default and no catalog, the alias resolves to the
    // hardcoded fallback rather than the literal "default" (which 404s).
    *provider
        .backend_default_model
        .write()
        .expect("default lock") = None;
    *provider.fetched_catalog.write().expect("catalog lock") = Vec::new();
    assert_eq!(
        provider.resolve_model_for_request("default"),
        DEFAULT_FALLBACK_MODEL
    );
    assert_eq!(
        provider.resolve_model_for_request("  "),
        DEFAULT_FALLBACK_MODEL
    );

    // A backend-advertised default takes precedence over the fallback.
    *provider
        .backend_default_model
        .write()
        .expect("default lock") = Some("gemini-3.5-flash-low".to_string());
    assert_eq!(
        provider.resolve_model_for_request("default"),
        "gemini-3.5-flash-low"
    );

    // Explicit model ids are always passed through untouched.
    assert_eq!(
        provider.resolve_model_for_request("claude-sonnet-4-6"),
        "claude-sonnet-4-6"
    );
}

#[test]
fn resolve_model_for_request_default_prefers_gemini_catalog_model() {
    let provider = AntigravityProvider::new();
    *provider
        .backend_default_model
        .write()
        .expect("default lock") = None;
    *provider.fetched_catalog.write().expect("catalog lock") = vec![
        CatalogModel {
            id: "claude-opus-4-6-thinking".to_string(),
            display_name: None,
            reset_time: None,
            tag_title: None,
            model_provider: None,
            max_tokens: None,
            max_output_tokens: None,
            recommended: true,
            available: true,
            remaining_fraction_milli: Some(1000),
        },
        CatalogModel {
            id: "gemini-3-flash".to_string(),
            display_name: None,
            reset_time: None,
            tag_title: None,
            model_provider: None,
            max_tokens: None,
            max_output_tokens: None,
            recommended: false,
            available: true,
            remaining_fraction_milli: Some(1000),
        },
    ];

    // Even though Claude is listed first (and recommended), the default alias
    // resolves to the Gemini model, which works reliably with tool use on the
    // Cloud Code backend. Claude on this backend rejects jcode's tool schemas.
    assert_eq!(
        provider.resolve_model_for_request("default"),
        "gemini-3-flash"
    );
}

#[test]
fn resolve_model_for_request_default_falls_back_to_any_catalog_model_without_gemini() {
    let provider = AntigravityProvider::new();
    *provider
        .backend_default_model
        .write()
        .expect("default lock") = None;
    *provider.fetched_catalog.write().expect("catalog lock") = vec![CatalogModel {
        id: "claude-opus-4-6-thinking".to_string(),
        display_name: None,
        reset_time: None,
        tag_title: None,
        model_provider: None,
        max_tokens: None,
        max_output_tokens: None,
        recommended: true,
        available: true,
        remaining_fraction_milli: Some(1000),
    }];

    // With no Gemini model available, fall back to the first available catalog
    // model rather than the hardcoded default.
    assert_eq!(
        provider.resolve_model_for_request("default"),
        "claude-opus-4-6-thinking"
    );
}

#[tokio::test]
async fn complete_uses_native_https_transport_not_cli_subprocess() {
    let provider = AntigravityProvider::new();
    let mut stream = provider
        .complete(&[], &[], "say hello", None)
        .await
        .expect("create stream");

    let first_event = stream
        .next()
        .await
        .expect("first event")
        .expect("connection event");

    match first_event {
        StreamEvent::ConnectionType { connection } => {
            assert_eq!(connection, "https");
            assert_ne!(connection, "cli subprocess");
        }
        other => panic!("expected connection type, got {other:?}"),
    }
}
