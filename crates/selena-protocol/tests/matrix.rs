use selena_protocol::chat;
use selena_protocol::convert;
use selena_protocol::ir::*;
use selena_protocol::messages;
use selena_protocol::responses;
use selena_protocol::sse::SseEvent;
use selena_protocol::stream::{decoder_for, encoder_for};
use serde_json::{Value, json};

fn encode_request(ir: &CanonicalRequest, target: Protocol) -> Value {
    let (value, _) = convert::request_to(ir, target).unwrap();
    value
}

fn decode_request(value: &Value, protocol: Protocol) -> CanonicalRequest {
    match protocol {
        Protocol::Chat => chat::decode_request(value).unwrap(),
        Protocol::Responses => responses::decode_request(value).unwrap(),
        Protocol::Messages => messages::decode_request(value).unwrap(),
    }
}

#[test]
fn request_text_matrix_round_trips() {
    let chat = json!({
        "model": "test-model",
        "messages": [
            {"role": "system", "content": "Be brief."},
            {"role": "user", "content": "Hello"}
        ],
        "max_completion_tokens": 128
    });
    let ir = chat::decode_request(&chat).unwrap();
    assert_eq!(ir.system[0].as_text(), "Be brief.");

    for target in [Protocol::Chat, Protocol::Responses, Protocol::Messages] {
        let encoded = encode_request(&ir, target);
        let decoded = decode_request(&encoded, target);
        assert_eq!(decoded.model, "test-model");
        assert_eq!(decoded.system[0].as_text(), "Be brief.");
        let user_text = decoded.messages[0]
            .content
            .first()
            .and_then(ContentPart::plain_text)
            .unwrap_or_default();
        assert_eq!(user_text, "Hello");
    }
}

#[test]
fn chat_tool_history_converts_to_responses_and_messages() {
    let chat = json!({
        "model": "test-model",
        "messages": [
            {"role": "user", "content": "weather?"},
            {"role": "assistant", "content": null, "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {"name": "get_weather", "arguments": "{\"city\":\"SF\"}"}
            }]},
            {"role": "tool", "tool_call_id": "call_1", "content": "sunny"}
        ],
        "tools": [{
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get weather",
                "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}
            }
        }]
    });
    let ir = chat::decode_request(&chat).unwrap();
    for target in [Protocol::Responses, Protocol::Messages] {
        let encoded = encode_request(&ir, target);
        let decoded = decode_request(&encoded, target);
        assert_eq!(decoded.tools.len(), 1);
        assert_eq!(decoded.tools[0].name(), Some("get_weather"));
        assert!(decoded.messages.iter().any(|message| {
            message
                .content
                .iter()
                .any(|part| matches!(part, ContentPart::ToolUse { id, .. } if id == "call_1"))
        }));
        assert!(decoded.messages.iter().any(|message| {
            message.role == Role::Tool
                && message.content.iter().any(|part| matches!(part, ContentPart::ToolResult { tool_use_id, .. } if tool_use_id == "call_1"))
        }));
    }
}

#[test]
fn responses_function_history_converts_to_chat() {
    let responses = json!({
        "model": "test-model",
        "instructions": "Use tools.",
        "input": [
            {"type": "message", "role": "user", "content": "weather?"},
            {"type": "function_call", "call_id": "call_1", "name": "get_weather", "arguments": "{\"city\":\"SF\"}"},
            {"type": "function_call_output", "call_id": "call_1", "output": "sunny"}
        ],
        "tools": [{
            "type": "function",
            "name": "get_weather",
            "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}
        }]
    });
    let ir = responses::decode_request(&responses).unwrap();
    let chat = chat::encode_request(&ir).unwrap();
    let messages = chat["messages"].as_array().unwrap();
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[0]["content"], "Use tools.");
    assert_eq!(messages[2]["tool_calls"][0]["id"], "call_1");
    assert_eq!(messages[3]["role"], "tool");
    assert_eq!(messages[3]["tool_call_id"], "call_1");
}

#[test]
fn messages_multimodal_and_tool_history_converts_to_chat() {
    let messages_payload = json!({
        "model": "test-model",
        "max_tokens": 128,
        "system": "Be useful.",
        "messages": [
            {"role": "user", "content": [
                {"type": "text", "text": "What is in this image?"},
                {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "aGVsbG8="}}
            ]},
            {"role": "assistant", "content": [
                {"type": "tool_use", "id": "toolu_1", "name": "lookup", "input": {"q": "x"}}
            ]},
            {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "toolu_1", "content": "result"}
            ]}
        ]
    });
    let ir = messages::decode_request(&messages_payload).unwrap();
    let chat = chat::encode_request(&ir).unwrap();
    assert_eq!(chat["messages"][0]["role"], "system");
    assert_eq!(
        chat["messages"][1]["content"][1]["image_url"]["url"],
        "data:image/png;base64,aGVsbG8="
    );
    assert_eq!(chat["messages"][2]["tool_calls"][0]["id"], "toolu_1");
    assert_eq!(chat["messages"][3]["role"], "tool");
}

