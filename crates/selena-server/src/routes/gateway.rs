use crate::error::{message_error, protocol_error};
use crate::sse_bridge;
use crate::{AppState, auth};
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use selena_core::CoreError;
use selena_core::models::NewRequestLog;
use selena_protocol::convert;
use selena_protocol::ir::{Protocol, Usage};
use selena_protocol::{ProtocolError, chat, messages, responses};
use serde_json::Value;
use std::time::Instant;

pub async fn chat(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    gateway(state, headers, body, Protocol::Chat).await
}

pub async fn responses(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    gateway(state, headers, body, Protocol::Responses).await
}

pub async fn messages(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    gateway(state, headers, body, Protocol::Messages).await
}

async fn gateway(state: AppState, headers: HeaderMap, body: Bytes, ingress: Protocol) -> Response {
    let started = Instant::now();
    let identity = match auth::gateway_auth(&state, ingress, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    if let (Some(key_id), limit) = (&identity.client_key_id, identity.rate_limit_rpm)
        && !state.core.allow_rate_limit(key_id, limit)
    {
        return message_error(
            ingress,
            StatusCode::TOO_MANY_REQUESTS,
            "client key rate limit exceeded",
        );
    }
    let body_json: Value = match serde_json::from_slice(&body) {
        Ok(body) => body,
        Err(error) => {
            return protocol_error(
                ingress,
                ProtocolError::invalid(format!("invalid JSON body: {error}"))
                    .with_code("invalid_json"),
            );
        }
    };
    let Some(model) = body_json.get("model").and_then(Value::as_str) else {
        return protocol_error(
            ingress,
            ProtocolError::invalid("`model` is required").with_param("model"),
        );
    };
    let stream = body_json
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let include_usage = body_json
        .get("stream_options")
        .and_then(|options| options.get("include_usage"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let resolved = match state.core.resolve_route(model).await {
        Ok(resolved) => resolved,
        Err(error) => return core_error(ingress, error),
    };
    let upstream_protocol = resolved.provider.protocol();
    let same_protocol = upstream_protocol == ingress;
    let mut warnings = Vec::new();

    let upstream_body = if same_protocol {
        body_json.clone()
    } else {
        let canonical = match decode_request(ingress, &body_json) {
            Ok(canonical) => canonical,
            Err(error) => return protocol_error(ingress, error),
        };
        match convert::request_to(&canonical, upstream_protocol) {
            Ok((value, conversion_warnings)) => {
                warnings = conversion_warnings.0;
                value
            }
            Err(error) => return protocol_error(ingress, error),
        }
    };

    let upstream = match state
        .core
        .send_upstream(
            &resolved.provider,
            &resolved.upstream_model,
            upstream_body,
            stream,
        )
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let response = core_error(ingress, error);
            log_request(
                &state,
                &LogContext {
                    ingress,
                    egress: upstream_protocol,
                    provider_id: Some(resolved.provider.id.clone()),
                    provider_name: Some(resolved.provider.name.clone()),
                    model_requested: model.to_string(),
                    model_upstream: resolved.upstream_model.clone(),
                    client_key_id: identity.client_key_id.clone(),
                    stream,
                    conversion_mode: if same_protocol {
                        "passthrough"
                    } else {
                        "converted"
                    },
                    warnings,
                    started,
                },
                StatusCode::BAD_GATEWAY,
                Usage::default(),
                Some("upstream_error".to_string()),
                Some("upstream request failed".to_string()),
                None,
            )
            .await;
            return response;
        }
    };

    let status = upstream.status();
    let upstream_request_id = upstream
        .headers()
        .get("x-request-id")
        .or_else(|| upstream.headers().get("request-id"))
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let content_type = upstream
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/json")
        .to_string();

    if !status.is_success() {
        let bytes = upstream.bytes().await.unwrap_or_default();
        if same_protocol {
            let response = (
                status,
                [(header::CONTENT_TYPE, content_type)],
                bytes.clone(),
            )
                .into_response();
            log_request(
                &state,
                &LogContext {
                    ingress,
                    egress: upstream_protocol,
                    provider_id: Some(resolved.provider.id.clone()),
                    provider_name: Some(resolved.provider.name.clone()),
                    model_requested: model.to_string(),
                    model_upstream: resolved.upstream_model.clone(),
                    client_key_id: identity.client_key_id.clone(),
                    stream,
                    conversion_mode: "passthrough",
                    warnings,
                    started,
                },
                status,
                Usage::default(),
                Some(format!("http_{}", status.as_u16())),
                Some(String::from_utf8_lossy(&bytes).chars().take(1000).collect()),
                upstream_request_id,
            )
            .await;
            return response;
        }
        let error = parse_upstream_error(upstream_protocol, status.as_u16(), &bytes);
        log_request(
            &state,
            &LogContext {
                ingress,
                egress: upstream_protocol,
                provider_id: Some(resolved.provider.id.clone()),
                provider_name: Some(resolved.provider.name.clone()),
                model_requested: model.to_string(),
                model_upstream: resolved.upstream_model.clone(),
                client_key_id: identity.client_key_id.clone(),
                stream,
                conversion_mode: "converted",
                warnings,
                started,
            },
            status,
            Usage::default(),
            error
                .code
                .clone()
                .or_else(|| Some(error.kind.openai_type().to_string())),
            Some(error.message.clone()),
            upstream_request_id,
        )
        .await;
        return protocol_error(ingress, error);
    }

    if stream && content_type.contains("text/event-stream") {
        if same_protocol {
            log_request(
                &state,
                &LogContext {
                    ingress,
                    egress: upstream_protocol,
                    provider_id: Some(resolved.provider.id.clone()),
                    provider_name: Some(resolved.provider.name.clone()),
                    model_requested: model.to_string(),
                    model_upstream: resolved.upstream_model.clone(),
                    client_key_id: identity.client_key_id.clone(),
                    stream: true,
                    conversion_mode: "passthrough",
                    warnings,
                    started,
                },
                StatusCode::OK,
                Usage::default(),
                None,
                None,
                upstream_request_id,
            )
            .await;
            return sse_response(Body::from_stream(upstream.bytes_stream()));
        }

        log_request(
            &state,
            &LogContext {
                ingress,
                egress: upstream_protocol,
                provider_id: Some(resolved.provider.id.clone()),
                provider_name: Some(resolved.provider.name.clone()),
                model_requested: model.to_string(),
                model_upstream: resolved.upstream_model.clone(),
                client_key_id: identity.client_key_id.clone(),
                stream: true,
                conversion_mode: "converted",
                warnings,
                started,
            },
            StatusCode::OK,
            Usage::default(),
            None,
            None,
            upstream_request_id,
        )
        .await;
        return sse_bridge::bridge_response(upstream, ingress, upstream_protocol, include_usage);
    }

    let bytes = match upstream.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => {
            return protocol_error(
                ingress,
                ProtocolError::stream(format!("failed to read upstream response: {error}")),
            );
        }
    };
    if same_protocol {
        let usage = decode_response(upstream_protocol, &bytes)
            .map(|response| response.usage)
            .unwrap_or_default();
        log_request(
            &state,
            &LogContext {
                ingress,
                egress: upstream_protocol,
                provider_id: Some(resolved.provider.id.clone()),
                provider_name: Some(resolved.provider.name.clone()),
                model_requested: model.to_string(),
                model_upstream: resolved.upstream_model.clone(),
                client_key_id: identity.client_key_id.clone(),
                stream: false,
                conversion_mode: "passthrough",
                warnings,
                started,
            },
            status,
            usage,
            None,
            None,
            upstream_request_id,
        )
        .await;
        return (status, [(header::CONTENT_TYPE, content_type)], bytes).into_response();
    }

    let canonical_response = match decode_response(upstream_protocol, &bytes) {
        Ok(response) => response,
        Err(error) => return protocol_error(ingress, error),
    };
    let (value, conversion_warnings) = match convert::response_to(&canonical_response, ingress) {
        Ok(result) => result,
        Err(error) => return protocol_error(ingress, error),
    };
    warnings.extend(conversion_warnings.0);
    log_request(
        &state,
        &LogContext {
            ingress,
            egress: upstream_protocol,
            provider_id: Some(resolved.provider.id.clone()),
            provider_name: Some(resolved.provider.name.clone()),
            model_requested: model.to_string(),
            model_upstream: resolved.upstream_model.clone(),
            client_key_id: identity.client_key_id.clone(),
            stream: false,
            conversion_mode: "converted",
            warnings,
            started,
        },
        status,
        canonical_response.usage.clone(),
        None,
        None,
        upstream_request_id,
    )
    .await;
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        value.to_string(),
    )
        .into_response()
}

