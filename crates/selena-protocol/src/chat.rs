use crate::error::ProtocolError;
use crate::ir::*;
use crate::util::*;
use serde_json::{Map, Value, json};

pub fn decode_request(value: &Value) -> Result<CanonicalRequest, ProtocolError> {
    let object = as_object(value)?;
    let model = required_str(object, "model")?;
    let mut request = CanonicalRequest::new(Protocol::Chat, model);

    let messages = object
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| ProtocolError::invalid("`messages` must be an array"))?;

    for message_value in messages {
        let message = message_value
            .as_object()
            .ok_or_else(|| ProtocolError::invalid("each message must be an object"))?;
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| ProtocolError::invalid("message role is required"))?;

        match role {
            "system" | "developer" => {
                let role = if role == "developer" {
                    Role::Developer
                } else {
                    Role::System
                };
                let content = parse_content(message.get("content"), ContentContext::System)?;
                request.system.push(SystemBlock { role, content });
            }
            "user" => {
                let mut msg = Message {
                    role: Role::User,
                    content: parse_content(message.get("content"), ContentContext::User)?,
                    name: optional_str(message, "name"),
                    tool_call_id: None,
                };
                if let Some(name) = message.get("name").and_then(Value::as_str) {
                    msg.name = Some(name.to_string());
                }
                request.messages.push(msg);
            }
            "assistant" => {
                let mut content = parse_content(message.get("content"), ContentContext::Assistant)?;
                if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
                    for call in tool_calls {
                        if let Some(part) = decode_tool_call(call, &mut request)? {
                            content.push(part);
                        }
                    }
                }
                if let Some(function_call) = message.get("function_call").and_then(Value::as_object)
                {
                    let name = function_call
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("function");
                    let raw = function_call
                        .get("arguments")
                        .and_then(Value::as_str)
                        .unwrap_or("{}");
                    let (input, raw_arguments) = parse_tool_arguments(raw);
                    content.push(ContentPart::ToolUse {
                        id: generated_tool_id(),
                        name: name.to_string(),
                        input,
                        raw_arguments,
                    });
                }
                if message.get("audio").is_some() {
                    request.mark(Capability::Audio);
                }
                request.messages.push(Message {
                    role: Role::Assistant,
                    content,
                    name: optional_str(message, "name"),
                    tool_call_id: None,
                });
            }
            "tool" => {
                let tool_use_id = required_str(message, "tool_call_id")?.to_string();
                let content = parse_content(message.get("content"), ContentContext::ToolResult)?;
                if content.iter().any(|part| {
                    !matches!(part, ContentPart::Text { .. } | ContentPart::Refusal { .. })
                }) {
                    request.mark(Capability::ToolResultMedia);
                }
                request.messages.push(Message {
                    role: Role::Tool,
                    content: vec![ContentPart::ToolResult {
                        tool_use_id,
                        content,
                        is_error: false,
                    }],
                    name: optional_str(message, "name"),
                    tool_call_id: None,
                });
            }
            "function" => {
                let name = required_str(message, "name")?.to_string();
                let content = parse_content(message.get("content"), ContentContext::ToolResult)?;
                request.messages.push(Message {
                    role: Role::Tool,
                    content: vec![ContentPart::ToolResult {
                        tool_use_id: name.clone(),
                        content,
                        is_error: false,
                    }],
                    name: Some(name),
                    tool_call_id: None,
                });
            }
            other => {
                return Err(ProtocolError::invalid(format!(
                    "unsupported chat message role `{other}`"
                )));
            }
        }
    }

    if let Some(tools) = object.get("tools").and_then(Value::as_array) {
        for tool in tools {
            let decoded = decode_tool(tool, &mut request)?;
            request.tools.push(decoded);
        }
    }
    if let Some(functions) = object.get("functions").and_then(Value::as_array) {
        for function in functions {
            let decoded = decode_function_tool(function, false, &mut request)?;
            request.tools.push(decoded);
        }
    }

    if let Some(choice) = object.get("tool_choice") {
        request.tool_choice = decode_tool_choice(choice, &mut request)?;
    }
    if let Some(format) = object.get("response_format") {
        request.response_format = decode_response_format(format)?;
    }
    request.parallel_tool_calls = optional_bool(object, "parallel_tool_calls");
    request.max_output_tokens = optional_u32(object, "max_completion_tokens")
        .or_else(|| optional_u32(object, "max_tokens"));
    request.temperature = optional_f64(object, "temperature");
    request.top_p = optional_f64(object, "top_p");
    request.stop = string_or_array(object, "stop");
    request.stream = optional_bool(object, "stream").unwrap_or(false);
    request.user = optional_str(object, "user");
    request.safety_identifier = optional_str(object, "safety_identifier");
    request.service_tier = optional_str(object, "service_tier");
    request.presence_penalty = optional_f64(object, "presence_penalty");
    request.frequency_penalty = optional_f64(object, "frequency_penalty");
    request.seed = optional_i64(object, "seed");
    request.n = optional_u32(object, "n").unwrap_or(1).max(1);
    request.logprobs = optional_bool(object, "logprobs").unwrap_or(false);
    request.top_logprobs = optional_u32(object, "top_logprobs");
    request.cache.key = optional_str(object, "prompt_cache_key");
    request.cache.retention = optional_str(object, "prompt_cache_retention");
    request.reasoning.effort = optional_str(object, "reasoning_effort");
    request.state.store = optional_bool(object, "store");

    if request.n > 1 {
        request.mark(Capability::MultipleChoices);
    }
    if request.logprobs {
        request.mark(Capability::Logprobs);
    }
    if object.get("web_search_options").is_some() {
        request.mark(Capability::HostedTools);
    }
    if object.get("prediction").is_some() {
        request.mark(Capability::Prediction);
    }
    if object.get("moderation").is_some() {
        request.mark(Capability::Moderation);
    }
    if object
        .get("modalities")
        .and_then(Value::as_array)
        .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("audio")))
    {
        request.mark(Capability::Audio);
    }
    if object.get("metadata").and_then(Value::as_object).is_some() {
        for (key, value) in object["metadata"].as_object().unwrap() {
            if let Some(value) = value.as_str() {
                request.metadata.insert(key.clone(), value.to_string());
            }
        }
    }

    Ok(request)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentContext {
    System,
    User,
    Assistant,
    ToolResult,
}

