#![allow(clippy::result_large_err)]

pub mod auth;
pub mod error;
pub mod host;
pub mod routes;
pub mod sse_bridge;

use axum::extract::State;
use axum::http::Method;
use axum::routing::{get, post};
use axum::{Json, Router};
use selena_core::SharedCore;
use selena_core::runtime::RuntimeInfo;
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub core: SharedCore,
    pub admin_token: String,
    pub runtime: RuntimeInfo,
}

pub fn build_router(state: AppState) -> Router {
    let admin = Router::new()
        .route("/auth/session", get(routes::auth::session))
        .route("/overview", get(routes::admin::overview))
        .route("/providers", get(routes::admin::list_providers))
        .route("/providers", post(routes::admin::create_provider))
        .route(
            "/providers/{id}",
            axum::routing::patch(routes::admin::update_provider)
                .delete(routes::admin::delete_provider),
        )
        .route(
            "/providers/{id}/keys",
            get(routes::admin::list_provider_keys).post(routes::admin::create_provider_key),
        )
        .route(
            "/providers/{id}/keys/{key_id}",
            axum::routing::delete(routes::admin::delete_provider_key),
        )
        .route(
            "/providers/{id}/keys/{key_id}/enabled",
            axum::routing::patch(routes::admin::set_provider_key_enabled),
        )
        .route("/providers/{id}/test", post(routes::admin::test_provider))
        .route("/model-pool", get(routes::admin::model_pool))
        .route("/routes", get(routes::admin::list_routes))
        .route("/routes", post(routes::admin::create_route))
        .route(
            "/routes/{id}",
            axum::routing::delete(routes::admin::delete_route),
        )
        .route("/client-keys", get(routes::admin::list_client_keys))
        .route("/client-keys", post(routes::admin::create_client_key))
        .route(
            "/client-keys/{id}",
            axum::routing::delete(routes::admin::delete_client_key),
        )
        .route(
            "/client-keys/{id}/enabled",
            axum::routing::patch(routes::admin::set_client_key_enabled),
        )
        .route("/requests", get(routes::admin::list_requests))
        .route("/usage", get(routes::admin::usage))
        .route("/settings", get(routes::admin::get_settings))
        .route("/settings", axum::routing::put(routes::admin::put_settings))
        .route("/capabilities", get(routes::admin::capabilities))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::admin_middleware,
        ));

    Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(models))
        .route("/v1/chat/completions", post(routes::gateway::chat))
        .route("/v1/responses", post(routes::gateway::responses))
        .route("/v1/messages", post(routes::gateway::messages))
        .route("/api/auth/login", post(routes::auth::login))
        .nest("/api", admin)
        .fallback(routes::static_assets::spa_handler)
        .with_state(state)
        .layer(RequestBodyLimitLayer::new(64 * 1024 * 1024))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PATCH,
                    Method::PUT,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
}

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "ok": true,
        "name": "selena",
        "version": env!("CARGO_PKG_VERSION"),
        "host": state.runtime.host,
        "port": state.runtime.port,
    }))
}

async fn models(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if let Err(response) =
        auth::gateway_auth(&state, selena_protocol::Protocol::Chat, &headers).await
    {
        return response;
    }
    match state.core.list_models().await {
        Ok(models) => Json(json!({
            "object": "list",
            "data": models.into_iter().enumerate().map(|(index, id)| json!({
                "id": id,
                "object": "model",
                "created": 0,
                "owned_by": "selena",
                "index": index,
            })).collect::<Vec<_>>(),
        }))
        .into_response(),
        Err(error) => crate::error::message_error(
            selena_protocol::Protocol::Chat,
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            &error.to_string(),
        ),
    }
}
