use crate::AppState;
use crate::error::ApiError;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use selena_core::models::{
    CreateClientKey, CreateProvider, CreateProviderKey, RequestLogQuery, UpdateProvider,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Deserialize, Default)]
pub struct RefreshQuery {
    pub refresh: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UsageQuery {
    pub days: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct EnabledRequest {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct SettingsRequest {
    pub default_provider: Option<String>,
    pub theme: Option<String>,
    pub port: Option<u16>,
}

pub async fn overview(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let overview = state.core.overview().await?;
    Ok(Json(
        serde_json::to_value(overview).map_err(|error| ApiError::internal(error.to_string()))?,
    ))
}

pub async fn list_providers(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let providers = state.core.store.list_providers().await?;
    let mut result = Vec::new();
    for provider in providers {
        let keys = state.core.store.list_provider_keys(&provider.id).await?;
        result.push(json!({
            "provider": provider,
            "key_count": keys.len(),
            "ready_keys": keys.iter().filter(|key| key.enabled && key.status == "ready").count(),
        }));
    }
    Ok(Json(json!({"data": result})))
}

pub async fn create_provider(
    State(state): State<AppState>,
    Json(input): Json<CreateProvider>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let provider = state.core.store.create_provider(input).await?;
    state.core.invalidate_model_cache(None);
    Ok((StatusCode::CREATED, Json(json!(provider))))
}

pub async fn update_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateProvider>,
) -> Result<Json<Value>, ApiError> {
    let provider = state.core.store.update_provider(&id, input).await?;
    state.core.invalidate_model_cache(Some(&id));
    Ok(Json(json!(provider)))
}

pub async fn delete_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.core.store.delete_provider(&id).await?;
    state.core.invalidate_model_cache(Some(&id));
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_provider_keys(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let keys = state.core.store.list_provider_keys(&id).await?;
    Ok(Json(json!({"data": keys})))
}

pub async fn model_pool(
    State(state): State<AppState>,
    Query(query): Query<RefreshQuery>,
) -> Result<Json<Value>, ApiError> {
    let pool = state
        .core
        .model_pool(query.refresh.unwrap_or(false))
        .await?;
    Ok(Json(
        serde_json::to_value(pool).map_err(|error| ApiError::internal(error.to_string()))?,
    ))
}

pub async fn create_provider_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<CreateProviderKey>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let key = state.core.store.create_provider_key(&id, input).await?;
    state.core.invalidate_model_cache(Some(&id));
    Ok((StatusCode::CREATED, Json(json!(key))))
}

pub async fn delete_provider_key(
    State(state): State<AppState>,
    Path((_provider_id, key_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    state.core.store.delete_provider_key(&key_id).await?;
    state.core.invalidate_model_cache(None);
    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_provider_key_enabled(
    State(state): State<AppState>,
    Path((_provider_id, key_id)): Path<(String, String)>,
    Json(input): Json<EnabledRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .core
        .store
        .set_provider_key_enabled(&key_id, input.enabled)
        .await?;
    state.core.invalidate_model_cache(None);
    Ok(StatusCode::NO_CONTENT)
}

pub async fn test_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let provider = state
        .core
        .store
        .get_provider(&id)
        .await?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "provider not found"))?;
    let key = state.core.store.select_provider_key(&provider.id).await?;
    match key {
        Some((key, _)) => Ok(Json(json!({
            "ok": true,
            "provider": provider.name,
            "key": key.prefix,
            "message": "A key is available. Selena does not spend tokens on a test request."
        }))),
        None => Ok(Json(json!({
            "ok": false,
            "provider": provider.name,
            "message": "No ready key is available."
        }))),
    }
}

pub async fn list_client_keys(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let keys = state.core.store.list_client_keys().await?;
    Ok(Json(json!({"data": keys})))
}

pub async fn create_client_key(
    State(state): State<AppState>,
    Json(input): Json<CreateClientKey>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let key = state.core.store.create_client_key(input).await?;
    Ok((StatusCode::CREATED, Json(json!(key))))
}

pub async fn delete_client_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.core.store.delete_client_key(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_client_key_enabled(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<EnabledRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .core
        .store
        .set_client_key_enabled(&id, input.enabled)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_requests(
    State(state): State<AppState>,
    Query(query): Query<RequestLogQuery>,
) -> Result<Json<Value>, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    let (logs, total) = state.core.store.query_request_logs(&query).await?;
    Ok(Json(json!({
        "data": logs,
        "total": total,
        "limit": limit,
        "offset": offset,
        "has_more": offset + limit < total,
    })))
}

pub async fn usage(
    State(state): State<AppState>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<Value>, ApiError> {
    let report = state.core.usage_report(query.days.unwrap_or(7)).await?;
    Ok(Json(
        serde_json::to_value(report).map_err(|error| ApiError::internal(error.to_string()))?,
    ))
}

pub async fn get_settings(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let default_provider = state.core.store.get_setting("default_provider").await?;
    let theme = state
        .core
        .store
        .get_setting("theme")
        .await?
        .unwrap_or_else(|| state.core.config.theme.clone());
    let configured_port = selena_core::config::load_file(&state.core.paths)
        .await
        .map(|config| config.port)
        .unwrap_or(state.runtime.port);
    Ok(Json(json!({
        "default_provider": default_provider,
        "theme": theme,
        "host": state.runtime.host,
        "port": state.runtime.port,
        "configured_port": configured_port,
        "restart_required": configured_port != state.runtime.port,
        "data_dir": state.core.paths.home,
    })))
}

pub async fn put_settings(
    State(state): State<AppState>,
    Json(input): Json<SettingsRequest>,
) -> Result<Json<Value>, ApiError> {
    if let Some(default_provider) = input.default_provider {
        state
            .core
            .store
            .set_setting("default_provider", &default_provider)
            .await?;
    }
    if let Some(theme) = input.theme {
        state.core.store.set_setting("theme", &theme).await?;
    }
    if let Some(port) = input.port {
        if port == 0 {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "port must be between 1 and 65535",
            ));
        }
        selena_core::config::set_port(&state.core.paths, port).await?;
    }
    get_settings(State(state)).await
}

pub async fn capabilities() -> Json<Value> {
    Json(json!({
        "protocols": ["chat_completions", "responses", "anthropic_messages"],
        "same_protocol": "passthrough",
        "cross_protocol": "canonical IR conversion",
        "unsupported_cross_protocol": [
            "hosted_tools", "server_tools", "previous_response_id", "conversation",
            "reasoning_items", "thinking_blocks", "audio", "n>1", "prompt_template"
        ],
        "lossy_fields": ["logprobs", "cache_control", "service_tier", "image_detail"],
    }))
}
