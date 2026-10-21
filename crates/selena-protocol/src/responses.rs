use crate::error::ProtocolError;
use crate::ir::*;
use crate::util::*;
use serde_json::{Map, Value, json};

pub fn decode_request(value: &Value) -> Result<CanonicalRequest, ProtocolError> {
    let object = as_object(value)?;
    let model = object
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| ProtocolError::invalid("`model` is required").with_param("model"))?;
    let mut request = CanonicalRequest::new(Protocol::Responses, model);

    if let Some(instructions) = object.get("instructions") {
        match instructions {
            Value::String(text) => request.system.push(SystemBlock::text(Role::System, text)),
            Value::Array(items) => {
                for item in items {
                    if let Some(text) = item.get("text").and_then(Value::as_str) {
                        request.system.push(SystemBlock::text(Role::System, text));
                    }
                }
            }
            _ => {}
        }
    }

    let input = object
        .get("input")
        .ok_or_else(|| ProtocolError::invalid("`input` is required").with_param("input"))?;
    match input {
        Value::String(text) => request.messages.push(Message::text(Role::User, text)),
        Value::Array(items) => {
            for item in items {
                decode_input_item(item, &mut request)?;
            }
        }
        _ => {
            return Err(ProtocolError::invalid(
                "`input` must be a string or an array of input items",
            ));
        }
    }

    if let Some(tools) = object.get("tools").and_then(Value::as_array) {
        for tool in tools {
            let decoded = decode_tool(tool, &mut request)?;
            request.tools.push(decoded);
        }
    }
    if let Some(choice) = object.get("tool_choice") {
        request.tool_choice = decode_tool_choice(choice, &mut request)?;
    }
    if let Some(format) = object.get("text").and_then(|text| text.get("format")) {
        request.response_format = decode_response_format(format)?;
    }
    request.parallel_tool_calls = object.get("parallel_tool_calls").and_then(Value::as_bool);
    request.max_output_tokens = optional_u32(object, "max_output_tokens");
    request.temperature = optional_f64(object, "temperature");
    request.top_p = optional_f64(object, "top_p");
    request.stream = object
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    request.user = optional_str(object, "user");
    request.safety_identifier = optional_str(object, "safety_identifier");
    request.service_tier = optional_str(object, "service_tier");
    request.state.store = object.get("store").and_then(Value::as_bool);
    request.state.previous_response_id = optional_str(object, "previous_response_id");
    request.state.conversation = optional_str(object, "conversation");
    request.state.background = object.get("background").and_then(Value::as_bool);
    request.state.include = string_or_array(object, "include");
    request.state.truncation = optional_str(object, "truncation");
    request.state.max_tool_calls = optional_u32(object, "max_tool_calls");
    request.reasoning.effort = object
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("effort"))
        .and_then(Value::as_str)
        .map(str::to_string);
    request.reasoning.summary = object
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("summary"))
        .and_then(Value::as_str)
        .map(str::to_string);
    if object.get("metadata").and_then(Value::as_object).is_some() {
        for (key, value) in object["metadata"].as_object().unwrap() {
            if let Some(value) = value.as_str() {
                request.metadata.insert(key.clone(), value.to_string());
            }
        }
    }

    if request.state.store == Some(true)
        || request.state.previous_response_id.is_some()
        || request.state.conversation.is_some()
    {
        request.mark(Capability::ResponsesState);
    }
    if request.state.background == Some(true) {
        request.mark(Capability::Background);
    }
    if !request.state.include.is_empty() {
        request.mark(Capability::Include);
    }
    if request.state.truncation.is_some() {
        request.mark(Capability::Truncation);
    }
    if object.get("prompt").is_some() || object.get("context_management").is_some() {
        request.mark(Capability::PromptTemplate);
    }
    if object.get("max_tool_calls").is_some() {
        request.mark(Capability::HostedTools);
    }
    if object.get("top_logprobs").is_some() {
        request.mark(Capability::Logprobs);
    }
    if object.get("access_programs").is_some() {
        request.mark(Capability::Unknown);
    }

    Ok(request)
}

