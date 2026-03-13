use crate::providers::traits::{
    ChatMessage, ChatRequest, ChatResponse, Provider, TokenUsage, ToolCall,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const GEMINI_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

pub struct GeminiProvider {
    api_key: String,
    client: Client,
}

impl GeminiProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            client: crate::providers::default_http_client(),
        }
    }
}

// ─── Request types ────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiSystemInstruction>,
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiToolSpec>>,
    generation_config: GeminiGenerationConfig,
}

#[derive(Serialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiTextPart>,
}

#[derive(Serialize)]
struct GeminiTextPart {
    text: String,
}

#[derive(Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<serde_json::Value>,
}

#[derive(Serialize)]
struct GeminiToolSpec {
    #[serde(rename = "functionDeclarations")]
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Serialize)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    temperature: f64,
}

// ─── Response types ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
    #[serde(default)]
    usage_metadata: Option<GeminiUsage>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiResponseContent>,
}

#[derive(Deserialize)]
struct GeminiResponseContent {
    parts: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiUsage {
    #[serde(default)]
    prompt_token_count: Option<u64>,
    #[serde(default)]
    candidates_token_count: Option<u64>,
}

// ─── Message conversion ───────────────────────────────────────────────────────

/// Convert internal ChatMessage history to Gemini format.
/// Returns (system_instruction, contents).
///
/// Key differences from OpenAI:
/// - Roles are "user" and "model" (not "assistant")
/// - Tool results go into user messages as functionResponse parts
/// - Multiple consecutive tool results are merged into one user content
/// - Tool call IDs must be resolved to function names via prior assistant messages
fn to_gemini_messages(
    messages: &[ChatMessage],
) -> (Option<GeminiSystemInstruction>, Vec<GeminiContent>) {
    // First pass: build map of tool_call_id → function_name
    let mut call_id_to_name: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for m in messages {
        if m.role == "assistant" {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&m.content) {
                if let Some(calls) = val.get("tool_calls").and_then(|v| v.as_array()) {
                    for call in calls {
                        let id = call
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        
                        // Try standard format first
                        let name = if let Some(f) = call.get("function") {
                             f.get("name").and_then(|v| v.as_str()).unwrap_or("tool").to_string()
                        } else {
                            // Fallback to simple format
                             call.get("name").and_then(|v| v.as_str()).unwrap_or("tool").to_string()
                        };

                        if !id.is_empty() {
                            call_id_to_name.insert(id, name);
                        }
                    }
                }
            }
        }
    }

    // Second pass: build contents
    let mut system_parts: Vec<String> = Vec::new();
    let mut contents: Vec<GeminiContent> = Vec::new();

    // We batch consecutive tool messages into one user content
    let mut pending_tool_parts: Vec<serde_json::Value> = Vec::new();

    let flush_tools = |pending: &mut Vec<serde_json::Value>, out: &mut Vec<GeminiContent>| {
        if !pending.is_empty() {
            out.push(GeminiContent {
                role: "user".to_string(),
                parts: std::mem::take(pending),
            });
        }
    };

    for m in messages {
        match m.role.as_str() {
            "system" => {
                flush_tools(&mut pending_tool_parts, &mut contents);
                system_parts.push(m.content.clone());
            }

            "assistant" => {
                flush_tools(&mut pending_tool_parts, &mut contents);

                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&m.content) {
                    if let Some(calls) = val.get("tool_calls").and_then(|v| v.as_array()) {
                        let text = val
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        let mut parts: Vec<serde_json::Value> = Vec::new();
                        if !text.is_empty() {
                            parts.push(serde_json::json!({"text": text}));
                        }
                        for call in calls {
                            // Try standard format first
                            let (name, args_str) = if let Some(f) = call.get("function") {
                                (
                                    f.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    f.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}").to_string()
                                )
                            } else {
                                // Fallback to simple format
                                (
                                    call.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    call.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}").to_string()
                                )
                            };

                            if !name.is_empty() {
                                let args: serde_json::Value = serde_json::from_str(&args_str)
                                    .unwrap_or(serde_json::json!({}));
                                parts.push(serde_json::json!({
                                    "functionCall": {"name": name, "args": args}
                                }));
                            }
                        }
                        contents.push(GeminiContent { role: "model".to_string(), parts });
                        continue;
                    }
                }

                // Plain assistant text
                contents.push(GeminiContent {
                    role: "model".to_string(),
                    parts: vec![serde_json::json!({"text": m.content})],
                });
            }

            "tool" => {
                // Accumulate tool results; will be flushed as a single user message
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&m.content) {
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
                    let func_name = call_id_to_name
                        .get(&tool_call_id)
                        .cloned()
                        .unwrap_or_else(|| "tool".to_string());

                    pending_tool_parts.push(serde_json::json!({
                        "functionResponse": {
                            "name": func_name,
                            "response": {"output": content}
                        }
                    }));
                }
            }

            _ => {
                // user
                flush_tools(&mut pending_tool_parts, &mut contents);
                contents.push(GeminiContent {
                    role: "user".to_string(),
                    parts: vec![serde_json::json!({"text": m.content})],
                });
            }
        }
    }

    flush_tools(&mut pending_tool_parts, &mut contents);

    let system = if system_parts.is_empty() {
        None
    } else {
        Some(GeminiSystemInstruction {
            parts: vec![GeminiTextPart { text: system_parts.join("\n") }],
        })
    };

    (system, contents)
}

// ─── Provider impl ────────────────────────────────────────────────────────────

#[async_trait]
impl Provider for GeminiProvider {
    fn name(&self) -> &str {
        "Gemini"
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
        let (system_instruction, contents) = to_gemini_messages(req.messages);

        let tools: Option<Vec<GeminiToolSpec>> = req.tools.map(|specs| {
            vec![GeminiToolSpec {
                function_declarations: specs
                    .iter()
                    .map(|s| GeminiFunctionDeclaration {
                        name: s.name.clone(),
                        description: s.description.clone(),
                        parameters: s.parameters.clone(),
                    })
                    .collect(),
            }]
        });

        let body = GeminiRequest {
            system_instruction,
            contents,
            tools,
            generation_config: GeminiGenerationConfig { temperature },
        };

        let url = format!("{}/{}:generateContent", GEMINI_BASE, model);

        let resp = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(crate::providers::api_error("Gemini", resp).await);
        }

        let raw = crate::providers::parse_json::<GeminiResponse>("Gemini", resp).await?;

        let mut text_parts: Vec<String> = Vec::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();

        if let Some(candidate) = raw.candidates.into_iter().next() {
            if let Some(content) = candidate.content {
                for part in content.parts {
                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                        if !text.is_empty() {
                            text_parts.push(text.to_string());
                        }
                    } else if let Some(fc) = part.get("functionCall") {
                        let name = fc
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let args = fc
                            .get("args")
                            .cloned()
                            .unwrap_or(serde_json::json!({}));
                        tool_calls.push(ToolCall {
                            id: uuid::Uuid::new_v4().to_string(),
                            name,
                            arguments: args.to_string(),
                        });
                    }
                }
            }
        }

        let text = if text_parts.is_empty() { None } else { Some(text_parts.join("\n")) };

        let usage = raw.usage_metadata.map(|u| TokenUsage {
            input_tokens: u.prompt_token_count,
            output_tokens: u.candidates_token_count,
        });

        Ok(ChatResponse { text, tool_calls, usage })
    }
}
