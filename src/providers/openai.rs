use crate::providers::traits::{
    ChatMessage, ChatRequest, ChatResponse, Provider, TokenUsage, ToolCall,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const OPENAI_BASE: &str = "https://api.openai.com/v1";

pub struct OpenAiProvider {
    api_key: String,
    base_url: String,
    client: Client,
}

impl OpenAiProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: OPENAI_BASE.to_string(),
            client: crate::providers::default_http_client(),
        }
    }

    #[allow(dead_code)]
    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            client: crate::providers::default_http_client(),
        }
    }
}

// ─── OpenAI types ────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    temperature: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCallOut>>,
}

#[derive(Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    kind: String,
    function: OpenAiFunctionSpec,
}

#[derive(Serialize)]
struct OpenAiFunctionSpec {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct OpenAiToolCallOut {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    function: OpenAiFunctionCall,
}

#[derive(Serialize, Deserialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
}

#[derive(Deserialize)]
struct OpenAiResponseMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCallOut>>,
}

impl OpenAiResponseMessage {
    fn effective_text(&self) -> Option<String> {
        match &self.content {
            Some(c) if !c.is_empty() => Some(c.clone()),
            _ => self.reasoning_content.clone(),
        }
    }
}

#[derive(Deserialize)]
struct OpenAiUsage {
    #[serde(default)]
    prompt_tokens: Option<u64>,
    #[serde(default)]
    completion_tokens: Option<u64>,
}

// ─── Message conversion ───────────────────────────────────────────────────────

fn parse_image_marker(text: &str) -> Option<(String, String, String)> {
    let prefix = "buzzclaw_image:";
    if !text.starts_with(prefix) {
        return None;
    }
    let json = &text[prefix.len()..];
    let val: serde_json::Value = serde_json::from_str(json).ok()?;
    let media_type = val.get("media_type")?.as_str()?.to_string();
    let data = val.get("data")?.as_str()?.to_string();
    let caption = val.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
    Some((media_type, data, caption))
}

fn to_openai_messages(messages: &[ChatMessage]) -> Vec<OpenAiMessage> {
    let mut out: Vec<OpenAiMessage> = Vec::new();
    let mut valid_tool_call_ids = std::collections::HashSet::new();

    for m in messages {
        if m.role == "user" {
            if let Some((media_type, data, caption)) = parse_image_marker(&m.content) {
                let mut parts = Vec::new();
                if !caption.is_empty() {
                    parts.push(serde_json::json!({"type":"text","text": caption}));
                }
                let url = format!("data:{};base64,{}", media_type, data);
                parts.push(serde_json::json!({"type":"image_url","image_url":{"url": url}}));
                out.push(OpenAiMessage {
                    role: "user".to_string(),
                    content: Some(serde_json::Value::Array(parts)),
                    tool_call_id: None,
                    tool_calls: None,
                });
                continue;
            }
        }

        // Unpack assistant messages that carry serialized tool_calls
        if m.role == "assistant" {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&m.content) {
                if let Some(calls_val) = val.get("tool_calls") {
                    let content = val
                        .get("content")
                        .and_then(|v| v.as_str())
                        .map(|s| serde_json::Value::String(s.to_string()));

                    let mut calls: Vec<OpenAiToolCallOut> = Vec::new();
                    if let Some(arr) = calls_val.as_array() {
                        for c in arr {
                            if let Ok(call) = serde_json::from_value::<OpenAiToolCallOut>(c.clone()) {
                                calls.push(call);
                            } else {
                                // Fallback to simple format
                                let id = c.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                                let name = c.get("name").and_then(|v| v.as_str());
                                let args = c.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
                                if let Some(n) = name {
                                    calls.push(OpenAiToolCallOut {
                                        id,
                                        kind: Some("function".to_string()),
                                        function: OpenAiFunctionCall {
                                            name: n.to_string(),
                                            arguments: args.to_string(),
                                        },
                                    });
                                }
                            }
                        }
                    }

                    if !calls.is_empty() {
                        for call in &calls {
                            if let Some(id) = call.id.as_deref() {
                                valid_tool_call_ids.insert(id.to_string());
                            }
                        }
                        out.push(OpenAiMessage {
                            role: "assistant".to_string(),
                            content,
                            tool_call_id: None,
                            tool_calls: Some(calls),
                        });
                        continue;
                    }
                }
            }
        }

        // Unpack tool result messages
        if m.role == "tool" {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&m.content) {
                let tool_call_id = val
                    .get("tool_call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !tool_call_id.is_empty() && valid_tool_call_ids.contains(tool_call_id) {
                    out.push(OpenAiMessage {
                        role: "tool".to_string(),
                        content: val
                            .get("content")
                            .and_then(|v| v.as_str())
                            .map(|s| serde_json::Value::String(s.to_string())),
                        tool_call_id: Some(tool_call_id.to_string()),
                        tool_calls: None,
                    });
                    continue;
                }
            }
        }

        out.push(OpenAiMessage {
            role: m.role.clone(),
            content: Some(serde_json::Value::String(m.content.clone())),
            tool_call_id: None,
            tool_calls: None,
        });
    }

    out
}

// ─── Provider impl ────────────────────────────────────────────────────────────

#[async_trait]
impl Provider for OpenAiProvider {
    fn name(&self) -> &str {
        "OpenAI"
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
        let tools: Option<Vec<OpenAiTool>> = req.tools.map(|specs| {
            specs
                .iter()
                .map(|s| OpenAiTool {
                    kind: "function".to_string(),
                    function: OpenAiFunctionSpec {
                        name: s.name.clone(),
                        description: s.description.clone(),
                        parameters: s.parameters.clone(),
                    },
                })
                .collect()
        });

        let body = OpenAiRequest {
            model: model.to_string(),
            messages: to_openai_messages(req.messages),
            temperature,
            tool_choice: tools.as_ref().map(|_| "auto".to_string()),
            tools,
        };

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(crate::providers::api_error("OpenAI", resp).await);
        }

        let raw = crate::providers::parse_json::<OpenAiResponse>("OpenAI", resp).await?;
        let message = raw
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("OpenAI returned no choices"))?
            .message;

        let text = message.effective_text();
        let tool_calls = message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tc| ToolCall {
                id: tc.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                name: tc.function.name,
                arguments: tc.function.arguments,
            })
            .collect();

        let usage = raw.usage.map(|u| TokenUsage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
        });

        Ok(ChatResponse { text, tool_calls, usage })
    }
}