#[test]
fn unsupported_native_features_fail_fast() {
    let responses = json!({
        "model": "test-model",
        "input": "hello",
        "previous_response_id": "resp_123"
    });
    let ir = responses::decode_request(&responses).unwrap();
    let error = convert::request_to(&ir, Protocol::Chat).unwrap_err();
    assert_eq!(error.code.as_deref(), Some("unsupported_feature"));

    let chat = json!({
        "model": "test-model",
        "messages": [{"role": "user", "content": "hello"}],
        "n": 2
    });
    let ir = chat::decode_request(&chat).unwrap();
    let error = convert::request_to(&ir, Protocol::Messages).unwrap_err();
    assert_eq!(error.code.as_deref(), Some("unsupported_feature"));
}

#[test]
fn cross_protocol_thinking_is_dropped_with_warning() {
    let messages_payload = json!({
        "model": "test-model",
        "max_tokens": 128,
        "thinking": {"type": "enabled", "budget_tokens": 1024},
        "messages": [
            {"role": "user", "content": "hello"},
            {"role": "assistant", "content": [
                {"type": "thinking", "thinking": "private reasoning", "signature": "sig_123"},
                {"type": "redacted_thinking", "data": "opaque"},
                {"type": "text", "text": "visible answer"}
            ]},
            {"role": "user", "content": "continue"}
        ]
    });
    let ir = messages::decode_request(&messages_payload).unwrap();

    let (chat, warnings) = convert::request_to(&ir, Protocol::Chat).unwrap();
    assert!(
        warnings
            .0
            .iter()
            .any(|warning| warning.contains("thinking"))
    );
    assert_eq!(chat["reasoning_effort"], "low");
    assert_eq!(chat["messages"][1]["content"], "visible answer");
    let encoded = chat.to_string();
    assert!(!encoded.contains("private reasoning"));
    assert!(!encoded.contains("sig_123"));

    let (responses, warnings) = convert::request_to(&ir, Protocol::Responses).unwrap();
    assert!(
        warnings
            .0
            .iter()
            .any(|warning| warning.contains("thinking"))
    );
    let encoded = responses.to_string();
    assert!(!encoded.contains("private reasoning"));
    assert!(!encoded.contains("opaque"));
}

#[test]
fn anthropic_effort_levels_pass_through() {
    let messages_payload = json!({
        "model": "test-model",
        "max_tokens": 4096,
        "thinking": {"type": "adaptive"},
        "output_config": {"effort": "xhigh"},
        "messages": [{"role": "user", "content": "hello"}]
    });
    let ir = messages::decode_request(&messages_payload).unwrap();
    assert_eq!(ir.reasoning.effort.as_deref(), Some("xhigh"));

    let (chat, _) = convert::request_to(&ir, Protocol::Chat).unwrap();
    assert_eq!(chat["reasoning_effort"], "xhigh");

    let (responses, _) = convert::request_to(&ir, Protocol::Responses).unwrap();
    assert_eq!(responses["reasoning"]["effort"], "xhigh");
}

#[test]
fn response_matrix_shapes() {
    let mut response = CanonicalResponse::new(Protocol::Chat, "chatcmpl-1", "test-model");
    response.output.push(OutputItem::Message {
        id: "msg_1".to_string(),
        role: Role::Assistant,
        content: vec![ContentPart::text("Hello")],
        annotations: Vec::new(),
    });
    response.output.push(OutputItem::ToolCall {
        id: "fc_1".to_string(),
        call_id: "call_1".to_string(),
        name: "lookup".to_string(),
        arguments: "{\"q\":\"x\"}".to_string(),
        status: "completed".to_string(),
    });
    response.usage = Usage {
        input_tokens: Some(10),
        output_tokens: Some(4),
        total_tokens: Some(14),
        cached_tokens: Some(2),
        cache_write_tokens: Some(1),
        reasoning_tokens: Some(1),
        server_tool_use: None,
    };
    let chat = convert::response_to(&response, Protocol::Chat).unwrap().0;
    assert_eq!(chat["object"], "chat.completion");
    assert_eq!(chat["choices"][0]["message"]["content"], "Hello");
    assert_eq!(
        chat["choices"][0]["message"]["tool_calls"][0]["id"],
        "call_1"
    );

    let responses = convert::response_to(&response, Protocol::Responses)
        .unwrap()
        .0;
    assert_eq!(responses["object"], "response");
    assert_eq!(responses["usage"]["input_tokens"], 10);
    assert_eq!(responses["output"][1]["type"], "function_call");

    let messages = convert::response_to(&response, Protocol::Messages)
        .unwrap()
        .0;
    assert_eq!(messages["type"], "message");
    assert_eq!(messages["stop_reason"], "tool_use");
    assert_eq!(messages["content"][1]["type"], "tool_use");
    assert_eq!(messages["usage"]["input_tokens"], 7);
    assert_eq!(messages["usage"]["cache_read_input_tokens"], 2);
    assert_eq!(messages["usage"]["cache_creation_input_tokens"], 1);
}

