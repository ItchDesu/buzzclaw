use crate::providers::{ChatMessage, ChatRequest, ChatResponse, Provider, ToolCall};
use crate::tools::{ToolRegistry, ToolResult};
use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Drives one complete user-to-final-answer cycle with optional tool use.
pub struct AgentLoop {
    provider: Arc<dyn Provider>,
    tools: Arc<ToolRegistry>,
    model: String,
    temperature: f64,
    system_prompt: Option<String>,
    max_tool_iterations: usize,
    max_history_messages: usize,
}

impl AgentLoop {
    pub fn new(
        provider: Arc<dyn Provider>,
        tools: Arc<ToolRegistry>,
        model: impl Into<String>,
        temperature: f64,
        system_prompt: Option<String>,
        max_tool_iterations: usize,
        max_history_messages: usize,
    ) -> Self {
        Self {
            provider,
            tools,
            model: model.into(),
            temperature,
            system_prompt,
            max_tool_iterations,
            max_history_messages,
        }
    }

    /// Run one full agent turn: user message → (tool calls → results)* → final answer.
    /// Returns the final text response.
    pub async fn run_turn(
        &self,
        history: &mut Vec<ChatMessage>,
        user_message: impl Into<String>,
    ) -> Result<String> {
        let user_msg = user_message.into();
        history.push(ChatMessage::user(&user_msg));

        // Inject system prompt at position 0 if not already there
        if !history.iter().any(|m| m.role == "system") {
            if let Some(sys) = &self.system_prompt {
                history.insert(0, ChatMessage::system(sys));
            }
        }

        let tool_specs = if self.provider.supports_tools() {
            Some(self.tools.specs())
        } else {
            None
        };

        let mut iterations = 0;

        loop {
            self.maybe_compact(history);

            let req = ChatRequest {
                messages: history,
                tools: tool_specs.as_deref(),
            };

            let resp = self
                .provider
                .chat(req, &self.model, self.temperature)
                .await?;

            if let Some(usage) = &resp.usage {
                debug!(
                    input_tokens = ?usage.input_tokens,
                    output_tokens = ?usage.output_tokens,
                    "token usage"
                );
            }

            // No tool calls → final answer
            if !resp.has_tool_calls() {
                let text = sanitize_assistant_text(&resp.text.unwrap_or_default());
                history.push(ChatMessage::assistant(&text));
                return Ok(text);
            }

            // Too many iterations → bail safely
            if iterations >= self.max_tool_iterations {
                warn!(
                    max = self.max_tool_iterations,
                    "max tool iterations reached — returning partial answer"
                );
                let text = resp.text_or_empty().to_string();
                history.push(ChatMessage::assistant(&text));
                return Ok(text);
            }
            iterations += 1;

            // Store assistant message with embedded tool_calls
            let assistant_msg = self.pack_assistant_msg(&resp);
            history.push(assistant_msg);

            // Execute all tool calls and collect results
            for call in &resp.tool_calls {
                let result = self.execute_tool(call).await;
                info!(tool = %call.name, success = result.success, "tool executed");

                let tool_msg = ChatMessage::tool_result(&call.id, &result.output);
                history.push(tool_msg);
            }
        }
    }

    /// Pack an assistant response with tool_calls into a single ChatMessage.
    fn pack_assistant_msg(&self, resp: &ChatResponse) -> ChatMessage {
        let calls_json: Vec<serde_json::Value> = resp
            .tool_calls
            .iter()
            .map(|tc| {
                serde_json::json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.name,
                        "arguments": tc.arguments,
                    }
                })
            })
            .collect();

        let content = sanitize_assistant_text(resp.text.as_deref().unwrap_or(""));
        let packed = serde_json::json!({
            "content": content,
            "tool_calls": calls_json,
        });

        ChatMessage::assistant(packed.to_string())
    }

    /// Execute a single tool call.
    async fn execute_tool(&self, call: &ToolCall) -> ToolResult {
        match self.tools.get(&call.name) {
            None => ToolResult::err(format!("unknown tool: {}", call.name)),
            Some(tool) => {
                let args: serde_json::Value =
                    serde_json::from_str(&call.arguments).unwrap_or(serde_json::Value::Null);
                tool.run(args).await
            }
        }
    }

    /// Compact history when it grows too large by summarizing old messages.
    fn maybe_compact(&self, history: &mut Vec<ChatMessage>) {
        let non_system: Vec<_> = history.iter().filter(|m| m.role != "system").collect();
        if non_system.len() <= self.max_history_messages {
            return;
        }

        // Keep system messages intact and the most recent N non-system messages.
        let keep_recent = self.max_history_messages / 2;
        let system_msgs: Vec<ChatMessage> = history
            .iter()
            .filter(|m| m.role == "system")
            .cloned()
            .collect();

        let non_sys: Vec<ChatMessage> = history
            .iter()
            .filter(|m| m.role != "system")
            .cloned()
            .collect();

        let drop_count = non_sys.len().saturating_sub(keep_recent);
        let kept = non_sys[drop_count..].to_vec();

        info!(
            dropped = drop_count,
            kept = kept.len(),
            "history compacted"
        );

        history.clear();
        history.extend(system_msgs);
        history.extend(kept);
    }
}

/// Remove accidental tool-call JSON blobs from assistant-visible text.
fn sanitize_assistant_text(text: &str) -> String {
    let mut clean = text.trim().to_string();

    if let Some(start_idx) = clean.find("<think>") {
        if let Some(end_idx) = clean.find("</think>") {
            let before = &clean[..start_idx];
            let after = &clean[end_idx + 8..];
            clean = format!("{}{}", before, after).trim().to_string();
        }
    }

    let mut json_candidate = clean.as_str();
    if json_candidate.starts_with("```json") && json_candidate.ends_with("```") {
        json_candidate = json_candidate.trim_start_matches("```json").trim_end_matches("```").trim();
    } else if json_candidate.starts_with("```") && json_candidate.ends_with("```") {
        json_candidate = json_candidate.trim_start_matches("```").trim_end_matches("```").trim();
    }

    if json_candidate.starts_with('{') {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_candidate) {
            if let Some(content) = val.get("content").and_then(|v| v.as_str()) {
                return content.to_string();
            } else if let Some(text) = val.get("text").and_then(|v| v.as_str()) {
                return text.to_string();
            } else if let Some(response) = val.get("response").and_then(|v| v.as_str()) {
                return response.to_string();
            } else if val.get("tool_calls").is_some() {
                return String::new();
            }
        }
    }

    clean
}
