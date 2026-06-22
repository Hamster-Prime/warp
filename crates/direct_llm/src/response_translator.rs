//! Translates OpenAI SSE response stream → Warp ResponseEvent stream.
//!
//! Critical protocol requirement:
//! 1. StreamInit → establishes the response stream
//! 2. AddMessagesToTask with AgentOutput → creates the message "exchange"
//!    that AppendToMessageContent will later target
//! 3. AppendToMessageContent with proper FieldMask → appends text deltas
//! 4. StreamFinished → signals completion

use std::sync::Arc;
use futures::stream::{StreamExt, BoxStream, unfold};
use warp_multi_agent_api as api;

use super::openai_client::ChatCompletionChunk;
use super::DirectLlmError;

struct StreamState {
    conversation_id: String,
    request_id: String,
    task_id: String,
    agent_message_id: String,
    initialized: bool,
    /// Whether we've sent the initial AddMessagesToTask for the agent response.
    agent_message_created: bool,
    finished: bool,
    pending: std::collections::VecDeque<api::ResponseEvent>,
}

pub fn translate_stream(
    sse_stream: BoxStream<'static, Result<ChatCompletionChunk, DirectLlmError>>,
    request: &api::Request,
    _model: String,
) -> BoxStream<'static, Result<api::ResponseEvent, Arc<DirectLlmError>>> {
    let conversation_id = uuid::Uuid::new_v4().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let task_id = request
        .task_context
        .as_ref()
        .and_then(|tc| tc.tasks.last())
        .map(|t| t.id.clone())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let state = StreamState {
        conversation_id,
        request_id,
        task_id,
        agent_message_id: uuid::Uuid::new_v4().to_string(),
        initialized: false,
        agent_message_created: false,
        finished: false,
        pending: std::collections::VecDeque::new(),
    };

    let stream = unfold(
        (state, sse_stream),
        |(mut state, mut sse_stream)| async move {
            loop {
                if let Some(event) = state.pending.pop_front() {
                    return Some((Ok(event), (state, sse_stream)));
                }

                if state.finished {
                    return None;
                }

                match sse_stream.next().await {
                    None => {
                        state.finished = true;
                        state.pending.push_back(make_stream_finished());
                        continue;
                    }
                    Some(Err(e)) => {
                        return Some((Err(Arc::new(e)), (state, sse_stream)));
                    }
                    Some(Ok(chunk)) => {
                        // 1. StreamInit on first chunk
                        if !state.initialized {
                            state.initialized = true;
                            state.pending.push_back(api::ResponseEvent {
                                r#type: Some(api::response_event::Type::Init(
                                    api::response_event::StreamInit {
                                        conversation_id: state.conversation_id.clone(),
                                        request_id: state.request_id.clone(),
                                        run_id: uuid::Uuid::new_v4().to_string(),
                                    },
                                )),
                            });
                        }

                        for choice in &chunk.choices {
                            if let Some(content) = &choice.delta.content
                                && !content.is_empty()
                            {
                                // 2. Create the agent message on first content delta
                                if !state.agent_message_created {
                                    state.agent_message_created = true;
                                    state.pending.push_back(make_client_actions(vec![
                                        api::ClientAction {
                                            action: Some(api::client_action::Action::AddMessagesToTask(
                                                api::client_action::AddMessagesToTask {
                                                    task_id: state.task_id.clone(),
                                                    messages: vec![api::Message {
                                                        id: state.agent_message_id.clone(),
                                                        task_id: state.task_id.clone(),
                                                        message: Some(api::message::Message::AgentOutput(
                                                            api::message::AgentOutput {
                                                                text: String::new(),
                                                            },
                                                        )),
                                                        ..Default::default()
                                                    }],
                                                },
                                            )),
                                        },
                                    ]));
                                }

                                // 3. Append content delta with proper FieldMask
                                state.pending.push_back(make_client_actions(vec![
                                    api::ClientAction {
                                        action: Some(api::client_action::Action::AppendToMessageContent(
                                            api::client_action::AppendToMessageContent {
                                                task_id: state.task_id.clone(),
                                                message: Some(api::Message {
                                                    id: state.agent_message_id.clone(),
                                                    task_id: state.task_id.clone(),
                                                    message: Some(api::message::Message::AgentOutput(
                                                        api::message::AgentOutput {
                                                            text: content.clone(),
                                                        },
                                                    )),
                                                    ..Default::default()
                                                }),
                                                mask: Some(prost_types::FieldMask {
                                                    paths: vec![
                                                        "message.agent_output.text".to_string(),
                                                    ],
                                                }),
                                            },
                                        )),
                                    },
                                ]));
                            }

                            if choice.finish_reason.is_some() {
                                state.finished = true;
                                state.pending.push_back(make_stream_finished());
                            }
                        }
                        continue;
                    }
                }
            }
        },
    );

    Box::pin(stream)
}

fn make_client_actions(actions: Vec<api::ClientAction>) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions { actions },
        )),
    }
}

fn make_stream_finished() -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::Finished(
            api::response_event::StreamFinished {
                reason: Some(api::response_event::stream_finished::Reason::Done(
                    api::response_event::stream_finished::Done {},
                )),
                ..Default::default()
            },
        )),
    }
}