fn decode_input_item(value: &Value, request: &mut CanonicalRequest) -> Result<(), ProtocolError> {
    let item = value
        .as_object()
        .ok_or_else(|| ProtocolError::invalid("input items must be objects"))?;
    let kind = item
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("message");
    match kind {
        "message" => {
            let role = match item.get("role").and_then(Value::as_str).unwrap_or("user") {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                "system" => Role::System,
                "developer" => Role::Developer,
                other => {
                    return Err(ProtocolError::invalid(format!(
                        "unsupported input message role `{other}`"
                    )));
                }
            };
            let content = parse_input_content(item.get("content"), role, request)?;
            request.messages.push(Message {
                role,
                content,
                name: None,
                tool_call_id: None,
            });
        }
        "function_call" => {
            let call_id = item
                .get("call_id")
                .or_else(|| item.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(generated_tool_id);
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| ProtocolError::invalid("function_call.name is required"))?;
            let raw = item
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}");
            let (input, raw_arguments) = parse_tool_arguments(raw);
            let part = ContentPart::ToolUse {
                id: call_id,
                name: name.to_string(),
                input,
                raw_arguments,
            };
            if let Some(Message {
                role: Role::Assistant,
                content,
                ..
            }) = request.messages.last_mut()
            {
                content.push(part);
            } else {
                request.messages.push(Message {
                    role: Role::Assistant,
                    content: vec![part],
                    name: None,
                    tool_call_id: None,
                });
            }
        }
        "function_call_output" => {
            let call_id = item
                .get("call_id")
                .and_then(Value::as_str)
                .ok_or_else(|| ProtocolError::invalid("function_call_output.call_id is required"))?
                .to_string();
            let content = parse_tool_output(item.get("output"), request)?;
            request.messages.push(Message {
                role: Role::Tool,
                content: vec![ContentPart::ToolResult {
                    tool_use_id: call_id,
                    content,
                    is_error: false,
                }],
                name: None,
                tool_call_id: None,
            });
        }
        "reasoning" => {
            request.mark(Capability::ReasoningItems);
        }
        "item_reference" => {
            request.mark(Capability::ResponsesState);
            request.mark(Capability::ItemReference);
        }
        "input_image" | "input_file" | "input_text" | "input_audio" => {
            let role = Role::User;
            let content = parse_input_content(Some(value), role, request)?;
            request.messages.push(Message {
                role,
                content,
                name: None,
                tool_call_id: None,
            });
        }
        "web_search_call"
        | "file_search_call"
        | "computer_call"
        | "computer_call_output"
        | "code_interpreter_call"
        | "image_generation_call"
        | "mcp_call"
        | "mcp_list_tools"
        | "mcp_approval_request"
        | "mcp_approval_response"
        | "custom_tool_call"
        | "custom_tool_call_output"
        | "local_shell_call"
        | "shell_call"
        | "apply_patch_call"
        | "tool_search_call"
        | "tool_search_output"
        | "program"
        | "program_output" => {
            request.mark(Capability::HostedTools);
        }
        other => {
            request.mark(Capability::Unknown);
            if !other.is_empty() {
                // Preserve the fact that something native-only was present without trying
                // to reinterpret it as a normal message.
            }
        }
    }
    Ok(())
}

fn parse_input_content(
    value: Option<&Value>,
    role: Role,
    request: &mut CanonicalRequest,
) -> Result<Vec<ContentPart>, ProtocolError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    match value {
        Value::String(text) => Ok(vec![ContentPart::text(text)]),
        Value::Array(parts) => {
            let mut out = Vec::new();
            for part in parts {
                let object = part.as_object().ok_or_else(|| {
                    ProtocolError::invalid("input content part must be an object")
                })?;
                match object.get("type").and_then(Value::as_str).unwrap_or("") {
                    "input_text" | "output_text" => out.push(ContentPart::text(
                        object
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    )),
                    "refusal" => out.push(ContentPart::Refusal {
                        refusal: object
                            .get("refusal")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    }),
                    "input_image" => {
                        let file_id = object
                            .get("file_id")
                            .and_then(Value::as_str)
                            .map(str::to_string);
                        if file_id.is_some() {
                            request.mark(Capability::ImageFileId);
                        }
                        out.push(ContentPart::Image {
                            url: object
                                .get("image_url")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            data: None,
                            file_id,
                            media_type: None,
                            detail: object
                                .get("detail")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                        });
                    }
                    "input_file" => {
                        let file_url = object
                            .get("file_url")
                            .and_then(Value::as_str)
                            .map(str::to_string);
                        if file_url.is_some() {
                            request.mark(Capability::FileUrl);
                        }
                        out.push(ContentPart::File {
                            file_data: object
                                .get("file_data")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            file_id: object
                                .get("file_id")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            file_url,
                            filename: object
                                .get("filename")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            media_type: None,
                        });
                    }
                    "input_audio" => {
                        request.mark(Capability::Audio);
                        let audio = object.get("input_audio").cloned().unwrap_or(Value::Null);
                        out.push(ContentPart::Audio {
                            data: audio
                                .get("data")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                            format: audio
                                .get("format")
                                .and_then(Value::as_str)
                                .unwrap_or("wav")
                                .to_string(),
                            transcript: None,
                        });
                    }
                    _ => {
                        request.mark(Capability::Unknown);
                        out.push(ContentPart::Unknown {
                            protocol: Protocol::Responses,
                            value: part.clone(),
                        });
                    }
                }
            }
            Ok(out)
        }
        _ => {
            let _ = role;
            Err(ProtocolError::invalid(
                "input message content must be a string or array",
            ))
        }
    }
}

