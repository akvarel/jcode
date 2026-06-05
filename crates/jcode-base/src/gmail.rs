use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::auth::google;

const GMAIL_API_BASE: &str = "https://gmail.googleapis.com/gmail/v1/users/me";
const COMPOSIO_DEFAULT_BASE: &str = "https://backend.composio.dev/api/v3.1";

/// Where the Gmail tool gets its credentials and authenticated transport.
///
/// `Direct` talks to the Google Gmail REST API using locally stored OAuth
/// tokens (the original behavior). `Composio` routes the *same* Gmail REST
/// calls through Composio's managed `proxy-execute` endpoint, so a
/// Google-verified app brokers auth: no unverified-app warning and no 7-day
/// testing-mode token expiry.
#[derive(Debug, Clone)]
pub enum GmailBackend {
    Direct,
    Composio(ComposioConfig),
}

#[derive(Debug, Clone)]
pub struct ComposioConfig {
    pub api_key: String,
    pub base_url: String,
    pub connected_account_id: Option<String>,
    pub user_id: Option<String>,
}

impl GmailBackend {
    /// Resolve the backend from environment configuration.
    ///
    /// Defaults to `Direct`. Set `JCODE_GMAIL_BACKEND=composio` (with
    /// `COMPOSIO_API_KEY` present) to broker Gmail through Composio.
    pub fn from_env() -> Self {
        let selection = std::env::var("JCODE_GMAIL_BACKEND")
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if selection == "composio" {
            if let Some(cfg) = ComposioConfig::from_env() {
                return GmailBackend::Composio(cfg);
            }
            eprintln!(
                "JCODE_GMAIL_BACKEND=composio but COMPOSIO_API_KEY is not set; falling back to direct Gmail backend"
            );
        }
        GmailBackend::Direct
    }

    pub fn label(&self) -> &'static str {
        match self {
            GmailBackend::Direct => "direct",
            GmailBackend::Composio(_) => "composio",
        }
    }
}

impl ComposioConfig {
    fn from_env() -> Option<Self> {
        let api_key = std::env::var("COMPOSIO_API_KEY").ok().filter(|s| !s.is_empty())?;
        let base_url = std::env::var("COMPOSIO_BASE_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| COMPOSIO_DEFAULT_BASE.to_string());
        let connected_account_id = std::env::var("COMPOSIO_GMAIL_CONNECTED_ACCOUNT_ID")
            .ok()
            .filter(|s| !s.is_empty());
        let user_id = std::env::var("COMPOSIO_GMAIL_USER_ID")
            .or_else(|_| std::env::var("COMPOSIO_USER_ID"))
            .ok()
            .filter(|s| !s.is_empty());
        Some(Self {
            api_key,
            base_url,
            connected_account_id,
            user_id,
        })
    }
}

pub struct GmailClient {
    http: reqwest::Client,
    backend: GmailBackend,
}

impl Default for GmailClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GmailClient {
    pub fn new() -> Self {
        Self::with_backend(GmailBackend::from_env())
    }

    pub fn with_backend(backend: GmailBackend) -> Self {
        Self {
            http: crate::provider::shared_http_client(),
            backend,
        }
    }

    pub fn backend_label(&self) -> &'static str {
        self.backend.label()
    }

    /// Whether this backend has credentials available to talk to Gmail.
    pub fn is_configured(&self) -> bool {
        match &self.backend {
            GmailBackend::Direct => google::has_tokens(),
            GmailBackend::Composio(cfg) => !cfg.api_key.is_empty(),
        }
    }

    /// Whether the current backend is allowed to send mail.
    ///
    /// The `Direct` backend honors the locally configured access tier
    /// (read-only logins cannot send). Composio connections request full
    /// Gmail scopes, so sending is available.
    pub fn can_send(&self) -> bool {
        match &self.backend {
            GmailBackend::Direct => google::load_tokens()
                .map(|t| t.tier.can_send())
                .unwrap_or(false),
            GmailBackend::Composio(_) => true,
        }
    }

