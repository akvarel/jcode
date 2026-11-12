use serde_json::Value;

/// Outcome of a persistent WebSocket continuation attempt.
pub(crate) enum PersistentWsResult {
    Success,
    /// A terminal API error was forwarded to the consumer. Do not replay it.
    TerminalError,
    /// Retry the full history over HTTP, not the rejected response chain.
    FallbackToHttps(String),
    NotAvailable,
    Failed(String),
}

impl PersistentWsResult {
    pub(crate) fn log(&self, model: &str) {
        use jcode_base::logging::LogLevel;
        let (level, phase, extra) = match self {
            Self::Success => (LogLevel::Info, "persistent_reuse_success", None),
            Self::NotAvailable => (LogLevel::Info, "persistent_reuse_unavailable", None),
            Self::Failed(error) => (
                LogLevel::Warn,
                "persistent_reuse_failed",
                Some(("error", error.clone())),
            ),
            Self::FallbackToHttps(_) => (
                LogLevel::Warn,
                "persistent_reuse_fallback_to_https",
                Some(("reason", "unaccepted_invalid_request".to_string())),
            ),
            Self::TerminalError => return,
        };
        let mut fields = vec![
            ("model", model.to_string()),
            ("transport", "websocket".to_string()),
        ];
        fields.extend(extra);
        super::log_openai_stream_lifecycle(level, phase, fields);
    }
}

/// Recover only the opaque, pre-acceptance WS rejection seen in the incident.
/// Specific validation/auth/quota errors and failures after response activity
/// remain terminal. Arbitrary 400s are not retryable.
pub(super) fn is_unaccepted_ws_invalid_request(text: &str, response_started: bool) -> bool {
    if response_started {
        return false;
    }
    let Ok(event) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    let error = &event["error"];
    event["type"] == "error"
        && event.get("response").is_none_or(Value::is_null)
        && event.get("response_id").is_none_or(Value::is_null)
        && error["type"] == "invalid_request_error"
        && error["message"] == "Invalid request"
        && error.get("code").is_none_or(Value::is_null)
        && error.get("param").is_none_or(Value::is_null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_invalid_request_recovery_is_narrow() {
        let bare = serde_json::json!({"type":"error","error":{
            "type":"invalid_request_error","message":"Invalid request"
        }});
        assert!(is_unaccepted_ws_invalid_request(&bare.to_string(), false));
        assert!(!is_unaccepted_ws_invalid_request(&bare.to_string(), true));
        assert!(!is_unaccepted_ws_invalid_request("not json", false));
        for (field, value) in [
            ("code", "invalid_api_key"),
            ("code", "invalid_json_schema"),
            ("param", "tools"),
            ("message", "Invalid tool schema."),
            ("type", "usage_limit_reached"),
            ("type", "authentication_error"),
        ] {
            let mut specific = bare.clone();
            specific["error"][field] = serde_json::json!(value);
            assert!(
                !is_unaccepted_ws_invalid_request(&specific.to_string(), false),
                "{specific}"
            );
        }
        let mut failed = bare.clone();
        failed["type"] = serde_json::json!("response.failed");
        assert!(!is_unaccepted_ws_invalid_request(
            &failed.to_string(),
            false
        ));
        let mut associated = bare;
        associated["response"] = serde_json::json!({"id":"resp_accepted"});
        assert!(!is_unaccepted_ws_invalid_request(
            &associated.to_string(),
            false
        ));
        associated.as_object_mut().unwrap().remove("response");
        associated["response_id"] = serde_json::json!("resp_accepted");
        assert!(!is_unaccepted_ws_invalid_request(
            &associated.to_string(),
            false
        ));
    }
}