fn parse_tool_output(
    value: Option<&Value>,
    request: &mut CanonicalRequest,
) -> Result<Vec<ContentPart>, ProtocolError> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(text)) => Ok(vec![ContentPart::text(text)]),
        Some(Value::Array(parts)) => {
            let mut out = Vec::new();
            for part in parts {
                let object = part
                    .as_object()
                    .ok_or_else(|| ProtocolError::invalid("tool output part must be an object"))?;
                match object.get("type").and_then(Value::as_str).unwrap_or("") {
                    "input_text" | "output_text" => out.push(ContentPart::text(
                        object
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    )),
                    "input_image" => {
                        request.mark(Capability::ToolResultMedia);
                        out.push(ContentPart::Image {
                            url: object
                                .get("image_url")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            data: None,
                            file_id: object
                                .get("file_id")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            media_type: None,
                            detail: object
                                .get("detail")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                        });
                    }
                    "input_file" => {
                        request.mark(Capability::ToolResultMedia);
                        out.push(ContentPart::File {
                            file_data: object
                                .get("file_data")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            file_id: object
                                .get("file_id")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            file_url: object
                                .get("file_url")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            filename: object
                                .get("filename")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            media_type: None,
                        });
                    }
                    _ => {
                        request.mark(Capability::Unknown);
                        out.push(ContentPart::Unknown {
                            protocol: Protocol::Responses,
                            value: part.clone(),
                        });
                    }
                }
            }
            Ok(out)
        }
        Some(other) => Ok(vec![ContentPart::text(other.to_string())]),
    }
}

fn decode_tool(value: &Value, request: &mut CanonicalRequest) -> Result<ToolDef, ProtocolError> {
    let object = value
        .as_object()
        .ok_or_else(|| ProtocolError::invalid("tool must be an object"))?;
    match object.get("type").and_then(Value::as_str).unwrap_or("") {
        "function" => {
            let name = object
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| ProtocolError::invalid("function tool name is required"))?;
            if object.get("output_schema").is_some() {
                request.mark(Capability::Unknown);
            }
            Ok(ToolDef::Function {
                name: name.to_string(),
                description: object
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                parameters: object
                    .get("parameters")
                    .cloned()
                    .unwrap_or_else(|| json!({"type": "object", "properties": {}})),
                strict: object.get("strict").and_then(Value::as_bool),
                output_schema: object.get("output_schema").cloned(),
            })
        }
        "custom" => {
            request.mark(Capability::CustomToolGrammar);
            Ok(ToolDef::Custom {
                name: object
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ProtocolError::invalid("custom tool name is required"))?
                    .to_string(),
                description: object
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                format: object.get("format").cloned(),
            })
        }
        _ => {
            request.mark(Capability::HostedTools);
            Ok(ToolDef::Hosted {
                protocol: Protocol::Responses,
                kind: object
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("hosted")
                    .to_string(),
                raw: value.clone(),
            })
        }
    }
}

