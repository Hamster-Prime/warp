//! OpenAI-compatible API client.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::DirectLlmConfig;

#[derive(Serialize, Debug)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
}

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
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCall>>,
}

pub async fn stream_chat_completion(
    config: &DirectLlmConfig,
    request: &ChatCompletionRequest,
) -> Result<futures::stream::BoxStream<'static, Result<ChatCompletionChunk, super::DirectLlmError>>, super::DirectLlmError> {
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    log::info!("[direct_llm] POST {url} model={}", config.model);
    log::info!("[direct_llm] messages count: {}", request.messages.len());

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
        log::error!("[direct_llm] API error: status={status}, body={body}");
        return Err(super::DirectLlmError::ApiError { status, body });
    }

    log::info!("[direct_llm] SSE stream connected, parsing chunks...");

    let byte_stream = response.bytes_stream();
    let chunk_stream = parse_sse_stream(byte_stream);
    Ok(Box::pin(chunk_stream))
}

/// Parses an SSE byte stream into ChatCompletionChunks.
fn parse_sse_stream(
    byte_stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
) -> impl futures::Stream<Item = Result<ChatCompletionChunk, super::DirectLlmError>> + Send {
    use futures::StreamExt;

    let buffer = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let buf2 = buffer.clone();

    byte_stream.filter_map(move |chunk_result| {
        let buffer = buf2.clone();

        let result = match chunk_result {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                log::info!("[direct_llm] raw SSE chunk: {} bytes", bytes.len());

                let mut buf = buffer.lock().unwrap();
                buf.push_str(&text);

                let mut last_chunk: Option<ChatCompletionChunk> = None;
                let mut total_parsed = 0;

                while let Some(pos) = buf.find('\n') {
                    let line = buf[..pos].trim().to_string();
                    buf.drain(..=pos);

                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }

                    if let Some(json_str) = line.strip_prefix("data: ") {
                        if json_str.trim() == "[DONE]" {
                            log::info!("[direct_llm] SSE [DONE] received");
                            continue;
                        }

                        match serde_json::from_str::<ChatCompletionChunk>(json_str) {
                            Ok(chunk) => {
                                total_parsed += 1;
                                let has_content = chunk.choices.iter().any(|c| {
                                    c.delta.content.as_ref().is_some_and(|s| !s.is_empty())
                                });
                                let has_finish = chunk.choices.iter().any(|c| c.finish_reason.is_some());
                                if has_content || has_finish {
                                    log::info!(
                                        "[direct_llm] parsed chunk #{}: content={} finish={:?}",
                                        total_parsed,
                                        has_content,
                                        chunk.choices.first().and_then(|c| c.finish_reason.as_deref())
                                    );
                                }
                                last_chunk = Some(chunk);
                            }
                            Err(e) => {
                                log::warn!("[direct_llm] Failed to parse SSE JSON: {e}");
                                log::warn!("[direct_llm] Raw JSON was: {json_str}");
                            }
                        }
                    } else {
                        log::warn!("[direct_llm] Unexpected SSE line: {line}");
                    }
                }

                if total_parsed > 0 {
                    log::info!("[direct_llm] parsed {total_parsed} chunks from this TCP segment");
                }

                last_chunk.map(Ok)
            }
            Err(e) => {
                log::error!("[direct_llm] HTTP stream error: {e}");
                Some(Err(super::DirectLlmError::Connection(e.to_string())))
            }
        };

        std::future::ready(result)
    })
}
