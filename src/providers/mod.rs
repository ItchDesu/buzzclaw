pub mod anthropic;
pub mod buzzster;
pub mod deepseek;
pub mod gemini;
pub mod kimi;
pub mod openai;
mod traits;

pub use traits::{ChatMessage, ChatRequest, ChatResponse, Provider, ToolCall};
use std::time::Duration;
use reqwest::Client;

/// Pick which API error message to surface from a failed HTTP response.
pub(crate) async fn api_error(name: &str, resp: reqwest::Response) -> anyhow::Error {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_else(|_| "Could not read error body".to_string());
    anyhow::anyhow!("{name} API error {status}: {body}")
}

/// Robust JSON parsing that surfaces the body on failure.
pub(crate) async fn parse_json<T: serde::de::DeserializeOwned>(name: &str, resp: reqwest::Response) -> anyhow::Result<T> {
    let body = resp.text().await.map_err(|e| anyhow::anyhow!("{name} failed to read response body: {e}"))?;
    
    if body.trim().is_empty() {
        anyhow::bail!("{name} returned an empty response body");
    }

    serde_json::from_str(&body).map_err(|e| {
        let snippet = if body.len() > 200 {
            format!("{}...", &body[..200])
        } else {
            body.clone()
        };
        anyhow::anyhow!("{name} failed to decode JSON: {e}\nResponse snippet: {snippet}")
    })
}

pub(crate) fn default_http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(60)) // Increased timeout
        .tcp_keepalive(Duration::from_secs(60))
        .build()
        .unwrap_or_else(|_| Client::new())
}