#[test]
fn response_decoders_build_canonical_response() {
    let chat = json!({
        "id": "chatcmpl-1",
        "object": "chat.completion",
        "created": 1,
        "model": "test-model",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "hi"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 2, "total_tokens": 3}
    });
    let chat = chat::decode_response(&chat).unwrap();
    assert_eq!(chat.text(), "hi");
    assert_eq!(chat.usage.total_tokens, Some(3));

    let responses = json!({
        "id": "resp_1",
        "object": "response",
        "created_at": 1,
        "status": "completed",
        "model": "test-model",
        "output": [{
            "id": "msg_1",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "hello", "annotations": [], "logprobs": []}]
        }],
        "usage": {"input_tokens": 3, "output_tokens": 2, "total_tokens": 5}
    });
    let responses = responses::decode_response(&responses).unwrap();
    assert_eq!(responses.text(), "hello");
    assert_eq!(responses.status, ResponseStatus::Completed);

    let messages_payload = json!({
        "id": "msg_1",
        "type": "message",
        "role": "assistant",
        "model": "test-model",
        "content": [{"type": "text", "text": "hello"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 3, "output_tokens": 2, "cache_read_input_tokens": 1}
    });
    let messages_response = messages::decode_response(&messages_payload).unwrap();
    assert_eq!(messages_response.text(), "hello");
    assert_eq!(messages_response.usage.input_tokens, Some(4));
}

fn chat_stream_events() -> Vec<SseEvent> {
    vec![
        SseEvent::data(
            json!({
                "id": "chatcmpl-1",
                "object": "chat.completion.chunk",
                "created": 1,
                "model": "test-model",
                "choices": [{"index": 0, "delta": {"role": "assistant", "content": ""}, "finish_reason": null}]
            })
            .to_string(),
        ),
        SseEvent::data(
            json!({
                "id": "chatcmpl-1",
                "object": "chat.completion.chunk",
                "created": 1,
                "model": "test-model",
                "choices": [{"index": 0, "delta": {"content": "Hello"}, "finish_reason": null}]
            })
            .to_string(),
        ),
        SseEvent::data(
            json!({
                "id": "chatcmpl-1",
                "object": "chat.completion.chunk",
                "created": 1,
                "model": "test-model",
                "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]
            })
            .to_string(),
        ),
        SseEvent::data(
            json!({
                "id": "chatcmpl-1",
                "object": "chat.completion.chunk",
                "created": 1,
                "model": "test-model",
                "choices": [],
                "usage": {"prompt_tokens": 3, "completion_tokens": 1, "total_tokens": 4}
            })
            .to_string(),
        ),
        SseEvent::data("[DONE]"),
    ]
}

#[test]
fn chat_stream_text_to_responses_and_messages() {
    let mut decoder = decoder_for(Protocol::Chat);
    let mut canonical = Vec::new();
    for event in chat_stream_events() {
        canonical.extend(decoder.decode(&event).unwrap());
    }
    assert!(canonical.iter().any(
        |event| matches!(event, CanonicalStreamEvent::TextDelta { delta, .. } if delta == "Hello")
    ));
    assert!(
        canonical
            .iter()
            .any(|event| matches!(event, CanonicalStreamEvent::End))
    );

    for target in [Protocol::Responses, Protocol::Messages] {
        let mut encoder = encoder_for(target, false);
        let mut encoded = Vec::new();
        for event in &canonical {
            encoded.extend(encoder.encode(event).unwrap());
        }
        encoded.extend(encoder.finish().unwrap());
        let names: Vec<&str> = encoded
            .iter()
            .filter_map(|event| event.event.as_deref())
            .collect();
        match target {
            Protocol::Responses => {
                assert!(names.contains(&"response.created"));
                assert!(names.contains(&"response.output_text.delta"));
                assert!(names.contains(&"response.completed"));
                assert!(!encoded.iter().any(SseEvent::is_done));
            }
            Protocol::Messages => {
                assert!(names.contains(&"message_start"));
                assert!(names.contains(&"content_block_delta"));
                assert!(names.contains(&"message_stop"));
                assert!(!encoded.iter().any(SseEvent::is_done));
            }
            Protocol::Chat => {}
        }
    }
}

