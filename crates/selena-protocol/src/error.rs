use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    InvalidRequest,
    Authentication,
    Permission,
    NotFound,
    Conflict,
    PayloadTooLarge,
    RateLimit,
    Server,
    Overloaded,
    Timeout,
    UnsupportedFeature,
    Stream,
    Internal,
}

impl ErrorKind {
    pub fn from_status(status: u16) -> Self {
        match status {
            400 | 422 => Self::InvalidRequest,
            401 => Self::Authentication,
            403 => Self::Permission,
            404 => Self::NotFound,
            409 => Self::Conflict,
            413 => Self::PayloadTooLarge,
            429 => Self::RateLimit,
            503 | 529 => Self::Overloaded,
            504 => Self::Timeout,
            500..=599 => Self::Server,
            _ => Self::Internal,
        }
    }

    pub fn openai_type(self) -> &'static str {
        match self {
            Self::InvalidRequest | Self::UnsupportedFeature => "invalid_request_error",
            Self::Authentication => "authentication_error",
            Self::Permission => "permission_denied",
            Self::NotFound => "not_found_error",
            Self::Conflict => "conflict_error",
            Self::PayloadTooLarge => "invalid_request_error",
            Self::RateLimit => "rate_limit_error",
            Self::Server | Self::Internal | Self::Stream => "server_error",
            Self::Overloaded => "service_unavailable_error",
            Self::Timeout => "timeout_error",
        }
    }

    pub fn anthropic_type(self) -> &'static str {
        match self {
            Self::InvalidRequest | Self::UnsupportedFeature => "invalid_request_error",
            Self::Authentication => "authentication_error",
            Self::Permission => "permission_error",
            Self::NotFound => "not_found_error",
            Self::Conflict => "conflict_error",
            Self::PayloadTooLarge => "request_too_large",
            Self::RateLimit => "rate_limit_error",
            Self::Server | Self::Internal | Self::Stream => "api_error",
            Self::Overloaded => "overloaded_error",
            Self::Timeout => "timeout_error",
        }
    }

    pub fn default_status(self) -> u16 {
        match self {
            Self::InvalidRequest | Self::UnsupportedFeature => 400,
            Self::Authentication => 401,
            Self::Permission => 403,
            Self::NotFound => 404,
            Self::Conflict => 409,
            Self::PayloadTooLarge => 413,
            Self::RateLimit => 429,
            Self::Server | Self::Internal | Self::Stream => 500,
            Self::Overloaded => 529,
            Self::Timeout => 504,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct ProtocolError {
    pub status: u16,
    pub kind: ErrorKind,
    pub message: String,
    pub param: Option<String>,
    pub code: Option<String>,
    pub feature: Option<String>,
    pub retryable: bool,
    pub upstream_request_id: Option<String>,
}

impl ProtocolError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            status: kind.default_status(),
            kind,
            message: message.into(),
            param: None,
            code: None,
            feature: None,
            retryable: matches!(
                kind,
                ErrorKind::RateLimit
                    | ErrorKind::Server
                    | ErrorKind::Overloaded
                    | ErrorKind::Timeout
                    | ErrorKind::Stream
            ),
            upstream_request_id: None,
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidRequest, message)
    }

    pub fn unsupported(feature: impl Into<String>, source: &str, target: &str) -> Self {
        let feature = feature.into();
        Self {
            status: 400,
            kind: ErrorKind::UnsupportedFeature,
            message: format!(
                "Feature `{feature}` cannot be converted from {source} to {target} without losing semantics"
            ),
            param: None,
            code: Some("unsupported_feature".to_string()),
            feature: Some(feature),
            retryable: false,
            upstream_request_id: None,
        }
    }

    pub fn upstream(status: u16, message: impl Into<String>, code: Option<String>) -> Self {
        let kind = ErrorKind::from_status(status);
        Self {
            status,
            kind,
            message: message.into(),
            param: None,
            code,
            feature: None,
            retryable: matches!(
                kind,
                ErrorKind::RateLimit
                    | ErrorKind::Server
                    | ErrorKind::Overloaded
                    | ErrorKind::Timeout
            ),
            upstream_request_id: None,
        }
    }

    pub fn stream(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Stream, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }

    pub fn with_param(mut self, param: impl Into<String>) -> Self {
        self.param = Some(param.into());
        self
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.upstream_request_id = Some(request_id.into());
        self
    }

    pub fn to_openai_json(&self) -> Value {
        json!({
            "error": {
                "message": self.message,
                "type": self.kind.openai_type(),
                "param": self.param,
                "code": self.code,
            }
        })
    }

    pub fn to_anthropic_json(&self, request_id: Option<&str>) -> Value {
        json!({
            "type": "error",
            "error": {
                "type": self.kind.anthropic_type(),
                "message": self.message,
            },
            "request_id": request_id,
        })
    }

    pub fn to_openai_event_json(&self) -> Value {
        json!({
            "type": "error",
            "code": self.code.clone().unwrap_or_else(|| self.kind.openai_type().to_string()),
            "message": self.message,
            "param": self.param,
        })
    }
}
