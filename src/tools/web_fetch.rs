use super::traits::{Tool, ToolResult};
use crate::tools::external_tools::external_tool_runner;
use async_trait::async_trait;
use futures_util::StreamExt;
use std::net::IpAddr;
use std::time::Duration;

pub struct WebFetchTool {
    client: reqwest::Client,
}

impl WebFetchTool {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch the content of a URL and return it as plain text. \
         HTML is converted to readable text. Good for reading web pages or APIs."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch."
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Maximum characters to return. Default: 8000."
                }
            },
            "required": ["url"]
        })
    }

    async fn run(&self, args: serde_json::Value) -> ToolResult {
        let url = match args.get("url").and_then(|v| v.as_str()) {
            Some(u) => u.to_string(),
            None => return ToolResult::err("missing 'url' argument"),
        };
        let max_chars = args
            .get("max_chars")
            .and_then(|v| v.as_u64())
            .unwrap_or(8000) as usize;

        if let Some(runner) = external_tool_runner() {
            return match runner.web_fetch(&url, max_chars) {
                Ok(res) => {
                    if res.success { ToolResult::ok(res.output) } else { ToolResult::err(res.output) }
                }
                Err(e) => ToolResult::err(format!("external web_fetch error: {e}")),
            };
        }

        let parsed = match reqwest::Url::parse(&url) {
            Ok(u) => u,
            Err(e) => return ToolResult::err(format!("invalid url: {e}")),
        };

        let scheme = parsed.scheme();
        if scheme != "http" && scheme != "https" {
            return ToolResult::err("only http/https urls are allowed");
        }

        if let Some(host) = parsed.host_str() {
            if is_blocked_host(host) {
                return ToolResult::err("blocked host");
            }
        } else {
            return ToolResult::err("missing host");
        }

        let resp = match self
            .client
            .get(parsed)
            .header("User-Agent", "buzzclaw/0.1 (bot)")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => return ToolResult::err(format!("HTTP error: {e}")),
        };

        if !resp.status().is_success() {
            return ToolResult::err(format!("HTTP {}", resp.status()));
        }

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let max_bytes = max_chars.saturating_mul(4).min(1_000_000);
        let mut buf: Vec<u8> = Vec::new();
        let mut truncated = false;

        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => return ToolResult::err(format!("failed to read body: {e}")),
            };
            if buf.len() + chunk.len() > max_bytes {
                let remaining = max_bytes.saturating_sub(buf.len());
                buf.extend_from_slice(&chunk[..remaining]);
                truncated = true;
                break;
            }
            buf.extend_from_slice(&chunk);
        }

        let body = String::from_utf8_lossy(&buf).to_string();

        let plain = if content_type.contains("text/html") {
            nanohtml2text::html2text(&body)
        } else {
            body
        };

        let trimmed = if plain.len() > max_chars || truncated {
            let slice = plain.chars().take(max_chars).collect::<String>();
            format!("{slice}…[truncated]")
        } else {
            plain
        };

        ToolResult::ok(trimmed)
    }
}

fn is_blocked_host(host: &str) -> bool {
    let host_lc = host.trim().to_ascii_lowercase();
    if host_lc == "localhost"
        || host_lc.ends_with(".localhost")
        || host_lc.ends_with(".local")
        || host_lc.ends_with(".localdomain")
        || host_lc.ends_with(".internal")
    {
        return true;
    }

    if let Ok(ip) = host_lc.parse::<IpAddr>() {
        return is_private_ip(&ip);
    }

    false
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
        }
    }
}
