use crate::providers::traits::{
    ChatMessage, ChatRequest, ChatResponse, Provider, TokenUsage, ToolCall,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// How the Anthropic credential should be sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnthropicAuthKind {
    /// Standard API key: `x-api-key: <token>`
    ApiKey,
    /// OAuth / setup token: `Authorization: Bearer <token>` + `anthropic-beta: oauth-2025-04-20`
    /// Required for tokens starting with `sk-ant-oat01-`.
    OAuthBearer,
}

/// Auto-detect the right auth kind from the token prefix.
///
/// - `sk-ant-oat01-...` → OAuth Bearer (requires the beta header too)
/// - `sk-ant-...`       → standard API key
/// - JWT (2+ dots)      → OAuth Bearer
/// - Anything else      → API key (safe default)
pub fn detect_auth_kind(token: &str) -> AnthropicAuthKind {
    let t = token.trim();
    // OAuth setup tokens — need Bearer + beta header
    if t.starts_with("sk-ant-oat01-") {
        return AnthropicAuthKind::OAuthBearer;
    }
    // Other Anthropic API keys (sk-ant-api03-...) — always x-api-key
    if t.starts_with("sk-ant-") {
        return AnthropicAuthKind::ApiKey;
    }
    // JWT-shaped token → treat as OAuth Bearer
    if t.matches('.').count() >= 2 {
        return AnthropicAuthKind::OAuthBearer;
    }
    AnthropicAuthKind::ApiKey
}

pub struct ClaudeProvider {
    api_key: String,
    auth_kind: AnthropicAuthKind,
    client: Client,
}

impl ClaudeProvider {
    /// Create a provider — auth kind is auto-detected from the token shape.
    pub fn new(api_key: impl Into<String>) -> Self {
        let key = api_key.into();
        let auth_kind = detect_auth_kind(&key);
        Self { api_key: key, auth_kind, client: crate::providers::default_http_client() }
    }

    /// Create a provider with an explicit auth kind (overrides auto-detection).
    #[allow(dead_code)]
    pub fn with_auth_kind(api_key: impl Into<String>, auth_kind: AnthropicAuthKind) -> Self {
        Self { api_key: api_key.into(), auth_kind, client: crate::providers::default_http_client() }
    }
}

// ─── Anthropic request / response types ──────────────────────────────────────

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    temperature: f64,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: serde_json::Value,
}

#[derive(Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
    #[allow(dead_code)]
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

// ─── Message conversion ───────────────────────────────────────────────────────

fn convert_messages(messages: &[ChatMessage]) -> (Option<String>, Vec<AnthropicMessage>) {
    let mut system_text = None;
    let mut anthropic_msgs = Vec::new();

    for msg in messages {
        if msg.role == "user" && msg.content.starts_with("buzzclaw_image:") {
            let json = msg.content.trim_start_matches("buzzclaw_image:");
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(json) {
                if let (Some(media_type), Some(data)) = (
                    val.get("media_type").and_then(|v| v.as_str()),
                    val.get("data").and_then(|v| v.as_str()),
                ) {
                    let caption = val.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    let mut parts = Vec::new();
                    if !caption.is_empty() {
                        parts.push(serde_json::json!({"type":"text","text": caption}));
                    }
                    parts.push(serde_json::json!({
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": media_type,
                            "data": data
                        }
                    }));
                    anthropic_msgs.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: serde_json::Value::Array(parts),
                    });
                    continue;
                }
            }
        }
        match msg.role.as_str() {
            "system" => {
                system_text = Some(msg.content.clone());
            }
            "tool" => {
                // Decode our packed tool_result JSON
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg.content) {
                    let tool_call_id = val
                        .get("tool_call_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let content = val
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    anthropic_msgs.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: serde_json::json!([{
                            "type": "tool_result",
                            "tool_use_id": tool_call_id,
                            "content": content,
                        }]),
                    });
                }
            }
            "assistant" => {
                // Assistant messages may contain embedded tool_calls JSON
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg.content) {
                    if let Some(calls_val) = val.get("tool_calls") {
                        // Reconstruct Anthropic content array
                        let text = val
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let mut parts: Vec<serde_json::Value> = Vec::new();
                        if !text.is_empty() {
                            parts.push(serde_json::json!({"type": "text", "text": text}));
                        }
                        
                        if let Some(arr) = calls_val.as_array() {
                            for c in arr {
                                let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                
                                // Try packed format {"function":{"name","arguments"}}
                                let (name, args_str) = if let Some(f) = c.get("function") {
                                    (
                                        f.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                        f.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}").to_string()
                                    )
                                } else {
                                    // Fallback to simple format {"name","arguments"}
                                    (
                                        c.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                        c.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}").to_string()
                                    )
                                };

                                if !name.is_empty() {
                                    let args_val: serde_json::Value = serde_json::from_str(&args_str)
                                        .unwrap_or(serde_json::Value::Object(Default::default()));
                                    parts.push(serde_json::json!({
                                        "type": "tool_use",
                                        "id": id,
                                        "name": name,
                                        "input": args_val,
                                    }));
                                }
                            }
                        }
                        
                        anthropic_msgs.push(AnthropicMessage {
                            role: "assistant".to_string(),
                            content: serde_json::Value::Array(parts),
                        });
                        continue;
                    }
                }
                anthropic_msgs.push(AnthropicMessage {
                    role: "assistant".to_string(),
                    content: serde_json::Value::String(msg.content.clone()),
                });
            }
            _ => {
                anthropic_msgs.push(AnthropicMessage {
                    role: msg.role.clone(),
                    content: serde_json::Value::String(msg.content.clone()),
                });
            }
        }
    }

    (system_text, anthropic_msgs)
}

// ─── Provider impl ────────────────────────────────────────────────────────────

#[async_trait]
impl Provider for ClaudeProvider {
    fn name(&self) -> &str {
        "claude"
    }

    fn supports_tools(&self) -> bool {
        true
    }

    async fn chat(
        &self,
        req: ChatRequest<'_>,
        model: &str,
        temperature: f64,
    ) -> anyhow::Result<ChatResponse> {
        let (system, messages) = convert_messages(req.messages);

        let tools = req.tools.map(|specs| {
            specs
                .iter()
                .map(|s| AnthropicTool {
                    name: s.name.clone(),
                    description: s.description.clone(),
                    input_schema: s.parameters.clone(),
                })
                .collect::<Vec<_>>()
        });

        let body = AnthropicRequest {
            model,
            max_tokens: 8192,
            temperature,
            messages,
            system: system.as_deref(),
            tools,
        };

        let mut req = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json");

        req = match self.auth_kind {
            AnthropicAuthKind::ApiKey => req.header("x-api-key", &self.api_key),
            AnthropicAuthKind::OAuthBearer => req
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("anthropic-beta", "oauth-2025-04-20"),
        };

        let resp = req.json(&body).send().await?;

        if !resp.status().is_success() {
            return Err(crate::providers::api_error("Claude", resp).await);
        }

        let raw = crate::providers::parse_json::<AnthropicResponse>("Claude", resp).await?;

        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for block in raw.content {
            match block {
                ContentBlock::Text { text } => text_parts.push(text),
                ContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall {
                        id,
                        name,
                        arguments: input.to_string(),
                    });
                }
            }
        }

        let text = if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join(""))
        };

        let usage = raw.usage.map(|u| TokenUsage {
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
        });

        Ok(ChatResponse { text, tool_calls, usage })
    }
}