#[test]
fn chat_stream_tool_call_to_messages() {
    let events = vec![
        SseEvent::data(
            json!({
                "id": "chatcmpl-2",
                "object": "chat.completion.chunk",
                "created": 1,
                "model": "test-model",
                "choices": [{"index": 0, "delta": {
                    "role": "assistant",
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "lookup", "arguments": "{\"q\":"}
                    }]
                }, "finish_reason": null}]
            })
            .to_string(),
        ),
        SseEvent::data(
            json!({
                "id": "chatcmpl-2",
                "object": "chat.completion.chunk",
                "created": 1,
                "model": "test-model",
                "choices": [{"index": 0, "delta": {
                    "tool_calls": [{"index": 0, "function": {"arguments": "\"x\"}"}}]
                }, "finish_reason": "tool_calls"}]
            })
            .to_string(),
        ),
        SseEvent::data("[DONE]"),
    ];
    let mut decoder = decoder_for(Protocol::Chat);
    let mut canonical = Vec::new();
    for event in events {
        canonical.extend(decoder.decode(&event).unwrap());
    }
    let mut encoder = encoder_for(Protocol::Messages, false);
    let mut encoded = Vec::new();
    for event in &canonical {
        encoded.extend(encoder.encode(event).unwrap());
    }
    encoded.extend(encoder.finish().unwrap());
    let names: Vec<&str> = encoded
        .iter()
        .filter_map(|event| event.event.as_deref())
        .collect();
    assert!(names.contains(&"message_start"));
    assert!(names.contains(&"content_block_start"));
    assert!(names.contains(&"message_delta"));
    assert!(names.contains(&"message_stop"));
    let tool_start = encoded
        .iter()
        .find(|event| {
            event.event.as_deref() == Some("content_block_start") && event.data.contains("tool_use")
        })
        .unwrap();
    assert!(tool_start.data.contains("lookup"));
    let message_delta = encoded
        .iter()
        .find(|event| event.event.as_deref() == Some("message_delta"))
        .unwrap();
    assert!(message_delta.data.contains("tool_use"));
}

#[test]
fn responses_stream_text_to_chat() {
    let events = vec![
        SseEvent::named(
            "response.created",
            json!({
                "type": "response.created",
                "sequence_number": 1,
                "response": {
                    "id": "resp_1",
                    "object": "response",
                    "created_at": 1,
                    "status": "in_progress",
                    "model": "test-model",
                    "output": []
                }
            })
            .to_string(),
        ),
        SseEvent::named(
            "response.output_text.delta",
            json!({
                "type": "response.output_text.delta",
                "sequence_number": 2,
                "item_id": "msg_1",
                "output_index": 0,
                "content_index": 0,
                "delta": "Hi"
            })
            .to_string(),
        ),
        SseEvent::named(
            "response.completed",
            json!({
                "type": "response.completed",
                "sequence_number": 3,
                "response": {
                    "id": "resp_1",
                    "object": "response",
                    "created_at": 1,
                    "status": "completed",
                    "model": "test-model",
                    "output": [],
                    "usage": {"input_tokens": 2, "output_tokens": 1, "total_tokens": 3}
                }
            })
            .to_string(),
        ),
    ];
    let mut decoder = decoder_for(Protocol::Responses);
    let mut canonical = Vec::new();
    for event in events {
        canonical.extend(decoder.decode(&event).unwrap());
    }
    assert!(canonical.iter().any(
        |event| matches!(event, CanonicalStreamEvent::TextDelta { delta, .. } if delta == "Hi")
    ));

    let mut encoder = encoder_for(Protocol::Chat, true);
    let mut encoded = Vec::new();
    for event in &canonical {
        encoded.extend(encoder.encode(event).unwrap());
    }
    encoded.extend(encoder.finish().unwrap());
    assert_eq!(encoded.last().unwrap().data, "[DONE]");
    assert!(
        encoded
            .iter()
            .any(|event| event.data.contains("\"content\":\"Hi\""))
    );
}
