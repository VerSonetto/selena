use crate::AppState;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use selena_protocol::{ErrorKind, Protocol, ProtocolError};
use subtle::ConstantTimeEq;

#[derive(Debug, Clone, Default)]
pub struct GatewayIdentity {
    pub client_key_id: Option<String>,
    pub rate_limit_rpm: Option<i64>,
    pub via_admin: bool,
}

pub async fn gateway_auth(
    state: &AppState,
    protocol: Protocol,
    headers: &HeaderMap,
) -> Result<GatewayIdentity, Response> {
    if let Some(token) = admin_token_from_headers(headers)
        && constant_time_eq(&token, &state.admin_token)
    {
        return Ok(GatewayIdentity {
            client_key_id: None,
            rate_limit_rpm: None,
            via_admin: true,
        });
    }

    let Some(secret) = api_key_from_headers(headers) else {
        if state
            .core
            .store
            .client_keys_present()
            .await
            .unwrap_or(false)
        {
            return Err(unauthorized(protocol, "missing API key"));
        }
        return Ok(GatewayIdentity::default());
    };

    if !state
        .core
        .store
        .client_keys_present()
        .await
        .unwrap_or(false)
    {
        return Ok(GatewayIdentity::default());
    }

    match state.core.store.verify_client_key(&secret).await {
        Ok(Some(key)) => Ok(GatewayIdentity {
            client_key_id: Some(key.id),
            rate_limit_rpm: key.rate_limit_rpm,
            via_admin: false,
        }),
        _ => Err(unauthorized(protocol, "invalid API key")),
    }
}

fn unauthorized(protocol: Protocol, message: &str) -> Response {
    let error = ProtocolError::new(ErrorKind::Authentication, message);
    crate::error::protocol_error(protocol, error)
}

pub async fn admin_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let Some(token) = admin_token_from_headers(request.headers()) else {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({"error": "admin token required"})),
        )
            .into_response();
    };
    if !constant_time_eq(&token, &state.admin_token) {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({"error": "invalid admin token"})),
        )
            .into_response();
    }
    next.run(request).await
}

pub fn admin_token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get("x-selena-admin")
        .and_then(|value| value.to_str().ok())
    {
        return Some(value.to_string());
    }
    bearer(headers)
}

pub fn api_key_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
    {
        return Some(value.to_string());
    }
    bearer(headers)
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    value
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.as_bytes().ct_eq(right.as_bytes()).into()
}