fn parse_content(
    value: Option<&Value>,
    context: ContentContext,
) -> Result<Vec<ContentPart>, ProtocolError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    match value {
        Value::Null => Ok(Vec::new()),
        Value::String(text) => Ok(vec![ContentPart::text(text)]),
        Value::Array(parts) => {
            let mut out = Vec::new();
            for part in parts {
                let object = part
                    .as_object()
                    .ok_or_else(|| ProtocolError::invalid("content part must be an object"))?;
                let kind = object
                    .get("type")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ProtocolError::invalid("content part type is required"))?;
                match kind {
                    "text" => {
                        let text = object
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        out.push(ContentPart::text(text));
                    }
                    "refusal" => {
                        if context != ContentContext::Assistant {
                            return Err(ProtocolError::invalid(
                                "refusal content is only valid on assistant messages",
                            ));
                        }
                        out.push(ContentPart::Refusal {
                            refusal: object
                                .get("refusal")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                        });
                    }
                    "image_url" => {
                        if !matches!(context, ContentContext::User) {
                            return Err(ProtocolError::invalid(
                                "image content is only valid on user messages",
                            ));
                        }
                        let image = object.get("image_url").cloned().unwrap_or(Value::Null);
                        let (url, detail) = if let Some(url) = image.as_str() {
                            (Some(url.to_string()), None)
                        } else {
                            (
                                image.get("url").and_then(Value::as_str).map(str::to_string),
                                image
                                    .get("detail")
                                    .and_then(Value::as_str)
                                    .map(str::to_string),
                            )
                        };
                        out.push(ContentPart::Image {
                            url,
                            data: None,
                            file_id: None,
                            media_type: None,
                            detail,
                        });
                    }
                    "input_audio" => {
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
                    "file" => {
                        let file = object.get("file").cloned().unwrap_or(Value::Null);
                        out.push(ContentPart::File {
                            file_data: file
                                .get("file_data")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            file_id: file
                                .get("file_id")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            file_url: None,
                            filename: file
                                .get("filename")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            media_type: None,
                        });
                    }
                    "audio" => {
                        out.push(ContentPart::Unknown {
                            protocol: Protocol::Chat,
                            value: part.clone(),
                        });
                    }
                    _ => out.push(ContentPart::Unknown {
                        protocol: Protocol::Chat,
                        value: part.clone(),
                    }),
                }
            }
            Ok(out)
        }
        _ => Err(ProtocolError::invalid(
            "content must be a string, array, or null",
        )),
    }
}

