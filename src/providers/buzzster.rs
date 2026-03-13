use crate::providers::traits::{
    ChatMessage, ChatRequest, ChatResponse, Provider, TokenUsage, ToolCall,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const BUZZSTER_BASE: &str = "https://api.buzzster.xyz/v1";

pub struct BuzzsterProvider {
    api_key: String,
    base_url: String,
    client: Client,
}

impl BuzzsterProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: BUZZSTER_BASE.to_string(),
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

// ─── Buzzster OpenAI-compatible types ────────────────────────────────────────

#[derive(Serialize)]
struct BuzzRequest {
    model: String,
    messages: Vec<BuzzMessage>,
    temperature: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<BuzzTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
}

#[derive(Serialize)]
struct BuzzMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<BuzzToolCallOut>>,
}

#[derive(Serialize)]
struct BuzzTool {
    #[serde(rename = "type")]
    kind: String,
    function: BuzzFunctionSpec,
}

#[derive(Serialize)]
struct BuzzFunctionSpec {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct BuzzToolCallOut {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    function: BuzzFunctionCall,
}

#[derive(Serialize, Deserialize)]
struct BuzzFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct BuzzResponse {
    choices: Vec<BuzzChoice>,
    #[serde(default)]
    usage: Option<BuzzUsage>,
}

#[derive(Deserialize)]
struct BuzzChoice {
    message: BuzzResponseMessage,
}

#[derive(Deserialize)]
struct BuzzResponseMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<BuzzToolCallOut>>,
}

impl BuzzResponseMessage {
    fn effective_text(&self) -> Option<String> {
        match &self.content {
            Some(c) if !c.is_empty() => Some(c.clone()),
            _ => self.reasoning_content.clone(),
        }
    }
}

#[derive(Deserialize)]
struct BuzzUsage {
    #[serde(default)]
    prompt_tokens: Option<u64>,
    #[serde(default)]
    completion_tokens: Option<u64>,
}

// ─── Message conversion ───────────────────────────────────────────────────────

fn to_buzz_messages(messages: &[ChatMessage]) -> Vec<BuzzMessage> {
    let mut out: Vec<BuzzMessage> = Vec::new();
    let mut valid_tool_call_ids = std::collections::HashSet::new();

    for m in messages {
        // Unpack assistant messages that carry serialized tool_calls
        if m.role == "assistant" {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&m.content) {
                if let Some(calls_val) = val.get("tool_calls") {
                    let content = val
                        .get("content")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let mut calls: Vec<BuzzToolCallOut> = Vec::new();
                    if let Some(arr) = calls_val.as_array() {
                        for c in arr {
                            if let Ok(call) = serde_json::from_value::<BuzzToolCallOut>(c.clone()) {
                                calls.push(call);
                            } else {
                                // Fallback to simple format
                                let id = c.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                                let name = c.get("name").and_then(|v| v.as_str());
                                let args = c.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
                                if let Some(n) = name {
                                    calls.push(BuzzToolCallOut {
                                        id,
                                        kind: Some("function".to_string()),
                                        function: BuzzFunctionCall {
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
                        out.push(BuzzMessage {
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
                    out.push(BuzzMessage {
                        role: "tool".to_string(),
                        content: val
                            .get("content")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        tool_call_id: Some(tool_call_id.to_string()),
                        tool_calls: None,
                    });
                    continue;
                }
            }
        }

        out.push(BuzzMessage {
            role: m.role.clone(),
            content: Some(m.content.clone()),
            tool_call_id: None,
            tool_calls: None,
        });
    }

    out
}

// ─── Provider impl ────────────────────────────────────────────────────────────

#[async_trait]
impl Provider for BuzzsterProvider {
    fn name(&self) -> &str {
        "buzzster"
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
        let tools: Option<Vec<BuzzTool>> = req.tools.map(|specs| {
            specs
                .iter()
                .map(|s| BuzzTool {
                    kind: "function".to_string(),
                    function: BuzzFunctionSpec {
                        name: s.name.clone(),
                        description: s.description.clone(),
                        parameters: s.parameters.clone(),
                    },
                })
                .collect()
        });

        let body = BuzzRequest {
            model: model.to_string(),
            messages: to_buzz_messages(req.messages),
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
            return Err(crate::providers::api_error("Buzzster", resp).await);
        }

        let raw = crate::providers::parse_json::<BuzzResponse>("Buzzster", resp).await?;
        let message = raw
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Buzzster returned no choices"))?
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