    /// Whether the current backend is allowed to delete/trash mail.
    pub fn can_delete(&self) -> bool {
        match &self.backend {
            GmailBackend::Direct => google::load_tokens()
                .map(|t| t.tier.can_delete())
                .unwrap_or(false),
            GmailBackend::Composio(_) => true,
        }
    }

    pub fn not_configured_message(&self) -> &'static str {
        match &self.backend {
            GmailBackend::Direct => {
                "Gmail is not configured. Run `jcode login google` to set up Gmail access."
            }
            GmailBackend::Composio(_) => {
                "Gmail (Composio backend) is not configured. Set COMPOSIO_API_KEY and connect your \
                 Gmail account in Composio, then retry."
            }
        }
    }

    /// Send an authenticated Gmail REST request and return the parsed JSON
    /// response. Both backends produce the identical Gmail API JSON shape, so
    /// callers can deserialize into the same typed structs.
    async fn request(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        match &self.backend {
            GmailBackend::Direct => self.request_direct(method, url, body).await,
            GmailBackend::Composio(cfg) => self.request_composio(cfg, method, url, body).await,
        }
    }

    async fn request_direct(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        let token = google::get_valid_token().await?;
        let mut req = self.http.request(method, url).bearer_auth(&token);
        if let Some(ref b) = body {
            req = req.json(b);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "Gmail API error {}: {}",
                status,
                truncate_error(&text)
            ));
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_str(&text)?)
    }

    async fn request_composio(
        &self,
        cfg: &ComposioConfig,
        method: reqwest::Method,
        url: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        let payload = build_composio_proxy_payload(cfg, method.as_str(), url, body);
        let endpoint = format!("{}/tools/execute/proxy", cfg.base_url.trim_end_matches('/'));
        let resp = self
            .http
            .post(&endpoint)
            .header("x-api-key", &cfg.api_key)
            .json(&payload)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "Composio proxy error {}: {}",
                status,
                truncate_error(&text)
            ));
        }
        let envelope: Value = serde_json::from_str(&text)?;
        // Composio wraps the upstream response as { data, status, headers }.
        if let Some(inner) = envelope.get("status").and_then(|s| s.as_u64()) {
            if inner >= 400 {
                return Err(anyhow::anyhow!(
                    "Gmail API error {} (via Composio): {}",
                    inner,
                    truncate_error(&envelope.get("data").map(|d| d.to_string()).unwrap_or_default())
                ));
            }
        }
        if let Some(err) = envelope.get("error").filter(|e| !e.is_null()) {
            return Err(anyhow::anyhow!("Composio error: {}", truncate_error(&err.to_string())));
        }
        Ok(envelope.get("data").cloned().unwrap_or(Value::Null))
    }

    pub async fn list_messages(
        &self,
        query: Option<&str>,
        label_ids: Option<&[&str]>,
        max_results: u32,
    ) -> Result<MessageList> {
        let mut url = format!("{}/messages?maxResults={}", GMAIL_API_BASE, max_results);

        if let Some(q) = query {
            url.push_str(&format!("&q={}", urlencoding::encode(q)));
        }
        if let Some(labels) = label_ids {
            for label in labels {
                url.push_str(&format!("&labelIds={}", label));
            }
        }

        let value = self.request(reqwest::Method::GET, &url, None).await?;
        Ok(serde_json::from_value(value)?)
    }

    pub async fn get_message(&self, id: &str, format: MessageFormat) -> Result<Message> {
        let url = format!(
            "{}/messages/{}?format={}",
            GMAIL_API_BASE,
            id,
            format.as_str()
        );
        let value = self.request(reqwest::Method::GET, &url, None).await?;
        Ok(serde_json::from_value(value)?)
    }

    pub async fn list_threads(&self, query: Option<&str>, max_results: u32) -> Result<ThreadList> {
        let mut url = format!("{}/threads?maxResults={}", GMAIL_API_BASE, max_results);

        if let Some(q) = query {
            url.push_str(&format!("&q={}", urlencoding::encode(q)));
        }

        let value = self.request(reqwest::Method::GET, &url, None).await?;
        Ok(serde_json::from_value(value)?)
    }

    pub async fn get_thread(&self, id: &str) -> Result<Thread> {
        let url = format!("{}/threads/{}?format=metadata", GMAIL_API_BASE, id);
        let value = self.request(reqwest::Method::GET, &url, None).await?;
        Ok(serde_json::from_value(value)?)
    }

    pub async fn list_labels(&self) -> Result<Vec<Label>> {
        let url = format!("{}/labels", GMAIL_API_BASE);
        #[derive(Deserialize)]
        struct LabelList {
            labels: Option<Vec<Label>>,
        }

        let value = self.request(reqwest::Method::GET, &url, None).await?;
        let list: LabelList = serde_json::from_value(value)?;
        Ok(list.labels.unwrap_or_default())
    }

    pub async fn create_draft(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        in_reply_to: Option<&str>,
        thread_id: Option<&str>,
    ) -> Result<Draft> {
        let url = format!("{}/drafts", GMAIL_API_BASE);

        let mut headers = format!(
            "To: {}\r\nSubject: {}\r\nContent-Type: text/plain; charset=utf-8\r\n",
            to, subject
        );
        if let Some(reply_to) = in_reply_to {
            headers.push_str(&format!(
                "In-Reply-To: {}\r\nReferences: {}\r\n",
                reply_to, reply_to
            ));
        }

        let raw = format!("{}\r\n{}", headers, body);
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw.as_bytes());

        let mut message = json!({ "raw": encoded });
        if let Some(tid) = thread_id {
            message["threadId"] = Value::String(tid.to_string());
        }

        let payload = json!({ "message": message });

        let value = self
            .request(reqwest::Method::POST, &url, Some(payload))
            .await?;
        Ok(serde_json::from_value(value)?)
    }

    pub async fn send_draft(&self, draft_id: &str) -> Result<Message> {
        let url = format!("{}/drafts/send", GMAIL_API_BASE);
        let payload = json!({ "id": draft_id });

        let value = self
            .request(reqwest::Method::POST, &url, Some(payload))
            .await?;
        Ok(serde_json::from_value(value)?)
    }

    pub async fn send_message(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        in_reply_to: Option<&str>,
        thread_id: Option<&str>,
    ) -> Result<Message> {
        let url = format!("{}/messages/send", GMAIL_API_BASE);

        let mut headers = format!(
            "To: {}\r\nSubject: {}\r\nContent-Type: text/plain; charset=utf-8\r\n",
            to, subject
        );
        if let Some(reply_to) = in_reply_to {
            headers.push_str(&format!(
                "In-Reply-To: {}\r\nReferences: {}\r\n",
                reply_to, reply_to
            ));
        }

        let raw = format!("{}\r\n{}", headers, body);
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw.as_bytes());

        let mut message = json!({ "raw": encoded });
        if let Some(tid) = thread_id {
            message["threadId"] = Value::String(tid.to_string());
        }

        let value = self
            .request(reqwest::Method::POST, &url, Some(message))
            .await?;
        Ok(serde_json::from_value(value)?)
    }

    pub async fn trash_message(&self, id: &str) -> Result<()> {
        let url = format!("{}/messages/{}/trash", GMAIL_API_BASE, id);
        self.request(reqwest::Method::POST, &url, None).await?;
        Ok(())
    }

    pub async fn modify_labels(
        &self,
        id: &str,
        add_labels: &[&str],
        remove_labels: &[&str],
    ) -> Result<()> {
        let url = format!("{}/messages/{}/modify", GMAIL_API_BASE, id);
        let payload = json!({
            "addLabelIds": add_labels,
            "removeLabelIds": remove_labels,
        });
        self.request(reqwest::Method::POST, &url, Some(payload))
            .await?;
        Ok(())
    }
}

