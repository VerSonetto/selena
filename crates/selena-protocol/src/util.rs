use serde_json::{Map, Value, json};

pub fn as_object(value: &Value) -> Result<&Map<String, Value>, crate::error::ProtocolError> {
    value.as_object().ok_or_else(|| {
        crate::error::ProtocolError::invalid("request body must be a JSON object")
            .with_code("invalid_json")
    })
}

pub fn required_str<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, crate::error::ProtocolError> {
    object.get(key).and_then(Value::as_str).ok_or_else(|| {
        crate::error::ProtocolError::invalid(format!("missing required string field `{key}`"))
            .with_param(key)
    })
}

pub fn optional_str(object: &Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub fn optional_bool(object: &Map<String, Value>, key: &str) -> Option<bool> {
    object.get(key).and_then(Value::as_bool)
}

pub fn optional_u32(object: &Map<String, Value>, key: &str) -> Option<u32> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

pub fn optional_f64(object: &Map<String, Value>, key: &str) -> Option<f64> {
    object.get(key).and_then(Value::as_f64)
}

pub fn optional_i64(object: &Map<String, Value>, key: &str) -> Option<i64> {
    object.get(key).and_then(Value::as_i64)
}

pub fn string_or_array(object: &Map<String, Value>, key: &str) -> Vec<String> {
    match object.get(key) {
        Some(Value::String(value)) => vec![value.clone()],
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

pub fn text_part(text: impl Into<String>) -> Value {
    json!({"type": "text", "text": text.into()})
}

pub fn text_from_parts(parts: &[Value]) -> String {
    parts
        .iter()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("")
}

pub fn parse_tool_arguments(raw: &str) -> (Value, Option<String>) {
    match serde_json::from_str::<Value>(raw) {
        Ok(value) => (value, None),
        Err(_) => (Value::Null, Some(raw.to_string())),
    }
}

pub fn tool_arguments_string(input: &Value, raw: Option<&str>) -> String {
    if let Some(raw) = raw {
        return raw.to_string();
    }
    serde_json::to_string(input).unwrap_or_else(|_| "{}".to_string())
}

pub fn generated_tool_id() -> String {
    format!("call_{}", uuid::Uuid::now_v7().simple())
}

pub fn generated_message_id() -> String {
    format!("msg_{}", uuid::Uuid::now_v7().simple())
}

pub fn generated_response_id() -> String {
    format!("resp_{}", uuid::Uuid::now_v7().simple())
}

pub fn generated_chat_id() -> String {
    format!("chatcmpl-{}", uuid::Uuid::now_v7().simple())
}

pub fn annotations_from_chat(value: Option<&Value>) -> Vec<Value> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let citation = item.get("url_citation")?;
                    Some(json!({
                        "type": "url_citation",
                        "url": citation.get("url").cloned().unwrap_or(Value::Null),
                        "title": citation.get("title").cloned().unwrap_or(Value::Null),
                        "start_index": citation.get("start_index").cloned().unwrap_or(Value::Null),
                        "end_index": citation.get("end_index").cloned().unwrap_or(Value::Null),
                    }))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn annotations_to_chat(annotations: &[Value]) -> Vec<Value> {
    annotations
        .iter()
        .filter_map(|annotation| {
            let object = annotation.as_object()?;
            if object.get("type").and_then(Value::as_str) != Some("url_citation") {
                return None;
            }
            Some(json!({
                "type": "url_citation",
                "url_citation": {
                    "url": object.get("url").cloned().unwrap_or(Value::Null),
                    "title": object.get("title").cloned().unwrap_or(Value::Null),
                    "start_index": object.get("start_index").cloned().unwrap_or(Value::Null),
                    "end_index": object.get("end_index").cloned().unwrap_or(Value::Null),
                }
            }))
        })
        .collect()
}
