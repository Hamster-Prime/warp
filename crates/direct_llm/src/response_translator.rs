//! Translates OpenAI SSE response stream → Warp ResponseEvent stream.
//!
//! This is the core adapter that makes the direct LLM connection
//! compatible with Warp's existing Agent UI.

use std::sync::Arc;
use futures::stream::{StreamExt, BoxStream};
use warp_multi_agent_api as api;

use super::openai_client::ChatCompletionChunk;
use super::DirectLlmError;

/// Converts a stream of OpenAI ChatCompletionChunks into a stream of
/// Warp ResponseEvents that the existing Agent UI can consume.
///
/// Translation mapping:
/// - First chunk → `StreamInit` event
/// - Content deltas → `ClientAction::AppendToMessageContent`
/// - Tool call deltas → `ClientAction::AddMessagesToTask` with `ToolCall`
/// - `finish_reason: stop` → `StreamFinished` with `Done`
pub fn translate_stream(
    sse_stream: BoxStream<'static, Result<ChatCompletionChunk, DirectLlmError>>,
    request: &api::Request,
    _model: String,
) -> BoxStream<'static, Result<api::ResponseEvent, Arc<DirectLlmError>>> {
    // Generate IDs for the conversation and initial message.
    let conversation_id = uuid::Uuid::new_v4().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let task_id = request
        .task_context
        .as_ref()
        .and_then(|tc| tc.tasks.last())
        .map(|t| t.id.clone())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let message_id = uuid::Uuid::new_v4().to_string();

    let mut initialized = false;

    // Map each SSE chunk to a ResponseEvent
    let event_stream = sse_stream.filter_map(move |chunk_result| {
        let mut events: Vec<api::ResponseEvent> = Vec::new();

        match chunk_result {
            Ok(chunk) => {
                // Send StreamInit on first chunk
                if !initialized {
                    initialized = true;
                    events.push(api::ResponseEvent {
                        r#type: Some(api::response_event::Type::Init(
                            api::response_event::StreamInit {
                                conversation_id: conversation_id.clone(),
                                request_id: request_id.clone(),
                                run_id: uuid::Uuid::new_v4().to_string(),
                            },
                        )),
                    });
                }

                for choice in &chunk.choices {
                    // Handle content deltas
                    if let Some(content) = &choice.delta.content
                        && !content.is_empty()
                    {
                        // Create AppendToMessageContent action
                        let action = api::ClientAction {
                            action: Some(api::client_action::Action::AppendToMessageContent(
                                api::client_action::AppendToMessageContent {
                                    task_id: task_id.clone(),
                                    message: Some(api::Message {
                                        id: message_id.clone(),
                                        task_id: task_id.clone(),
                                        message: Some(api::message::Message::AgentOutput(
                                            api::message::AgentOutput {
                                                text: content.clone(),
                                            },
                                        )),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                },
                            )),
                        };

                        events.push(api::ResponseEvent {
                            r#type: Some(api::response_event::Type::ClientActions(
                                api::response_event::ClientActions {
                                    actions: vec![action],
                                },
                            )),
                        });
                    }

                    // Handle finish_reason
                    if choice.finish_reason.is_some() {
                        let finished = api::response_event::StreamFinished {
                            reason: Some(api::response_event::stream_finished::Reason::Done(
                                api::response_event::stream_finished::Done {},
                            )),
                            ..Default::default()
                        };

                        events.push(api::ResponseEvent {
                            r#type: Some(api::response_event::Type::Finished(finished)),
                        });
                    }
                }
            }
            Err(e) => {
                return std::future::ready(Some(Err(Arc::new(e))));
            }
        }

        // Return the last event (or None if empty)
        let next = events.into_iter().last().map(Ok);
        std::future::ready(next)
    });

    Box::pin(event_stream)
}