fn decode_tool_choice(
    value: &Value,
    request: &mut CanonicalRequest,
) -> Result<ToolChoice, ProtocolError> {
    if let Some(value) = value.as_str() {
        return match value {
            "auto" => Ok(ToolChoice::Auto),
            "none" => Ok(ToolChoice::None),
            "required" => Ok(ToolChoice::Required),
            other => Err(ProtocolError::invalid(format!(
                "unsupported tool_choice `{other}`"
            ))),
        };
    }
    let object = value
        .as_object()
        .ok_or_else(|| ProtocolError::invalid("tool_choice must be a string or object"))?;
    match object.get("type").and_then(Value::as_str).unwrap_or("") {
        "function" => Ok(ToolChoice::Function {
            name: object
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| ProtocolError::invalid("tool_choice.name is required"))?
                .to_string(),
        }),
        "custom" => {
            request.mark(Capability::CustomToolGrammar);
            Ok(ToolChoice::Custom {
                name: object
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ProtocolError::invalid("tool_choice.name is required"))?
                    .to_string(),
            })
        }
        "allowed_tools" => {
            request.mark(Capability::AllowedTools);
            Ok(ToolChoice::Auto)
        }
        other => {
            request.mark(Capability::HostedTools);
            Ok(ToolChoice::Hosted {
                protocol: Protocol::Responses,
                raw: json!({"type": other, "raw": value}),
            })
        }
    }
}

fn decode_response_format(value: &Value) -> Result<ResponseFormat, ProtocolError> {
    let object = value
        .as_object()
        .ok_or_else(|| ProtocolError::invalid("text.format must be an object"))?;
    match object.get("type").and_then(Value::as_str).unwrap_or("text") {
        "text" => Ok(ResponseFormat::Text),
        "json_object" => Ok(ResponseFormat::JsonObject),
        "json_schema" => Ok(ResponseFormat::JsonSchema {
            name: object
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("response")
                .to_string(),
            description: object
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_string),
            schema: object
                .get("schema")
                .cloned()
                .unwrap_or_else(|| json!({"type": "object"})),
            strict: object.get("strict").and_then(Value::as_bool),
        }),
        other => Err(ProtocolError::invalid(format!(
            "unsupported text.format `{other}`"
        ))),
    }
}

