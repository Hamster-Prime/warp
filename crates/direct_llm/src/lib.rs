//! Direct LLM connection layer for local-only Warp builds.
//!
//! Replaces the Warp server relay (`app.warp.dev/ai/multi-agent`) with direct
//! calls to OpenAI-compatible APIs. Translates between Warp's protobuf
//! message format and standard OpenAI chat completion API.
//!
//! ## Architecture
//!
//! ```text
//! Warp Agent UI
//!   ↕ (warp_multi_agent_api::Request / ResponseEvent)
//! direct_llm::translate_request() / stream_to_response_events()
//!   ↕ (OpenAI ChatCompletion JSON)
//! reqwest → user's OpenAI-compatible endpoint
//! ```
//!
//! ## Key translations
//!
//! | Warp protobuf | OpenAI API |
//! |---|---|
//! | `Task.messages[].UserQuery` | `messages[].role=user` |
//! | `Task.messages[].AgentOutput` | `messages[].role=assistant` |
//! | `Task.messages[].ToolCall` | `messages[].tool_calls` |
//! | `Task.messages[].ToolCallResult` | `messages[].role=tool` |
//! | SSE `AppendToMessageContent` | `choices[].delta.content` |
//! | `ClientAction` with `ToolCall` | `choices[].delta.tool_calls` |

pub mod openai_client;
pub mod request_translator;
pub mod response_translator;
pub mod system_prompt;

use std::sync::Arc;
use futures::stream::BoxStream;
use warp_multi_agent_api as api;

/// Configuration for a direct LLM connection.
#[derive(Clone, Debug)]
pub struct DirectLlmConfig {
    /// Base URL of the OpenAI-compatible API (e.g. "https://api.openai.com/v1").
    pub base_url: String,
    /// API key for authentication.
    pub api_key: String,
    /// Model ID to use (e.g. "gpt-4o", "claude-3-5-sonnet-20241022").
    pub model: String,
}

/// Generates a stream of `ResponseEvent`s by directly calling the LLM API.
///
/// This is the main entry point, called by `ServerApi::generate_multi_agent_output`
/// in local-only builds.
///
/// Translation flow:
/// 1. Extract conversation history from `Request.task_context`
/// 2. Build system prompt locally
/// 3. Convert to OpenAI chat messages
/// 4. Call the LLM API with streaming
/// 5. Convert SSE chunks → `ResponseEvent::ClientActions(AppendToMessageContent)`
/// 6. Handle tool calls → `ResponseEvent::ClientActions(AddMessagesToTask(ToolCall))`
pub async fn generate_direct(
    config: DirectLlmConfig,
    request: &api::Request,
) -> anyhow::Result<BoxStream<'static, Result<api::ResponseEvent, Arc<DirectLlmError>>>> {
    // 1. Translate Warp request → OpenAI messages
    let (messages, system_prompt) = request_translator::translate_request(request)?;

    // 2. Build OpenAI request
    let system_message = openai_client::ChatMessage {
        role: "system".to_string(),
        content: Some(system_prompt),
        tool_calls: None,
        tool_call_id: None,
    };
    let openai_request = openai_client::ChatCompletionRequest {
        model: config.model.clone(),
        messages: std::iter::once(system_message).chain(messages).collect(),
        stream: true,
        tools: request_translator::extract_tool_definitions(request),
    };

    // 3. Call API and get SSE stream
    let sse_stream = openai_client::stream_chat_completion(&config, &openai_request).await?;

    // 4. Translate SSE → ResponseEvent stream
    Ok(response_translator::translate_stream(
        sse_stream,
        request,
        config.model.clone(),
    ))
}

/// Error type for direct LLM connection failures.
#[derive(Debug, thiserror::Error)]
pub enum DirectLlmError {
    #[error("Failed to connect to LLM API: {0}")]
    Connection(String),
    #[error("LLM API returned error: status={status}, body={body}")]
    ApiError { status: u16, body: String },
    #[error("Failed to parse LLM response: {0}")]
    ParseError(String),
    #[error("No API key or endpoint configured. Set up a custom endpoint in Settings → AI.")]
    NotConfigured,
}

// Re-export thiserror (used in DirectLlmError)
extern crate thiserror;
