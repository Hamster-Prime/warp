//! Translates Warp protobuf request → OpenAI chat messages.

use warp_multi_agent_api as api;

use super::openai_client::{ChatMessage, ToolDefinition, FunctionDefinition};
use super::system_prompt;

/// Converts a Warp `Request` into OpenAI-format messages + system prompt.
pub fn translate_request(
    request: &api::Request,
) -> Result<(Vec<ChatMessage>, String), super::DirectLlmError> {
    let system = system_prompt::build_system_prompt();

    let mut messages = Vec::new();

    // Extract conversation history from task_context
    if let Some(task_context) = &request.task_context {
        for task in &task_context.tasks {
            for msg in &task.messages {
                if let Some(msg_type) = &msg.message {
                    match msg_type {
                        api::message::Message::UserQuery(query) => {
                            messages.push(ChatMessage {
                                role: "user".to_string(),
                                content: Some(query.query.clone()),
                                tool_calls: None,
                                tool_call_id: None,
                            });
                        }
                        api::message::Message::AgentOutput(output) => {
                            messages.push(ChatMessage {
                                role: "assistant".to_string(),
                                content: Some(output.text.clone()),
                                tool_calls: None,
                                tool_call_id: None,
                            });
                        }
                        api::message::Message::ToolCall(tool_call) => {
                            // Convert Warp tool call to OpenAI tool_calls format
                            let (name, args) = warp_tool_to_openai(tool_call);
                            messages.push(ChatMessage {
                                role: "assistant".to_string(),
                                content: None,
                                tool_calls: Some(vec![super::openai_client::ToolCall {
                                    id: msg.id.clone(),
                                    call_type: "function".to_string(),
                                    function: super::openai_client::FunctionCall {
                                        name,
                                        arguments: Some(args),
                                    },
                                }]),
                                tool_call_id: None,
                            });
                        }
                        api::message::Message::ToolCallResult(result) => {
                            let content = warp_tool_result_to_text(result);
                            messages.push(ChatMessage {
                                role: "tool".to_string(),
                                content: Some(content),
                                tool_calls: None,
                                tool_call_id: Some(msg.id.clone()),
                            });
                        }
                        // Skip other message types for now
                        _ => {}
                    }
                }
            }
        }
    }

    // Extract the latest user input from `input`
    if let Some(input) = &request.input
        && let Some(input_type) = &input.r#type
        && let api::request::input::Type::UserInputs(user_inputs) = input_type
    {
        for user_input in &user_inputs.inputs {
            if let Some(api::request::input::user_inputs::user_input::Input::UserQuery(query)) =
                &user_input.input
            {
                messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: Some(query.query.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
        }
    }

    Ok((messages, system))
}

/// Extracts tool definitions from the request settings.
///
/// Maps Warp's 34 tool types to OpenAI function definitions.
pub fn extract_tool_definitions(request: &api::Request) -> Option<Vec<ToolDefinition>> {
    if let Some(settings) = &request.settings {
        let tools: Vec<ToolDefinition> = settings
            .supported_tools
            .iter()
            .filter_map(|&tool_type| warp_tool_to_definition(tool_type))
            .collect();

        if tools.is_empty() {
            None
        } else {
            Some(tools)
        }
    } else {
        None
    }
}

/// Maps a Warp ToolType to an OpenAI function definition.
fn warp_tool_to_definition(tool_type: i32) -> Option<ToolDefinition> {
    let tool_type = api::ToolType::try_from(tool_type).ok()?;
    let (name, description, params) = match tool_type {
        api::ToolType::RunShellCommand => (
            "run_shell_command",
            "Execute a shell command on the user's system.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "The shell command to execute"},
                    "working_directory": {"type": "string", "description": "Working directory for the command"}
                },
                "required": ["command"]
            }),
        ),
        api::ToolType::ReadFiles => (
            "read_files",
            "Read the contents of one or more files.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "paths": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "List of file paths to read"
                    }
                },
                "required": ["paths"]
            }),
        ),
        api::ToolType::ApplyFileDiffs => (
            "apply_file_diffs",
            "Apply file diffs to modify existing files.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "diffs": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": {"type": "string"},
                                "content": {"type": "string", "description": "The new file content or diff"}
                            }
                        }
                    }
                },
                "required": ["diffs"]
            }),
        ),
        api::ToolType::Grep => (
            "grep",
            "Search for patterns in files using grep.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": "string", "description": "Directory to search in"},
                    "include": {"type": "string", "description": "File pattern to include"}
                },
                "required": ["pattern"]
            }),
        ),
        api::ToolType::FileGlob => (
            "file_glob",
            "Find files matching a glob pattern.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Glob pattern (e.g. **/*.rs)"},
                    "path": {"type": "string", "description": "Root directory"}
                },
                "required": ["pattern"]
            }),
        ),
        api::ToolType::CallMcpTool => (
            "call_mcp_tool",
            "Call a tool provided by an MCP (Model Context Protocol) server.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "server_name": {"type": "string"},
                    "tool_name": {"type": "string"},
                    "arguments": {"type": "string", "description": "JSON-encoded arguments"}
                },
                "required": ["server_name", "tool_name"]
            }),
        ),
        _ => return None, // Skip unmapped tools
    };

    Some(ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: name.to_string(),
            description: Some(description.to_string()),
            parameters: Some(params),
        },
    })
}

/// Converts a Warp ToolCall protobuf to (function_name, arguments_json).
fn warp_tool_to_openai(tool_call: &api::message::ToolCall) -> (String, String) {
    use api::message::tool_call::Tool as T;

    let Some(tool) = &tool_call.tool else {
        return ("unknown".to_string(), "{}".to_string());
    };

    match tool {
        T::RunShellCommand(cmd) => (
            "run_shell_command".to_string(),
            serde_json::json!({"command": cmd.command}).to_string(),
        ),
        T::ReadFiles(read) => (
            "read_files".to_string(),
            serde_json::json!({"paths": read.files.iter().map(|f| &f.name).collect::<Vec<_>>()})
                .to_string(),
        ),
        T::ApplyFileDiffs(diffs) => (
            "apply_file_diffs".to_string(),
            serde_json::json!({
                "diffs": diffs.diffs.iter().map(|d| serde_json::json!({
                    "path": d.file_path,
                    "content": d.replace,
                })).collect::<Vec<_>>()
            })
            .to_string(),
        ),
        T::Grep(grep) => (
            "grep".to_string(),
            serde_json::json!({"queries": grep.queries, "path": grep.path}).to_string(),
        ),
        _ => ("unknown".to_string(), "{}".to_string()),
    }
}

/// Converts a Warp ToolCallResult to a text representation for OpenAI.
fn warp_tool_result_to_text(result: &api::message::ToolCallResult) -> String {
    use api::message::tool_call_result::Result as R;

    let Some(result) = &result.result else {
        return "Tool result empty".to_string();
    };

    match result {
        R::RunShellCommand(r) => match &r.result {
            Some(api::run_shell_command_result::Result::CommandFinished(finished)) => {
                format!(
                    "Exit code: {}\nOutput:\n{}",
                    finished.exit_code, finished.output
                )
            }
            _ => "Command running".to_string(),
        },
        _ => "Tool result not yet supported".to_string(),
    }
}