fn decode_request(
    protocol: Protocol,
    value: &Value,
) -> Result<selena_protocol::CanonicalRequest, ProtocolError> {
    match protocol {
        Protocol::Chat => chat::decode_request(value),
        Protocol::Responses => responses::decode_request(value),
        Protocol::Messages => messages::decode_request(value),
    }
}

fn decode_response(
    protocol: Protocol,
    bytes: &[u8],
) -> Result<selena_protocol::CanonicalResponse, ProtocolError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| ProtocolError::stream(format!("invalid upstream JSON: {error}")))?;
    match protocol {
        Protocol::Chat => chat::decode_response(&value),
        Protocol::Responses => responses::decode_response(&value),
        Protocol::Messages => messages::decode_response(&value),
    }
}

fn parse_upstream_error(protocol: Protocol, status: u16, bytes: &[u8]) -> ProtocolError {
    let value: Value = serde_json::from_slice(bytes).unwrap_or(Value::Null);
    let (message, code) = match protocol {
        Protocol::Chat | Protocol::Responses => {
            let error = value.get("error");
            (
                error
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("upstream request failed"),
                error
                    .and_then(|error| error.get("code"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        }
        Protocol::Messages => {
            let error = value.get("error");
            (
                error
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("upstream request failed"),
                error
                    .and_then(|error| error.get("type"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        }
    };
    ProtocolError::upstream(status, message, code)
}

fn core_error(protocol: Protocol, error: CoreError) -> Response {
    match error {
        CoreError::NoProvider => message_error(
            protocol,
            StatusCode::SERVICE_UNAVAILABLE,
            "no provider configured; add one in Selena",
        ),
        CoreError::NoKey(provider) => message_error(
            protocol,
            StatusCode::SERVICE_UNAVAILABLE,
            &format!("no available API key for provider `{provider}`"),
        ),
        CoreError::Message(message) => message_error(protocol, StatusCode::BAD_GATEWAY, &message),
        CoreError::Http(error) => {
            message_error(protocol, StatusCode::BAD_GATEWAY, &error.to_string())
        }
        CoreError::Other(error) => {
            message_error(protocol, StatusCode::BAD_GATEWAY, &error.to_string())
        }
    }
}

fn sse_response(body: Body) -> Response {
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/event-stream; charset=utf-8"),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
            (
                HeaderName::from_static("x-accel-buffering"),
                HeaderValue::from_static("no"),
            ),
        ],
        body,
    )
        .into_response()
}

struct LogContext {
    ingress: Protocol,
    egress: Protocol,
    provider_id: Option<String>,
    provider_name: Option<String>,
    model_requested: String,
    model_upstream: String,
    client_key_id: Option<String>,
    stream: bool,
    conversion_mode: &'static str,
    warnings: Vec<String>,
    started: Instant,
}

#[allow(clippy::too_many_arguments)]
async fn log_request(
    state: &AppState,
    context: &LogContext,
    status: StatusCode,
    usage: Usage,
    error_code: Option<String>,
    error_message: Option<String>,
    upstream_request_id: Option<String>,
) {
    let log = NewRequestLog {
        ingress_protocol: context.ingress.as_str().to_string(),
        egress_protocol: context.egress.as_str().to_string(),
        provider_id: context.provider_id.clone(),
        provider_name: context.provider_name.clone(),
        model_requested: context.model_requested.clone(),
        model_upstream: context.model_upstream.clone(),
        client_key_id: context.client_key_id.clone(),
        status_code: status.as_u16() as i64,
        error_code,
        stream: context.stream,
        conversion_mode: context.conversion_mode.to_string(),
        latency_ms: context.started.elapsed().as_millis() as i64,
        ttft_ms: None,
        input_tokens: usage.input_tokens.map(|value| value as i64),
        output_tokens: usage.output_tokens.map(|value| value as i64),
        cached_tokens: usage.cached_tokens.map(|value| value as i64),
        reasoning_tokens: usage.reasoning_tokens.map(|value| value as i64),
        conversion_warnings: context.warnings.clone(),
        upstream_request_id,
        error_message: error_message.map(|message| message.chars().take(2000).collect()),
    };
    if let Err(error) = state.core.record_request(log).await {
        tracing::warn!("failed to record request: {error}");
    }
}