fn decode_tool_call(
    value: &Value,
    request: &mut CanonicalRequest,
) -> Result<Option<ContentPart>, ProtocolError> {
    let object = value
        .as_object()
        .ok_or_else(|| ProtocolError::invalid("tool_calls entries must be objects"))?;
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("function");
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(generated_tool_id);
    match kind {
        "function" => {
            let function = object.get("function").cloned().unwrap_or(Value::Null);
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| ProtocolError::invalid("tool call function name is required"))?;
            let raw = function
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}");
            let (input, raw_arguments) = parse_tool_arguments(raw);
            Ok(Some(ContentPart::ToolUse {
                id,
                name: name.to_string(),
                input,
                raw_arguments,
            }))
        }
        "custom" => {
            request.mark(Capability::CustomToolGrammar);
            Ok(None)
        }
        _ => {
            request.mark(Capability::Unknown);
            Ok(None)
        }
    }
}

fn decode_tool(value: &Value, request: &mut CanonicalRequest) -> Result<ToolDef, ProtocolError> {
    let object = value
        .as_object()
        .ok_or_else(|| ProtocolError::invalid("tools entries must be objects"))?;
    match object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("function")
    {
        "function" => decode_function_tool(value, true, request),
        "custom" => {
            let custom = object.get("custom").cloned().unwrap_or(Value::Null);
            request.mark(Capability::CustomToolGrammar);
            Ok(ToolDef::Custom {
                name: custom
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ProtocolError::invalid("custom tool name is required"))?
                    .to_string(),
                description: custom
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                format: custom.get("format").cloned(),
            })
        }
        _ => {
            request.mark(Capability::HostedTools);
            Ok(ToolDef::Hosted {
                protocol: Protocol::Chat,
                kind: "hosted".to_string(),
                raw: value.clone(),
            })
        }
    }
}

fn decode_function_tool(
    value: &Value,
    nested: bool,
    request: &mut CanonicalRequest,
) -> Result<ToolDef, ProtocolError> {
    let object = value
        .as_object()
        .ok_or_else(|| ProtocolError::invalid("function tool must be an object"))?;
    let function = if nested {
        object.get("function").cloned().unwrap_or(Value::Null)
    } else {
        value.clone()
    };
    let name = function
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| ProtocolError::invalid("function tool name is required"))?;
    if function.get("output_schema").is_some() {
        request.mark(Capability::Unknown);
    }
    Ok(ToolDef::Function {
        name: name.to_string(),
        description: function
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string),
        parameters: function
            .get("parameters")
            .cloned()
            .unwrap_or_else(|| json!({"type": "object", "properties": {}})),
        strict: function.get("strict").and_then(Value::as_bool),
        output_schema: function.get("output_schema").cloned(),
    })
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
    match object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "function" => Ok(ToolChoice::Function {
            name: object
                .get("function")
                .and_then(|value| value.get("name"))
                .and_then(Value::as_str)
                .ok_or_else(|| ProtocolError::invalid("tool_choice function name is required"))?
                .to_string(),
        }),
        "custom" => {
            request.mark(Capability::CustomToolGrammar);
            Ok(ToolChoice::Custom {
                name: object
                    .get("custom")
                    .and_then(|value| value.get("name"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| ProtocolError::invalid("tool_choice custom name is required"))?
                    .to_string(),
            })
        }
        "allowed_tools" => {
            request.mark(Capability::AllowedTools);
            Ok(ToolChoice::Auto)
        }
        _ => {
            request.mark(Capability::HostedTools);
            Ok(ToolChoice::Hosted {
                protocol: Protocol::Chat,
                raw: value.clone(),
            })
        }
    }
}

