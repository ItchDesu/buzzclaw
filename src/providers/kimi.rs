use crate::providers::traits::{
    ChatMessage, ChatRequest, ChatResponse, Provider, TokenUsage, ToolCall,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const KIMI_BASE: &str = "https://api.moonshot.ai/v1";

pub struct KimiProvider {
    api_key: String,
    base_url: String,
    client: Client,
}

impl KimiProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: KIMI_BASE.to_string(),
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

// ─── Kimi (Moonshot) types ───────────────────────────────────────────────────

#[derive(Serialize)]
struct KimiRequest {
    model: String,
    messages: Vec<KimiMessage>,
    temperature: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<KimiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
}

#[derive(Serialize)]
struct KimiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<KimiToolCallOut>>,
}

#[derive(Serialize)]
struct KimiTool {
    #[serde(rename = "type")]
    kind: String,
    function: KimiFunctionSpec,
}

#[derive(Serialize)]
struct KimiFunctionSpec {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct KimiToolCallOut {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    function: KimiFunctionCall,
}

#[derive(Serialize, Deserialize)]
struct KimiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct KimiResponse {
    choices: Vec<KimiChoice>,
    #[serde(default)]
    usage: Option<KimiUsage>,
}

#[derive(Deserialize)]
struct KimiChoice {
    message: KimiResponseMessage,
}

#[derive(Deserialize)]
struct KimiResponseMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<KimiToolCallOut>>,
}

#[derive(Deserialize)]
struct KimiUsage {
    #[serde(default)]
    prompt_tokens: Option<u64>,
    #[serde(default)]
    completion_tokens: Option<u64>,
}

// ─── Message conversion ──────────────────────────────────────────────────────

fn to_kimi_messages(messages: &[ChatMessage]) -> Vec<KimiMessage> {
    messages
        .iter()
        .map(|m| {
            // Unpack assistant messages that carry serialized tool_calls
            if m.role == "assistant" {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&m.content) {
                    if val.get("tool_calls").is_some() {
                        let content = val
                            .get("content")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let calls: Vec<KimiToolCallOut> = val
                            .get("tool_calls")
                            .and_then(|v| serde_json::from_value(v.clone()).ok())
                            .unwrap_or_default();
                        return KimiMessage {
                            role: "assistant".to_string(),
                            content,
                            tool_call_id: None,
                            tool_calls: Some(calls),
                        };
                    }
                }
            }

            // Unpack tool result messages
            if m.role == "tool" {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&m.content) {
                    return KimiMessage {
                        role: "tool".to_string(),
                        content: val
                            .get("content")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        tool_call_id: val
                            .get("tool_call_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        tool_calls: None,
                    };
                }
            }

            KimiMessage {
                role: m.role.clone(),
                content: Some(m.content.clone()),
                tool_call_id: None,
                tool_calls: None,
            }
        })
        .collect()
}

// ─── Provider impl ───────────────────────────────────────────────────────────

#[async_trait]
impl Provider for KimiProvider {
    fn name(&self) -> &str {
        "Kimi"
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
        let tools: Option<Vec<KimiTool>> = req.tools.map(|specs| {
            specs
                .iter()
                .map(|s| KimiTool {
                    kind: "function".to_string(),
                    function: KimiFunctionSpec {
                        name: s.name.clone(),
                        description: s.description.clone(),
                        parameters: s.parameters.clone(),
                    },
                })
                .collect()
        });

        let body = KimiRequest {
            model: model.to_string(),
            messages: to_kimi_messages(req.messages),
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
            return Err(crate::providers::api_error("Kimi", resp).await);
        }

        let raw = crate::providers::parse_json::<KimiResponse>("Kimi", resp).await?;
        let message = raw
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Kimi returned no choices"))?
            .message;

        let text = message.content;
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
