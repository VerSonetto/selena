use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    pub auth_type: String,
    pub extra_headers: String,
    pub enabled: bool,
    pub timeout_ms: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Provider {
    pub fn protocol(&self) -> selena_protocol::Protocol {
        match self.protocol.as_str() {
            "responses" => selena_protocol::Protocol::Responses,
            "anthropic_messages" => selena_protocol::Protocol::Messages,
            _ => selena_protocol::Protocol::Chat,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateProvider {
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    #[serde(default = "default_auth_type")]
    pub auth_type: String,
    #[serde(default)]
    pub extra_headers: Option<serde_json::Value>,
    #[serde(default)]
    pub timeout_ms: Option<i64>,
}

fn default_auth_type() -> String {
    "bearer".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateProvider {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub auth_type: Option<String>,
    pub extra_headers: Option<serde_json::Value>,
    pub enabled: Option<bool>,
    pub timeout_ms: Option<i64>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ProviderKey {
    pub id: String,
    pub provider_id: String,
    pub label: String,
    pub prefix: String,
    pub enabled: bool,
    pub status: String,
    pub cooldown_until: Option<i64>,
    pub failure_count: i64,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateProviderKey {
    pub label: String,
    pub secret: String,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ClientKey {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub enabled: bool,
    pub rate_limit_rpm: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct ClientKeyWithHash {
    pub id: String,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub enabled: bool,
    pub rate_limit_rpm: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateClientKey {
    pub name: String,
    #[serde(default)]
    pub rate_limit_rpm: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreatedClientKey {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub secret: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct RequestLogRow {
    pub id: String,
    pub ts: i64,
    pub ingress_protocol: String,
    pub egress_protocol: String,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub model_requested: String,
    pub model_upstream: String,
    pub client_key_id: Option<String>,
    pub status_code: i64,
    pub error_code: Option<String>,
    pub stream: bool,
    pub conversion_mode: String,
    pub latency_ms: i64,
    pub ttft_ms: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub cached_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub conversion_warnings: String,
    pub upstream_request_id: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewRequestLog {
    pub ingress_protocol: String,
    pub egress_protocol: String,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub model_requested: String,
    pub model_upstream: String,
    pub client_key_id: Option<String>,
    pub status_code: i64,
    pub error_code: Option<String>,
    pub stream: bool,
    pub conversion_mode: String,
    pub latency_ms: i64,
    pub ttft_ms: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub cached_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub conversion_warnings: Vec<String>,
    pub upstream_request_id: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RequestLogCompletion {
    pub status_code: i64,
    pub latency_ms: i64,
    pub ttft_ms: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub cached_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub conversion_warnings: Vec<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RequestLogQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub protocol: Option<String>,
    pub status: Option<String>,
    pub provider_id: Option<String>,
    pub stream: Option<bool>,
    pub search: Option<String>,
    pub days: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderModel {
    pub id: String,
    pub display_name: Option<String>,
    pub created_at: Option<i64>,
    pub owned_by: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderModelGroup {
    pub provider_id: String,
    pub provider_name: String,
    pub protocol: String,
    pub enabled: bool,
    pub models: Vec<ProviderModel>,
    pub error: Option<String>,
    pub fetched_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelProviderRef {
    pub id: String,
    pub name: String,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelPoolEntry {
    pub id: String,
    pub display_name: Option<String>,
    pub providers: Vec<ModelProviderRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelPool {
    pub groups: Vec<ProviderModelGroup>,
    pub models: Vec<ModelPoolEntry>,
    pub fetched_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Overview {
    pub version: String,
    pub host: String,
    pub port: u16,
    pub providers: i64,
    pub enabled_providers: i64,
    pub client_keys: i64,
    pub requests_today: i64,
    pub errors_today: i64,
    pub input_tokens_today: i64,
    pub output_tokens_today: i64,
    pub avg_latency_ms: f64,
    pub recent_requests: Vec<RequestLogRow>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UsageSummary {
    pub requests: i64,
    pub errors: i64,
    pub usage_requests: i64,
    pub stream_requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub cached_tokens: i64,
    pub cache_write_tokens: i64,
    pub reasoning_tokens: i64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: i64,
    pub p95_latency_ms: i64,
    pub avg_ttft_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageBucket {
    pub key: String,
    pub label: String,
    pub requests: i64,
    pub errors: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub cached_tokens: i64,
    pub cache_write_tokens: i64,
    pub reasoning_tokens: i64,
    pub avg_latency_ms: f64,
    pub avg_ttft_ms: Option<f64>,
}

impl UsageBucket {
    pub fn empty(key: impl Into<String>) -> Self {
        let key = key.into();
        Self {
            label: key.clone(),
            key,
            requests: 0,
            errors: 0,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            cached_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            avg_latency_ms: 0.0,
            avg_ttft_ms: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageReport {
    pub days: i64,
    pub since: i64,
    pub summary: UsageSummary,
    pub daily: Vec<UsageBucket>,
    pub by_provider: Vec<UsageBucket>,
    pub by_model: Vec<UsageBucket>,
    pub by_protocol: Vec<UsageBucket>,
}

#[derive(Debug, Clone)]
pub struct ResolvedRoute {
    pub provider: Provider,
    pub upstream_model: String,
}