/// Build the request body for Composio's `proxy-execute` endpoint, which makes
/// an authenticated HTTP call to the connected toolkit (Gmail) on our behalf.
fn build_composio_proxy_payload(
    cfg: &ComposioConfig,
    method: &str,
    url: &str,
    body: Option<Value>,
) -> Value {
    let mut payload = json!({
        "endpoint": url,
        "method": method,
    });
    if let Some(b) = body {
        payload["body"] = b;
    }
    if let Some(account) = &cfg.connected_account_id {
        payload["connected_account_id"] = Value::String(account.clone());
    }
    if let Some(user) = &cfg.user_id {
        payload["user_id"] = Value::String(user.clone());
    }
    payload
}

fn truncate_error(text: &str) -> String {
    const MAX: usize = 400;
    let trimmed = text.trim();
    if trimmed.len() <= MAX {
        trimmed.to_string()
    } else {
        format!("{}…", &trimmed[..MAX])
    }
}

use base64::Engine;

#[derive(Debug, Clone, Copy)]
pub enum MessageFormat {
    Full,
    Metadata,
}

impl MessageFormat {
    fn as_str(&self) -> &'static str {
        match self {
            MessageFormat::Full => "full",
            MessageFormat::Metadata => "metadata",
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MessageList {
    pub messages: Option<Vec<MessageRef>>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
    #[serde(rename = "resultSizeEstimate")]
    pub result_size_estimate: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MessageRef {
    pub id: String,
    #[serde(rename = "threadId")]
    pub thread_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Message {
    pub id: String,
    #[serde(rename = "threadId")]
    pub thread_id: Option<String>,
    #[serde(rename = "labelIds")]
    pub label_ids: Option<Vec<String>>,
    pub snippet: Option<String>,
    pub payload: Option<MessagePayload>,
    #[serde(rename = "internalDate")]
    pub internal_date: Option<String>,
    #[serde(rename = "sizeEstimate")]
    pub size_estimate: Option<u32>,
}

impl Message {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.payload.as_ref().and_then(|p| {
            p.headers.as_ref().and_then(|headers| {
                headers
                    .iter()
                    .find(|h| h.name.eq_ignore_ascii_case(name))
                    .map(|h| h.value.as_str())
            })
        })
    }

    pub fn subject(&self) -> Option<&str> {
        self.header("Subject")
    }

    pub fn from(&self) -> Option<&str> {
        self.header("From")
    }

    pub fn date(&self) -> Option<&str> {
        self.header("Date")
    }

    pub fn body_text(&self) -> Option<String> {
        self.payload.as_ref().and_then(|p| p.extract_text())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MessagePayload {
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub headers: Option<Vec<Header>>,
    pub body: Option<MessageBody>,
    pub parts: Option<Vec<MessagePayload>>,
}

impl MessagePayload {
    #[expect(
        clippy::collapsible_if,
        reason = "Nested MIME/body decoding is kept explicit for readability"
    )]
    fn extract_text(&self) -> Option<String> {
        if let Some(ref mime) = self.mime_type {
            if mime == "text/plain" {
                if let Some(ref body) = self.body {
                    if let Some(ref data) = body.data {
                        if let Ok(bytes) =
                            base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(data)
                        {
                            return String::from_utf8(bytes).ok();
                        }
                        if let Ok(bytes) = base64::engine::general_purpose::URL_SAFE.decode(data) {
                            return String::from_utf8(bytes).ok();
                        }
                    }
                }
            }
        }

        if let Some(ref parts) = self.parts {
            for part in parts {
                if let Some(text) = part.extract_text() {
                    return Some(text);
                }
            }
        }

        None
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MessageBody {
    pub size: Option<u32>,
    pub data: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ThreadList {
    pub threads: Option<Vec<ThreadRef>>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
    #[serde(rename = "resultSizeEstimate")]
    pub result_size_estimate: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ThreadRef {
    pub id: String,
    pub snippet: Option<String>,
    #[serde(rename = "historyId")]
    pub history_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Thread {
    pub id: String,
    pub messages: Option<Vec<Message>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Label {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub label_type: Option<String>,
    #[serde(rename = "messagesTotal")]
    pub messages_total: Option<u32>,
    #[serde(rename = "messagesUnread")]
    pub messages_unread: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Draft {
    pub id: String,
    pub message: Option<MessageRef>,
}

pub fn format_message_summary(msg: &Message) -> String {
    let from = msg.from().unwrap_or("(unknown)");
    let subject = msg.subject().unwrap_or("(no subject)");
    let date = msg.date().unwrap_or("");
    let snippet = msg.snippet.as_deref().unwrap_or("");
    let labels = msg
        .label_ids
        .as_ref()
        .map(|l| l.join(", "))
        .unwrap_or_default();

    format!(
        "From: {}\nSubject: {}\nDate: {}\nLabels: {}\nSnippet: {}\nID: {}",
        from, subject, date, labels, snippet, msg.id
    )
}

pub fn format_message_full(msg: &Message) -> String {
    let mut out = format_message_summary(msg);
    if let Some(body) = msg.body_text() {
        out.push_str("\n\n--- Body ---\n");
        out.push_str(&body);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> ComposioConfig {
        ComposioConfig {
            api_key: "test-key".to_string(),
            base_url: COMPOSIO_DEFAULT_BASE.to_string(),
            connected_account_id: Some("ca_123".to_string()),
            user_id: Some("me".to_string()),
        }
    }

    #[test]
    fn composio_proxy_payload_get_has_no_body() {
        let url = format!("{}/messages?maxResults=10", GMAIL_API_BASE);
        let payload = build_composio_proxy_payload(&cfg(), "GET", &url, None);
        assert_eq!(payload["endpoint"], url);
        assert_eq!(payload["method"], "GET");
        assert!(payload.get("body").is_none());
        assert_eq!(payload["connected_account_id"], "ca_123");
        assert_eq!(payload["user_id"], "me");
    }

    #[test]
    fn composio_proxy_payload_post_includes_body() {
        let url = format!("{}/messages/send", GMAIL_API_BASE);
        let body = json!({ "raw": "abc" });
        let payload = build_composio_proxy_payload(&cfg(), "POST", &url, Some(body.clone()));
        assert_eq!(payload["method"], "POST");
        assert_eq!(payload["body"], body);
    }

    #[test]
    fn composio_proxy_payload_omits_optional_account_fields() {
        let bare = ComposioConfig {
            api_key: "k".to_string(),
            base_url: COMPOSIO_DEFAULT_BASE.to_string(),
            connected_account_id: None,
            user_id: None,
        };
        let payload = build_composio_proxy_payload(&bare, "GET", "http://x/y", None);
        assert!(payload.get("connected_account_id").is_none());
        assert!(payload.get("user_id").is_none());
    }

    #[test]
    fn direct_backend_label_and_default() {
        let backend = GmailBackend::Direct;
        assert_eq!(backend.label(), "direct");
        let client = GmailClient::with_backend(GmailBackend::Direct);
        assert_eq!(client.backend_label(), "direct");
    }

    #[test]
    fn composio_backend_is_configured_and_can_send() {
        let client = GmailClient::with_backend(GmailBackend::Composio(cfg()));
        assert_eq!(client.backend_label(), "composio");
        assert!(client.is_configured());
        // Composio connections request full Gmail scopes.
        assert!(client.can_send());
        assert!(client.can_delete());
    }

    #[test]
    fn truncate_error_caps_length() {
        let short = truncate_error("  hi  ");
        assert_eq!(short, "hi");
        let long = "x".repeat(1000);
        let capped = truncate_error(&long);
        assert!(capped.len() <= 401 + 3); // 400 chars + ellipsis byte
        assert!(capped.ends_with('…'));
    }
}
