use crate::AppState;
use crate::auth::constant_time_eq;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    token: String,
}

pub async fn login(State(state): State<AppState>, Json(request): Json<LoginRequest>) -> Response {
    if constant_time_eq(request.token.trim(), &state.admin_token) {
        Json(json!({"ok": true})).into_response()
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid admin token"})),
        )
            .into_response()
    }
}

pub async fn session(State(state): State<AppState>) -> Response {
    Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "host": state.runtime.host,
        "port": state.runtime.port,
    }))
    .into_response()
}