pub fn encode_request(request: &CanonicalRequest) -> Result<Value, ProtocolError> {
    let mut object = Map::new();
    object.insert("model".to_string(), Value::String(request.model.clone()));

    if !request.system.is_empty() {
        let text = request
            .system
            .iter()
            .map(SystemBlock::as_text)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        if !text.is_empty() {
            object.insert("instructions".to_string(), Value::String(text));
        }
    }

    let mut input = Vec::new();
    for message in &request.messages {
        match message.role {
            Role::Tool => {
                for part in &message.content {
                    if let ContentPart::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } = part
                    {
                        input.push(json!({
                            "type": "function_call_output",
                            "call_id": tool_use_id,
                            "output": tool_output_to_responses(content)?,
                        }));
                    }
                }
            }
            Role::User | Role::Assistant | Role::System | Role::Developer => {
                let role = match message.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::System => "system",
                    Role::Developer => "developer",
                    Role::Tool => unreachable!(),
                };
                let mut content = Vec::new();
                let mut tool_calls = Vec::new();
                for part in &message.content {
                    match part {
                        ContentPart::ToolUse {
                            id,
                            name,
                            input,
                            raw_arguments,
                        } => tool_calls.push(json!({
                            "type": "function_call",
                            "call_id": id,
                            "name": name,
                            "arguments": tool_arguments_string(input, raw_arguments.as_deref()),
                        })),
                        _ => content.push(content_part_to_responses(part, message.role)?),
                    }
                }
                if !content.is_empty() {
                    let content_value = if content.len() == 1
                        && content[0].get("type").and_then(Value::as_str) == Some("input_text")
                    {
                        content[0]
                            .get("text")
                            .cloned()
                            .unwrap_or_else(|| Value::String(String::new()))
                    } else {
                        Value::Array(content)
                    };
                    input.push(json!({
                        "type": "message",
                        "role": role,
                        "content": content_value,
                    }));
                }
                input.extend(tool_calls);
            }
        }
    }
    object.insert("input".to_string(), Value::Array(input));

    if !request.tools.is_empty() {
        let mut tools = Vec::new();
        for tool in &request.tools {
            match tool {
                ToolDef::Function {
                    name,
                    description,
                    parameters,
                    strict,
                    ..
                } => {
                    let mut value = Map::new();
                    value.insert("type".to_string(), Value::String("function".to_string()));
                    value.insert("name".to_string(), Value::String(name.clone()));
                    if let Some(description) = description {
                        value.insert(
                            "description".to_string(),
                            Value::String(description.clone()),
                        );
                    }
                    value.insert("parameters".to_string(), parameters.clone());
                    if let Some(strict) = strict {
                        value.insert("strict".to_string(), Value::Bool(*strict));
                    }
                    tools.push(Value::Object(value));
                }
                ToolDef::Custom {
                    name,
                    description,
                    format,
                } => {
                    let mut value = Map::new();
                    value.insert("type".to_string(), Value::String("custom".to_string()));
                    value.insert("name".to_string(), Value::String(name.clone()));
                    if let Some(description) = description {
                        value.insert(
                            "description".to_string(),
                            Value::String(description.clone()),
                        );
                    }
                    if let Some(format) = format {
                        value.insert("format".to_string(), format.clone());
                    }
                    tools.push(Value::Object(value));
                }
                ToolDef::Hosted { .. } => {
                    return Err(ProtocolError::unsupported(
                        "hosted_tools",
                        request.source.as_str(),
                        Protocol::Responses.as_str(),
                    ));
                }
            }
        }
        object.insert("tools".to_string(), Value::Array(tools));
    }

    if !request.tools.is_empty() || request.tool_choice != ToolChoice::Auto {
        object.insert(
            "tool_choice".to_string(),
            encode_tool_choice(&request.tool_choice)?,
        );
    }
    if let Some(parallel) = request.parallel_tool_calls {
        object.insert("parallel_tool_calls".to_string(), Value::Bool(parallel));
    }
    if let Some(value) = request.max_output_tokens {
        object.insert("max_output_tokens".to_string(), json!(value));
    }
    if let Some(value) = request.temperature {
        object.insert("temperature".to_string(), json!(value));
    }
    if let Some(value) = request.top_p {
        object.insert("top_p".to_string(), json!(value));
    }
    if request.stream {
        object.insert("stream".to_string(), Value::Bool(true));
    }
    if let Some(value) = &request.reasoning.effort {
        object.insert("reasoning".to_string(), json!({"effort": value}));
    }
    if !request.metadata.is_empty() {
        object.insert("metadata".to_string(), json!(request.metadata));
    }
    if let Some(value) = &request.user {
        object.insert("user".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = &request.safety_identifier {
        object.insert(
            "safety_identifier".to_string(),
            Value::String(value.clone()),
        );
    }
    if let Some(value) = &request.service_tier {
        object.insert("service_tier".to_string(), Value::String(value.clone()));
    }
    if request.response_format != ResponseFormat::Text {
        object.insert(
            "text".to_string(),
            json!({"format": encode_response_format(&request.response_format)?}),
        );
    }
    if let Some(value) = request.state.store {
        object.insert("store".to_string(), Value::Bool(value));
    }
    if let Some(value) = &request.state.previous_response_id {
        object.insert(
            "previous_response_id".to_string(),
            Value::String(value.clone()),
        );
    }
    if let Some(value) = &request.state.conversation {
        object.insert("conversation".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = request.state.background {
        object.insert("background".to_string(), Value::Bool(value));
    }
    if !request.state.include.is_empty() {
        object.insert("include".to_string(), json!(request.state.include));
    }
    if let Some(value) = &request.state.truncation {
        object.insert("truncation".to_string(), Value::String(value.clone()));
    }

    Ok(Value::Object(object))
}

fn content_part_to_responses(part: &ContentPart, role: Role) -> Result<Value, ProtocolError> {
    Ok(match part {
        ContentPart::Text { text } => {
            if role == Role::Assistant {
                json!({"type": "output_text", "text": text, "annotations": [], "logprobs": []})
            } else {
                json!({"type": "input_text", "text": text})
            }
        }
        ContentPart::Refusal { refusal } => json!({"type": "refusal", "refusal": refusal}),
        ContentPart::Image {
            url,
            data,
            file_id,
            media_type,
            detail,
        } => {
            let data_url = data.as_ref().map(|data| {
                format!(
                    "data:{};base64,{}",
                    media_type.as_deref().unwrap_or("image/png"),
                    data
                )
            });
            let image_url = url.as_ref().or(data_url.as_ref());
            json!({
                "type": "input_image",
                "image_url": image_url,
                "file_id": file_id,
                "detail": detail.clone().unwrap_or_else(|| "auto".to_string()),
            })
        }
        ContentPart::File {
            file_data,
            file_id,
            file_url,
            filename,
            ..
        } => json!({
            "type": "input_file",
            "file_data": file_data,
            "file_id": file_id,
            "file_url": file_url,
            "filename": filename,
        }),
        ContentPart::Audio { data, format, .. } => json!({
            "type": "input_audio",
            "input_audio": {"data": data, "format": format},
        }),
        ContentPart::Thinking { .. } => {
            return Err(ProtocolError::unsupported(
                "thinking_content",
                "unknown",
                Protocol::Responses.as_str(),
            ));
        }
        ContentPart::ToolResult { .. } | ContentPart::ToolUse { .. } => {
            return Err(ProtocolError::invalid(
                "tool content must be emitted as a dedicated input item",
            ));
        }
        ContentPart::Unknown { .. } => {
            return Err(ProtocolError::unsupported(
                "unknown_content",
                "unknown",
                Protocol::Responses.as_str(),
            ));
        }
    })
}

fn tool_output_to_responses(content: &[ContentPart]) -> Result<Value, ProtocolError> {
    if content.len() == 1
        && let ContentPart::Text { text } = &content[0]
    {
        return Ok(Value::String(text.clone()));
    }
    let mut parts = Vec::new();
    for part in content {
        parts.push(content_part_to_responses(part, Role::Tool)?);
    }
    Ok(Value::Array(parts))
}

fn encode_tool_choice(choice: &ToolChoice) -> Result<Value, ProtocolError> {
    Ok(match choice {
        ToolChoice::Auto => Value::String("auto".to_string()),
        ToolChoice::None => Value::String("none".to_string()),
        ToolChoice::Required => Value::String("required".to_string()),
        ToolChoice::Function { name } => json!({"type": "function", "name": name}),
        ToolChoice::Custom { name } => json!({"type": "custom", "name": name}),
        ToolChoice::Hosted { .. } => {
            return Err(ProtocolError::unsupported(
                "hosted_tool_choice",
                "unknown",
                Protocol::Responses.as_str(),
            ));
        }
    })
}

fn encode_response_format(format: &ResponseFormat) -> Result<Value, ProtocolError> {
    Ok(match format {
        ResponseFormat::Text => json!({"type": "text"}),
        ResponseFormat::JsonObject => json!({"type": "json_object"}),
        ResponseFormat::JsonSchema {
            name,
            description,
            schema,
            strict,
        } => json!({
            "type": "json_schema",
            "name": name,
            "description": description,
            "schema": schema,
            "strict": strict,
        }),
    })
}

pub fn decode_response(value: &Value) -> Result<CanonicalResponse, ProtocolError> {
    let object = as_object(value)?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("resp_unknown");
    let model = object
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let mut response = CanonicalResponse::new(Protocol::Responses, id, model);
    response.created_at =
        optional_i64(object, "created_at").unwrap_or_else(|| chrono::Utc::now().timestamp());
    response.service_tier = optional_str(object, "service_tier");
    response.usage = decode_usage(object.get("usage"));
    response.metadata = object
        .get("metadata")
        .and_then(Value::as_object)
        .map(|metadata| {
            metadata
                .iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();

    let status = object
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("completed");
    response.status = match status {
        "completed" => ResponseStatus::Completed,
        "incomplete" => ResponseStatus::Incomplete,
        "failed" => ResponseStatus::Failed,
        "in_progress" => ResponseStatus::InProgress,
        "cancelled" => ResponseStatus::Cancelled,
        "queued" => ResponseStatus::Queued,
        _ => ResponseStatus::Completed,
    };
    if response.status == ResponseStatus::Incomplete {
        let reason = object
            .get("incomplete_details")
            .and_then(|value| value.get("reason"))
            .and_then(Value::as_str);
        response.incomplete_reason = reason.map(str::to_string);
        response.stop_reason = match reason {
            Some("max_output_tokens") => StopReason::MaxTokens,
            Some("content_filter") => StopReason::ContentFilter,
            Some(other) => StopReason::Unknown(other.to_string()),
            None => StopReason::Unknown("incomplete".to_string()),
        };
    }
    if let Some(error) = object.get("error").and_then(Value::as_object) {
        let error = ProtocolError::upstream(
            502,
            error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("upstream response failed"),
            error
                .get("code")
                .and_then(Value::as_str)
                .map(str::to_string),
        );
        response.error = Some(error);
        if response.status == ResponseStatus::Failed {
            response.stop_reason = StopReason::Unknown("failed".to_string());
        }
    }

    if let Some(output) = object.get("output").and_then(Value::as_array) {
        for item in output {
            decode_output_item(item, &mut response)?;
        }
    }
    if response.stop_reason == StopReason::EndTurn
        && response
            .output
            .iter()
            .any(|item| matches!(item, OutputItem::ToolCall { .. }))
    {
        response.stop_reason = StopReason::ToolUse;
    }
    Ok(response)
}

fn decode_output_item(
    value: &Value,
    response: &mut CanonicalResponse,
) -> Result<(), ProtocolError> {
    let item = value
        .as_object()
        .ok_or_else(|| ProtocolError::invalid("output items must be objects"))?;
    match item.get("type").and_then(Value::as_str).unwrap_or("") {
        "message" => {
            let content = parse_response_content(item.get("content"))?;
            let annotations = item
                .get("content")
                .and_then(Value::as_array)
                .map(|parts| {
                    parts
                        .iter()
                        .flat_map(|part| {
                            part.get("annotations")
                                .and_then(Value::as_array)
                                .cloned()
                                .unwrap_or_default()
                        })
                        .collect()
                })
                .unwrap_or_default();
            response.output.push(OutputItem::Message {
                id: item
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(generated_message_id),
                role: Role::Assistant,
                content,
                annotations,
            });
        }
        "function_call" => {
            let call_id = item
                .get("call_id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(generated_tool_id);
            response.output.push(OutputItem::ToolCall {
                id: item
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("fc_{}", uuid::Uuid::now_v7().simple())),
                call_id,
                name: item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("function")
                    .to_string(),
                arguments: item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}")
                    .to_string(),
                status: item
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("completed")
                    .to_string(),
            });
        }
        "reasoning" => {
            let summary = item
                .get("summary")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.get("text").and_then(Value::as_str))
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let content = item
                .get("content")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.get("text").and_then(Value::as_str))
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            response.output.push(OutputItem::Reasoning {
                id: item
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(generated_message_id),
                summary,
                content,
                encrypted_content: item
                    .get("encrypted_content")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                signature: None,
            });
        }
        kind => {
            response.output.push(OutputItem::Hosted {
                protocol: Protocol::Responses,
                kind: kind.to_string(),
                raw: value.clone(),
            });
        }
    }
    Ok(())
}

fn parse_response_content(value: Option<&Value>) -> Result<Vec<ContentPart>, ProtocolError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Value::Array(parts) = value else {
        return Err(ProtocolError::invalid(
            "output message content must be an array",
        ));
    };
    let mut out = Vec::new();
    for part in parts {
        let object = part
            .as_object()
            .ok_or_else(|| ProtocolError::invalid("output content part must be an object"))?;
        match object.get("type").and_then(Value::as_str).unwrap_or("") {
            "output_text" => out.push(ContentPart::text(
                object
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )),
            "refusal" => out.push(ContentPart::Refusal {
                refusal: object
                    .get("refusal")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            }),
            _ => out.push(ContentPart::Unknown {
                protocol: Protocol::Responses,
                value: part.clone(),
            }),
        }
    }
    Ok(out)
}

pub fn encode_response(response: &CanonicalResponse) -> Result<Value, ProtocolError> {
    let id = if response.source == Protocol::Responses {
        response.id.clone()
    } else {
        generated_response_id()
    };
    let mut output = Vec::new();
    for item in &response.output {
        match item {
            OutputItem::Message {
                id,
                role,
                content,
                annotations,
            } => {
                let parts = content
                    .iter()
                    .map(|part| match part {
                        ContentPart::Text { text } => json!({
                            "type": "output_text",
                            "text": text,
                            "annotations": annotations,
                            "logprobs": [],
                        }),
                        ContentPart::Refusal { refusal } => {
                            json!({"type": "refusal", "refusal": refusal})
                        }
                        _ => json!({"type": "output_text", "text": "", "annotations": [], "logprobs": []}),
                    })
                    .collect::<Vec<_>>();
                output.push(json!({
                    "type": "message",
                    "id": id,
                    "role": role,
                    "status": if response.status == ResponseStatus::Completed { "completed" } else { "incomplete" },
                    "content": parts,
                }));
            }
            OutputItem::ToolCall {
                id,
                call_id,
                name,
                arguments,
                status,
            } => {
                output.push(json!({
                    "type": "function_call",
                    "id": if id.is_empty() { format!("fc_{}", uuid::Uuid::now_v7().simple()) } else { id.clone() },
                    "call_id": call_id,
                    "name": name,
                    "arguments": arguments,
                    "status": if status.is_empty() { "completed" } else { status },
                }));
            }
            OutputItem::Reasoning {
                id,
                summary,
                content,
                encrypted_content,
                ..
            } => {
                let summary = summary
                    .iter()
                    .map(|text| json!({"type": "summary_text", "text": text}))
                    .collect::<Vec<_>>();
                let content = content
                    .iter()
                    .map(|text| json!({"type": "reasoning_text", "text": text}))
                    .collect::<Vec<_>>();
                output.push(json!({
                    "type": "reasoning",
                    "id": id,
                    "summary": summary,
                    "content": content,
                    "encrypted_content": encrypted_content,
                    "status": "completed",
                }));
            }
            OutputItem::Hosted { raw, .. } => output.push(raw.clone()),
        }
    }

    let incomplete_details = if response.status == ResponseStatus::Incomplete {
        json!({"reason": response.incomplete_reason.clone().unwrap_or_else(|| "max_output_tokens".to_string())})
    } else {
        Value::Null
    };
    let usage = if response.usage.is_empty() {
        Value::Null
    } else {
        encode_usage(&response.usage)
    };

    Ok(json!({
        "id": id,
        "object": "response",
        "created_at": response.created_at,
        "status": response.status.as_str(),
        "error": response.error.as_ref().map(|error| json!({
            "code": error.code.clone().unwrap_or_else(|| "server_error".to_string()),
            "message": error.message,
            "param": error.param,
            "type": error.kind.openai_type(),
        })),
        "incomplete_details": incomplete_details,
        "instructions": null,
        "max_output_tokens": null,
        "model": response.model,
        "output": output,
        "parallel_tool_calls": null,
        "previous_response_id": null,
        "reasoning": null,
        "store": false,
        "temperature": null,
        "text": {"format": {"type": "text"}},
        "tool_choice": "auto",
        "tools": [],
        "top_p": null,
        "truncation": "disabled",
        "usage": usage,
        "user": null,
        "metadata": response.metadata,
        "service_tier": response.service_tier,
        "completed_at": if response.status == ResponseStatus::Completed { Some(chrono::Utc::now().timestamp()) } else { None },
    }))
}

pub fn decode_usage(value: Option<&Value>) -> Usage {
    let Some(usage) = value.and_then(Value::as_object) else {
        return Usage::default();
    };
    let input_details = usage.get("input_tokens_details").and_then(Value::as_object);
    let output_details = usage
        .get("output_tokens_details")
        .and_then(Value::as_object);
    Usage {
        input_tokens: usage.get("input_tokens").and_then(Value::as_u64),
        output_tokens: usage.get("output_tokens").and_then(Value::as_u64),
        total_tokens: usage.get("total_tokens").and_then(Value::as_u64),
        cached_tokens: input_details
            .and_then(|details| details.get("cached_tokens"))
            .and_then(Value::as_u64),
        cache_write_tokens: input_details
            .and_then(|details| details.get("cache_write_tokens"))
            .and_then(Value::as_u64),
        reasoning_tokens: output_details
            .and_then(|details| details.get("reasoning_tokens"))
            .and_then(Value::as_u64),
        server_tool_use: None,
    }
}

pub fn encode_usage(usage: &Usage) -> Value {
    let mut value = json!({
        "input_tokens": usage.input_tokens.unwrap_or(0),
        "output_tokens": usage.output_tokens.unwrap_or(0),
        "total_tokens": usage.total_or_sum().unwrap_or(0),
        "input_tokens_details": {
            "cached_tokens": usage.cached_tokens.unwrap_or(0),
            "cache_write_tokens": usage.cache_write_tokens.unwrap_or(0),
        },
        "output_tokens_details": {
            "reasoning_tokens": usage.reasoning_tokens.unwrap_or(0),
        },
    });
    if let Some(server_tool_use) = &usage.server_tool_use {
        value["server_tool_use"] = server_tool_use.clone();
    }
    value
}