fn decode_response_format(value: &Value) -> Result<ResponseFormat, ProtocolError> {
    let object = value
        .as_object()
        .ok_or_else(|| ProtocolError::invalid("response_format must be an object"))?;
    match object.get("type").and_then(Value::as_str).unwrap_or("text") {
        "text" => Ok(ResponseFormat::Text),
        "json_object" => Ok(ResponseFormat::JsonObject),
        "json_schema" => {
            let schema = object
                .get("json_schema")
                .and_then(Value::as_object)
                .ok_or_else(|| ProtocolError::invalid("response_format.json_schema is required"))?;
            Ok(ResponseFormat::JsonSchema {
                name: schema
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("response")
                    .to_string(),
                description: schema
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                schema: schema
                    .get("schema")
                    .cloned()
                    .unwrap_or_else(|| json!({"type": "object"})),
                strict: schema.get("strict").and_then(Value::as_bool),
            })
        }
        other => Err(ProtocolError::invalid(format!(
            "unsupported response_format `{other}`"
        ))),
    }
}

pub fn encode_request(request: &CanonicalRequest) -> Result<Value, ProtocolError> {
    let mut object = Map::new();
    object.insert("model".to_string(), Value::String(request.model.clone()));
    let mut messages = Vec::new();

    for system in &request.system {
        let role = if system.role == Role::Developer {
            "developer"
        } else {
            "system"
        };
        let mut text = String::new();
        for part in &system.content {
            match part {
                ContentPart::Text { text: value } => text.push_str(value),
                _ => {
                    return Err(ProtocolError::unsupported(
                        "non_text_system_content",
                        request.source.as_str(),
                        Protocol::Chat.as_str(),
                    ));
                }
            }
        }
        messages.push(json!({"role": role, "content": text}));
    }

    for message in &request.messages {
        match message.role {
            Role::System | Role::Developer => {
                let role = if message.role == Role::Developer {
                    "developer"
                } else {
                    "system"
                };
                messages.push(json!({
                    "role": role,
                    "content": content_to_chat_system(&message.content)?,
                }));
            }
            Role::User => {
                messages.push(json!({
                    "role": "user",
                    "content": content_to_chat(&message.content, false)?,
                }));
            }
            Role::Assistant => {
                let mut content_parts = Vec::new();
                let mut tool_calls = Vec::new();
                for part in &message.content {
                    match part {
                        ContentPart::Text { .. } | ContentPart::Refusal { .. } => {
                            content_parts.push(part.clone())
                        }
                        ContentPart::ToolUse {
                            id,
                            name,
                            input,
                            raw_arguments,
                        } => tool_calls.push(json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": tool_arguments_string(input, raw_arguments.as_deref()),
                            }
                        })),
                        ContentPart::Thinking { .. } => {}
                        other => {
                            return Err(ProtocolError::unsupported(
                                format!("assistant_content_{other:?}"),
                                request.source.as_str(),
                                Protocol::Chat.as_str(),
                            ));
                        }
                    }
                }
                let content = if content_parts.is_empty() {
                    Value::Null
                } else {
                    content_to_chat(&content_parts, true)?
                };
                let message_name = message.name.clone();
                let mut encoded = Map::new();
                encoded.insert("role".to_string(), Value::String("assistant".to_string()));
                encoded.insert("content".to_string(), content);
                if !tool_calls.is_empty() {
                    encoded.insert("tool_calls".to_string(), Value::Array(tool_calls));
                }
                if let Some(name) = message_name {
                    encoded.insert("name".to_string(), Value::String(name));
                }
                messages.push(Value::Object(encoded));
            }
            Role::Tool => {
                let tool_call_id = message
                    .tool_call_id
                    .clone()
                    .or_else(|| {
                        message.content.iter().find_map(|part| match part {
                            ContentPart::ToolResult { tool_use_id, .. } => {
                                Some(tool_use_id.clone())
                            }
                            _ => None,
                        })
                    })
                    .ok_or_else(|| {
                        ProtocolError::invalid("tool message is missing tool_call_id")
                    })?;
                let mut text = String::new();
                let mut has_media = false;
                for part in &message.content {
                    match part {
                        ContentPart::ToolResult { content, .. } => {
                            for item in content {
                                match item {
                                    ContentPart::Text { text: value } => text.push_str(value),
                                    ContentPart::Refusal { refusal } => text.push_str(refusal),
                                    _ => has_media = true,
                                }
                            }
                        }
                        ContentPart::Text { text: value } => text.push_str(value),
                        _ => has_media = true,
                    }
                }
                if has_media {
                    return Err(ProtocolError::unsupported(
                        "tool_result_media",
                        request.source.as_str(),
                        Protocol::Chat.as_str(),
                    ));
                }
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "content": text,
                }));
            }
        }
    }

    object.insert("messages".to_string(), Value::Array(messages));

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
                    let mut function = Map::new();
                    function.insert("name".to_string(), Value::String(name.clone()));
                    if let Some(description) = description {
                        function.insert(
                            "description".to_string(),
                            Value::String(description.clone()),
                        );
                    }
                    function.insert("parameters".to_string(), parameters.clone());
                    if let Some(strict) = strict {
                        function.insert("strict".to_string(), Value::Bool(*strict));
                    }
                    tools.push(json!({"type": "function", "function": function}));
                }
                ToolDef::Custom {
                    name,
                    description,
                    format,
                } => {
                    let mut custom = Map::new();
                    custom.insert("name".to_string(), Value::String(name.clone()));
                    if let Some(description) = description {
                        custom.insert(
                            "description".to_string(),
                            Value::String(description.clone()),
                        );
                    }
                    if let Some(format) = format {
                        custom.insert("format".to_string(), format.clone());
                    }
                    tools.push(json!({"type": "custom", "custom": custom}));
                }
                ToolDef::Hosted { .. } => {
                    return Err(ProtocolError::unsupported(
                        "hosted_tools",
                        request.source.as_str(),
                        Protocol::Chat.as_str(),
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
    if request.max_output_tokens.is_some() {
        object.insert(
            "max_completion_tokens".to_string(),
            json!(request.max_output_tokens),
        );
    }
    if let Some(value) = request.temperature {
        object.insert("temperature".to_string(), json!(value));
    }
    if let Some(value) = request.top_p {
        object.insert("top_p".to_string(), json!(value));
    }
    if !request.stop.is_empty() {
        object.insert("stop".to_string(), json!(request.stop));
    }
    if request.stream {
        object.insert("stream".to_string(), Value::Bool(true));
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
    if let Some(value) = &request.reasoning.effort {
        object.insert("reasoning_effort".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = &request.cache.key {
        object.insert("prompt_cache_key".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = request.presence_penalty {
        object.insert("presence_penalty".to_string(), json!(value));
    }
    if let Some(value) = request.frequency_penalty {
        object.insert("frequency_penalty".to_string(), json!(value));
    }
    if let Some(value) = request.seed {
        object.insert("seed".to_string(), json!(value));
    }
    if request.n > 1 {
        object.insert("n".to_string(), json!(request.n));
    }
    if request.logprobs {
        object.insert("logprobs".to_string(), Value::Bool(true));
    }
    if let Some(value) = request.top_logprobs {
        object.insert("top_logprobs".to_string(), json!(value));
    }
    if request.response_format != ResponseFormat::Text {
        object.insert(
            "response_format".to_string(),
            encode_response_format(&request.response_format)?,
        );
    }

    Ok(Value::Object(object))
}

fn content_to_chat_system(parts: &[ContentPart]) -> Result<Value, ProtocolError> {
    let mut texts = Vec::new();
    for part in parts {
        match part {
            ContentPart::Text { text } => texts.push(text.clone()),
            _ => return Err(ProtocolError::invalid("system content must be text")),
        }
    }
    if texts.len() == 1 {
        Ok(Value::String(texts.remove(0)))
    } else {
        Ok(Value::Array(
            texts
                .into_iter()
                .map(|text| json!({"type": "text", "text": text}))
                .collect(),
        ))
    }
}

fn content_to_chat(parts: &[ContentPart], assistant: bool) -> Result<Value, ProtocolError> {
    if parts.len() == 1
        && let ContentPart::Text { text } = &parts[0]
    {
        return Ok(Value::String(text.clone()));
    }
    let mut out = Vec::new();
    for part in parts {
        match part {
            ContentPart::Text { text } => out.push(json!({"type": "text", "text": text})),
            ContentPart::Refusal { refusal } if assistant => {
                out.push(json!({"type": "refusal", "refusal": refusal}))
            }
            ContentPart::Refusal { .. } => {
                return Err(ProtocolError::invalid(
                    "refusal content is only valid on assistant messages",
                ));
            }
            ContentPart::Image {
                url,
                data,
                file_id,
                media_type,
                detail,
            } => {
                if let Some(file_id) = file_id {
                    out.push(json!({
                        "type": "file",
                        "file": {"file_id": file_id}
                    }));
                } else {
                    let data_url = data.as_ref().map(|data| {
                        format!(
                            "data:{};base64,{}",
                            media_type.as_deref().unwrap_or("image/png"),
                            data
                        )
                    });
                    let image_url = url.as_ref().or(data_url.as_ref()).ok_or_else(|| {
                        ProtocolError::invalid("image content has no url, data, or file_id")
                    })?;
                    out.push(json!({
                        "type": "image_url",
                        "image_url": {"url": image_url, "detail": detail}
                    }));
                }
            }
            ContentPart::File {
                file_data,
                file_id,
                filename,
                ..
            } => {
                let mut file = Map::new();
                if let Some(value) = file_data {
                    file.insert("file_data".to_string(), Value::String(value.clone()));
                }
                if let Some(value) = file_id {
                    file.insert("file_id".to_string(), Value::String(value.clone()));
                }
                if let Some(value) = filename {
                    file.insert("filename".to_string(), Value::String(value.clone()));
                }
                out.push(json!({"type": "file", "file": file}));
            }
            ContentPart::Audio { data, format, .. } => out.push(json!({
                "type": "input_audio",
                "input_audio": {"data": data, "format": format}
            })),
            ContentPart::ToolResult { .. } => {
                return Err(ProtocolError::invalid(
                    "tool_result content must be carried by a tool message",
                ));
            }
            ContentPart::Thinking { .. } => {}
            ContentPart::ToolUse { .. } => {
                return Err(ProtocolError::invalid(
                    "tool_use content must be carried by tool_calls",
                ));
            }
            ContentPart::Unknown { .. } => {
                return Err(ProtocolError::unsupported(
                    "unknown_content",
                    "unknown",
                    Protocol::Chat.as_str(),
                ));
            }
        }
    }
    Ok(Value::Array(out))
}

fn encode_tool_choice(choice: &ToolChoice) -> Result<Value, ProtocolError> {
    Ok(match choice {
        ToolChoice::Auto => Value::String("auto".to_string()),
        ToolChoice::None => Value::String("none".to_string()),
        ToolChoice::Required => Value::String("required".to_string()),
        ToolChoice::Function { name } => json!({
            "type": "function",
            "function": {"name": name}
        }),
        ToolChoice::Custom { name } => json!({
            "type": "custom",
            "custom": {"name": name}
        }),
        ToolChoice::Hosted { .. } => {
            return Err(ProtocolError::unsupported(
                "hosted_tool_choice",
                "unknown",
                Protocol::Chat.as_str(),
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
            "json_schema": {
                "name": name,
                "description": description,
                "schema": schema,
                "strict": strict,
            }
        }),
    })
}

pub fn decode_response(value: &Value) -> Result<CanonicalResponse, ProtocolError> {
    let object = as_object(value)?;
    if let Some(error) = object.get("error").and_then(Value::as_object) {
        return Err(ProtocolError::upstream(
            502,
            error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("upstream chat completion failed"),
            error
                .get("code")
                .and_then(Value::as_str)
                .map(str::to_string),
        ));
    }
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("chatcmpl-unknown");
    let model = object
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let created = optional_i64(object, "created").unwrap_or_else(|| chrono::Utc::now().timestamp());
    let choice = object
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(Value::as_object)
        .ok_or_else(|| ProtocolError::stream("chat completion has no choices"))?;
    let message = choice
        .get("message")
        .and_then(Value::as_object)
        .ok_or_else(|| ProtocolError::stream("chat completion choice has no message"))?;
    let finish_reason = choice
        .get("finish_reason")
        .and_then(Value::as_str)
        .unwrap_or("stop");
    let mut response = CanonicalResponse::new(Protocol::Chat, id.to_string(), model.to_string());
    response.created_at = created;
    response.service_tier = optional_str(object, "service_tier");
    response.usage = decode_usage(object.get("usage"));

    let mut content = parse_content(message.get("content"), ContentContext::Assistant)?;
    if let Some(refusal) = message.get("refusal").and_then(Value::as_str) {
        content.push(ContentPart::Refusal {
            refusal: refusal.to_string(),
        });
    }
    let annotations = annotations_from_chat(message.get("annotations"));
    if !content.is_empty() || !annotations.is_empty() {
        response.output.push(OutputItem::Message {
            id: generated_message_id(),
            role: Role::Assistant,
            content,
            annotations,
        });
    }
    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        for call in tool_calls {
            if let Some(call) = decode_response_tool_call(call)? {
                response.output.push(call);
            }
        }
    }
    if let Some(function_call) = message.get("function_call").and_then(Value::as_object) {
        let name = function_call
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("function");
        let arguments = function_call
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or("{}");
        response.output.push(OutputItem::ToolCall {
            id: format!("fc_{}", uuid::Uuid::now_v7().simple()),
            call_id: generated_tool_id(),
            name: name.to_string(),
            arguments: arguments.to_string(),
            status: "completed".to_string(),
        });
    }

    match finish_reason {
        "stop" => {
            response.status = ResponseStatus::Completed;
            response.stop_reason = StopReason::EndTurn;
        }
        "tool_calls" | "function_call" => {
            response.status = ResponseStatus::Completed;
            response.stop_reason = StopReason::ToolUse;
        }
        "length" => {
            response.status = ResponseStatus::Incomplete;
            response.stop_reason = StopReason::MaxTokens;
            response.incomplete_reason = Some("max_output_tokens".to_string());
        }
        "content_filter" => {
            response.status = ResponseStatus::Incomplete;
            response.stop_reason = StopReason::ContentFilter;
            response.incomplete_reason = Some("content_filter".to_string());
        }
        other => {
            response.status = ResponseStatus::Completed;
            response.stop_reason = StopReason::Unknown(other.to_string());
        }
    }
    Ok(response)
}

fn decode_response_tool_call(value: &Value) -> Result<Option<OutputItem>, ProtocolError> {
    let object = value
        .as_object()
        .ok_or_else(|| ProtocolError::invalid("tool_calls entries must be objects"))?;
    if object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("function")
        != "function"
    {
        return Ok(None);
    }
    let function = object.get("function").cloned().unwrap_or(Value::Null);
    let name = function
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("function");
    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or("{}");
    let call_id = object
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(generated_tool_id);
    Ok(Some(OutputItem::ToolCall {
        id: format!("fc_{}", uuid::Uuid::now_v7().simple()),
        call_id,
        name: name.to_string(),
        arguments: arguments.to_string(),
        status: "completed".to_string(),
    }))
}

pub fn encode_response(response: &CanonicalResponse) -> Result<Value, ProtocolError> {
    let mut text = String::new();
    let mut refusal = String::new();
    let mut annotations = Vec::new();
    let mut tool_calls = Vec::new();
    for item in &response.output {
        match item {
            OutputItem::Message {
                content,
                annotations: item_annotations,
                ..
            } => {
                for part in content {
                    match part {
                        ContentPart::Text { text: value } => text.push_str(value),
                        ContentPart::Refusal { refusal: value } => refusal.push_str(value),
                        _ => {}
                    }
                }
                annotations.extend(item_annotations.iter().cloned());
            }
            OutputItem::ToolCall {
                call_id,
                name,
                arguments,
                ..
            } => tool_calls.push(json!({
                "id": call_id,
                "type": "function",
                "function": {"name": name, "arguments": arguments}
            })),
            OutputItem::Reasoning { .. } | OutputItem::Hosted { .. } => {}
        }
    }

    let mut message = Map::new();
    message.insert("role".to_string(), Value::String("assistant".to_string()));
    message.insert(
        "content".to_string(),
        if text.is_empty() {
            Value::Null
        } else {
            Value::String(text)
        },
    );
    if !refusal.is_empty() {
        message.insert("refusal".to_string(), Value::String(refusal));
    }
    if !tool_calls.is_empty() {
        message.insert("tool_calls".to_string(), Value::Array(tool_calls));
    }
    if !annotations.is_empty() {
        message.insert(
            "annotations".to_string(),
            Value::Array(annotations_to_chat(&annotations)),
        );
    }

    let effective_stop = response.effective_stop_reason();
    let finish_reason = if response.status == ResponseStatus::Incomplete
        && effective_stop == StopReason::MaxTokens
    {
        "length"
    } else {
        effective_stop.as_chat_finish_reason()
    };

    Ok(json!({
        "id": response.id,
        "object": "chat.completion",
        "created": response.created_at,
        "model": response.model,
        "choices": [{
            "index": 0,
            "message": Value::Object(message),
            "finish_reason": finish_reason,
            "logprobs": null,
        }],
        "usage": encode_usage(&response.usage),
        "service_tier": response.service_tier,
    }))
}

pub fn decode_usage(value: Option<&Value>) -> Usage {
    let Some(usage) = value.and_then(Value::as_object) else {
        return Usage::default();
    };
    let prompt_details = usage
        .get("prompt_tokens_details")
        .and_then(Value::as_object);
    let completion_details = usage
        .get("completion_tokens_details")
        .and_then(Value::as_object);
    Usage {
        input_tokens: usage.get("prompt_tokens").and_then(Value::as_u64),
        output_tokens: usage.get("completion_tokens").and_then(Value::as_u64),
        total_tokens: usage.get("total_tokens").and_then(Value::as_u64),
        cached_tokens: prompt_details
            .and_then(|details| details.get("cached_tokens"))
            .and_then(Value::as_u64),
        cache_write_tokens: prompt_details
            .and_then(|details| details.get("cache_write_tokens"))
            .and_then(Value::as_u64),
        reasoning_tokens: completion_details
            .and_then(|details| details.get("reasoning_tokens"))
            .and_then(Value::as_u64),
        server_tool_use: None,
    }
}

pub fn encode_usage(usage: &Usage) -> Value {
    let mut value = json!({
        "prompt_tokens": usage.input_tokens.unwrap_or(0),
        "completion_tokens": usage.output_tokens.unwrap_or(0),
        "total_tokens": usage.total_or_sum().unwrap_or(0),
    });
    let mut prompt_details = Map::new();
    if let Some(cached) = usage.cached_tokens {
        prompt_details.insert("cached_tokens".to_string(), json!(cached));
    }
    if let Some(write) = usage.cache_write_tokens {
        prompt_details.insert("cache_write_tokens".to_string(), json!(write));
    }
    if !prompt_details.is_empty() {
        value["prompt_tokens_details"] = Value::Object(prompt_details);
    }
    let mut completion_details = Map::new();
    if let Some(reasoning) = usage.reasoning_tokens {
        completion_details.insert("reasoning_tokens".to_string(), json!(reasoning));
    }
    if !completion_details.is_empty() {
        value["completion_tokens_details"] = Value::Object(completion_details);
    }
    value
}
