//! OpenAI-compatible API client.
//!
//! Handles HTTP communication with any OpenAI-compatible endpoint
//! (OpenAI, Anthropic via proxy, Ollama, LM Studio, etc.)

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::DirectLlmConfig;

/// OpenAI chat completion request.
#[derive(Serialize, Debug)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
}

/// A single chat message in OpenAI format.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// OpenAI function/tool definition.
#[derive(Serialize, Debug)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

#[derive(Serialize, Debug)]
pub struct FunctionDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
}

/// A tool call from the assistant.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FunctionCall {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// A chunk from the streaming SSE response.
#[derive(Deserialize, Debug)]
pub struct ChatCompletionChunk {
    pub choices: Vec<ChunkChoice>,
}

#[derive(Deserialize, Debug)]
pub struct ChunkChoice {
    pub delta: Delta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// Starts a streaming chat completion request.
///
/// Sends POST to `{base_url}/chat/completions` with streaming enabled.
/// Returns a stream of `ChatCompletionChunk`s parsed from the SSE response.
pub async fn stream_chat_completion(
    config: &DirectLlmConfig,
    request: &ChatCompletionRequest,
) -> Result<futures::stream::BoxStream<'static, Result<ChatCompletionChunk, super::DirectLlmError>>, super::DirectLlmError> {
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| super::DirectLlmError::Connection(e.to_string()))?;

    let response = client
        .post(&url)
        .bearer_auth(&config.api_key)
        .json(request)
        .send()
        .await
        .map_err(|e| super::DirectLlmError::Connection(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(super::DirectLlmError::ApiError { status, body });
    }

    // Convert the byte stream into a stream of parsed ChatCompletionChunks.
    let byte_stream = response.bytes_stream();

    let chunk_stream = parse_sse_stream(byte_stream);

    Ok(Box::pin(chunk_stream))
}

/// Parses an SSE byte stream into ChatCompletionChunks.
///
/// SSE format: lines starting with "data: " contain JSON payloads.
/// A line "data: [DONE]" signals the end of the stream.
fn parse_sse_stream(
    byte_stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
) -> impl futures::Stream<Item = Result<ChatCompletionChunk, super::DirectLlmError>> + Send {
    use futures::StreamExt;

    // Buffer for accumulating partial SSE lines across chunk boundaries.
    let mut buffer = String::new();

    byte_stream.filter_map(move |chunk_result| {
        let result = match chunk_result {
            Ok(bytes) => {
                buffer.push_str(&String::from_utf8_lossy(&bytes));

                let mut results = Vec::new();

                // Process complete lines in the buffer.
                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].trim().to_string();
                    buffer = buffer[pos + 1..].to_string();

                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }

                    if let Some(json_str) = line.strip_prefix("data: ") {
                        if json_str.trim() == "[DONE]" {
                            continue;
                        }

                        match serde_json::from_str::<ChatCompletionChunk>(json_str) {
                            Ok(chunk) => results.push(Ok(chunk)),
                            Err(e) => {
                                log::warn!("Failed to parse SSE chunk: {e}");
                            }
                        }
                    }
                }

                if results.is_empty() {
                    None
                } else {
                    // Return the last chunk (typically one per SSE event).
                    results.into_iter().last()
                }
            }
            Err(e) => Some(Err(super::DirectLlmError::Connection(e.to_string()))),
        };

        std::future::ready(result)
    })
}
