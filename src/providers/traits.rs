use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: content.into() }
    }

    pub fn tool_result(tool_call_id: &str, content: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: serde_json::json!({
                "tool_call_id": tool_call_id,
                "content": content.into()
            })
            .to_string(),
        }
    }
}

/// A tool call requested by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Token usage from a single LLM response.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

/// An LLM response — may contain text, tool calls, or both.
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub text: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<TokenUsage>,
}

impl ChatResponse {
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }

    pub fn text_or_empty(&self) -> &str {
        self.text.as_deref().unwrap_or("")
    }
}

/// Request payload passed to providers.
#[derive(Debug, Clone, Copy)]
pub struct ChatRequest<'a> {
    pub messages: &'a [ChatMessage],
    pub tools: Option<&'a [crate::tools::ToolSpec]>,
}

/// Core provider trait — implement for any LLM backend.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Provider name (for logging / error messages).
    #[allow(dead_code)]
    fn name(&self) -> &str;

    /// Whether the provider supports native function calling.
    fn supports_tools(&self) -> bool {
        false
    }

    /// Full agentic chat: messages + optional tools.
    async fn chat(&self, req: ChatRequest<'_>, model: &str, temperature: f64)
        -> anyhow::Result<ChatResponse>;

    /// Simple one-shot call — no history, no tools.
    #[allow(dead_code)]
    async fn simple_chat(
        &self,
        system: Option<&str>,
        user: &str,
        model: &str,
        temperature: f64,
    ) -> anyhow::Result<String> {
        let mut msgs = Vec::new();
        if let Some(s) = system {
            msgs.push(ChatMessage::system(s));
        }
        msgs.push(ChatMessage::user(user));
        let resp = self
            .chat(ChatRequest { messages: &msgs, tools: None }, model, temperature)
            .await?;
        Ok(resp.text.unwrap_or_default())
    }
}
