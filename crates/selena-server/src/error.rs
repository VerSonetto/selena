use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use selena_protocol::{Protocol, ProtocolError};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            [(header::CONTENT_TYPE, "application/json")],
            json!({"error": {"message": self.message, "type": "selena_error"}}).to_string(),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self::internal(error.to_string())
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        Self::internal(error.to_string())
    }
}

pub fn protocol_error(protocol: Protocol, error: ProtocolError) -> Response {
    let status = StatusCode::from_u16(error.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = match protocol {
        Protocol::Chat | Protocol::Responses => error.to_openai_json(),
        Protocol::Messages => error.to_anthropic_json(None),
    };
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response()
}

pub fn message_error(protocol: Protocol, status: StatusCode, message: &str) -> Response {
    protocol_error(
        protocol,
        ProtocolError::new(
            selena_protocol::ErrorKind::from_status(status.as_u16()),
            message,
        ),
    )
}
