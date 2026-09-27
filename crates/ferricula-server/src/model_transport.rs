use std::env;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde_json::Value;

use crate::model::{InferenceTransport, ProviderRequest, ProviderResponse};

/// Production HTTP transport. It resolves secret values only at execution
/// time from the env-var names carried in a validated provider request.
#[derive(Debug, Default)]
pub struct HttpInferenceTransport;

impl HttpInferenceTransport {
    fn client(timeout_ms: u64) -> Result<Client> {
        Ok(Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .build()?)
    }

    fn optional_key(name: &str) -> Result<Option<String>> {
        if name.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(env::var(name).with_context(|| {
            format!("model credential environment variable {name} is not set")
        })?))
    }
}

impl InferenceTransport for HttpInferenceTransport {
    fn execute(&self, request: &ProviderRequest) -> Result<ProviderResponse> {
        match request {
            ProviderRequest::NoModel { reason, echo } => Ok(ProviderResponse {
                text: format!("[no-model:{reason}] {echo}"),
                input_tokens: 0,
                output_tokens: 0,
                raw: serde_json::json!({ "mode": "no_model", "reason": reason }),
            }),
            ProviderRequest::OpenAiCompatible {
                url,
                api_key_env,
                timeout_ms,
                body,
            } => {
                let client = Self::client(*timeout_ms)?;
                let mut request = client.post(url).json(body);
                if let Some(key) = Self::optional_key(api_key_env)? {
                    request = request.bearer_auth(key);
                }
                let raw: Value = request.send()?.error_for_status()?.json()?;
                let message = raw.pointer("/choices/0/message")
                    .context("OpenAI-compatible response contained no message")?;
                let content = message.get("content").and_then(Value::as_str).unwrap_or_default();
                let native = native_tool_calls(message);
                if content.is_empty() && native.is_empty() && message.get("content").is_none_or(Value::is_null) {
                    bail!("OpenAI-compatible response contained no message content");
                }
                // Some servers (Ollama with GLM's chat template) parse the
                // text protocol's `<use_tool>` blocks into structured
                // `tool_calls`; put them back so the chat loop sees them.
                let text = if native.is_empty() { content.to_string() } else { format!("{content}{native}") };
                Ok(ProviderResponse {
                    text,
                    input_tokens: usage_u32(&raw, "/usage/prompt_tokens"),
                    output_tokens: usage_u32(&raw, "/usage/completion_tokens"),
                    raw,
                })
            }
            ProviderRequest::Anthropic {
                url,
                api_key_env,
                timeout_ms,
                body,
            } => {
                let key = Self::optional_key(api_key_env)?
                    .context("Anthropic request requires an API key environment variable")?;
                let raw: Value = Self::client(*timeout_ms)?
                    .post(url)
                    .header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01")
                    .json(body)
                    .send()?
                    .error_for_status()?
                    .json()?;
                let Some(content) = raw.get("content").and_then(Value::as_array) else {
                    bail!("Anthropic response contained no content array");
                };
                let text = content
                    .iter()
                    .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(|block| block.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n");
                if text.is_empty() {
                    bail!("Anthropic response contained no text content");
                }
                Ok(ProviderResponse {
                    text,
                    input_tokens: usage_u32(&raw, "/usage/input_tokens"),
                    output_tokens: usage_u32(&raw, "/usage/output_tokens"),
                    raw,
                })
            }
        }
    }
}

/// Structured `message.tool_calls` rendered as `<use_tool>` blocks
/// (`arguments` may be a JSON object or a JSON-encoded string).
fn native_tool_calls(message: &Value) -> String {
    let Some(calls) = message.get("tool_calls").and_then(Value::as_array) else { return String::new() };
    calls.iter().filter_map(|call| {
        let function = call.get("function")?;
        let name = function.get("name")?.as_str()?;
        let arguments = match function.get("arguments") {
            Some(Value::String(s)) => serde_json::from_str(s).unwrap_or_else(|_| serde_json::json!({})),
            Some(value @ Value::Object(_)) => value.clone(),
            _ => serde_json::json!({}),
        };
        Some(format!("<use_tool>{}</use_tool>", serde_json::json!({ "name": name, "arguments": arguments })))
    }).collect()
}

fn usage_u32(value: &Value, pointer: &str) -> u32 {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .and_then(|number| u32::try_from(number).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_tool_calls_become_text_blocks() {
        let message = serde_json::json!({ "content": "", "tool_calls": [
            { "function": { "name": "read_document", "arguments": { "doc_id": "eecee3fca20e6eb7" } } },
            { "function": { "name": "search_memory", "arguments": "{\"query\":\"fence\"}" } },
        ]});
        let text = native_tool_calls(&message);
        assert_eq!(text.matches("<use_tool>").count(), 2);
        assert!(text.contains("\"doc_id\":\"eecee3fca20e6eb7\""));
        assert!(text.contains("\"query\":\"fence\""));
        assert_eq!(native_tool_calls(&serde_json::json!({ "content": "hi" })), "");
    }

    #[test]
    fn missing_usage_is_zero() {
        assert_eq!(usage_u32(&serde_json::json!({}), "/usage/input_tokens"), 0);
    }
}
