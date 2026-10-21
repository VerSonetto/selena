pub mod config;
pub mod db;
pub mod models;
pub mod provider;
pub mod runtime;
pub mod secrets;
pub mod store;

use crate::config::{AppConfig, ConfigPaths};
use crate::db::Database;
use crate::models::{
    ModelPool, ModelPoolEntry, ModelProviderRef, NewRequestLog, Overview, Provider, ProviderModel,
    ProviderModelGroup, ResolvedRoute, Route,
};
use crate::provider::ProviderClient;
use crate::secrets::SecretBox;
use crate::store::Store;
use anyhow::Result;
use parking_lot::Mutex;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("no provider configured")]
    NoProvider,
    #[error("no available API key for provider `{0}`")]
    NoKey(String),
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

#[derive(Debug, Clone)]
struct CachedProviderModels {
    models: Vec<ProviderModel>,
    error: Option<String>,
    fetched_at: i64,
}

pub struct Core {
    pub config: AppConfig,
    pub paths: ConfigPaths,
    pub store: Store,
    pub http: reqwest::Client,
    rate_buckets: Mutex<HashMap<String, (i64, u32)>>,
    models_cache: Mutex<HashMap<String, CachedProviderModels>>,
}

impl Core {
    pub async fn new(config: AppConfig, paths: ConfigPaths) -> Result<Self> {
        paths.ensure_home()?;
        let database = Database::connect(&paths.database_file).await?;
        let secrets = SecretBox::load_or_create(&paths.master_key_file)?;
        let store = Store::new(database.pool, secrets);
        let http = reqwest::Client::builder()
            .user_agent(format!("Selena/{}", env!("CARGO_PKG_VERSION")))
            .pool_max_idle_per_host(16)
            .build()?;
        Ok(Self {
            config,
            paths,
            store,
            http,
            rate_buckets: Mutex::new(HashMap::new()),
            models_cache: Mutex::new(HashMap::new()),
        })
    }

    pub async fn model_pool(&self, refresh: bool) -> Result<ModelPool> {
        let providers = self.store.list_providers().await?;
        let groups = futures::future::join_all(
            providers
                .iter()
                .map(|provider| self.provider_models(provider, refresh)),
        )
        .await;

        let mut entries: BTreeMap<String, ModelPoolEntry> = BTreeMap::new();
        for group in &groups {
            for model in &group.models {
                let entry = entries
                    .entry(model.id.clone())
                    .or_insert_with(|| ModelPoolEntry {
                        id: model.id.clone(),
                        display_name: model.display_name.clone(),
                        providers: Vec::new(),
                    });
                if entry.display_name.is_none() {
                    entry.display_name = model.display_name.clone();
                }
                if !entry
                    .providers
                    .iter()
                    .any(|provider| provider.id == group.provider_id)
                {
                    entry.providers.push(ModelProviderRef {
                        id: group.provider_id.clone(),
                        name: group.provider_name.clone(),
                        protocol: group.protocol.clone(),
                    });
                }
            }
        }

        Ok(ModelPool {
            groups,
            models: entries.into_values().collect(),
            fetched_at: chrono::Utc::now().timestamp(),
        })
    }

    pub fn invalidate_model_cache(&self, provider_id: Option<&str>) {
        let mut cache = self.models_cache.lock();
        if let Some(provider_id) = provider_id {
            cache.remove(provider_id);
        } else {
            cache.clear();
        }
    }

    async fn provider_models(&self, provider: &Provider, refresh: bool) -> ProviderModelGroup {
        if !provider.enabled {
            return ProviderModelGroup {
                provider_id: provider.id.clone(),
                provider_name: provider.name.clone(),
                protocol: provider.protocol.clone(),
                enabled: false,
                models: Vec::new(),
                error: Some("Provider is disabled".to_string()),
                fetched_at: None,
            };
        }

        if !refresh
            && let Some(cached) = self.models_cache.lock().get(&provider.id).cloned()
            && chrono::Utc::now().timestamp() - cached.fetched_at < 120
        {
            return self.model_group_from_cache(provider, cached);
        }

        let (models, error) = match self.fetch_provider_models(provider).await {
            Ok(models) => (models, None),
            Err(error) => (Vec::new(), Some(error.to_string())),
        };
        let cached = CachedProviderModels {
            models,
            error,
            fetched_at: chrono::Utc::now().timestamp(),
        };
        self.models_cache
            .lock()
            .insert(provider.id.clone(), cached.clone());
        self.model_group_from_cache(provider, cached)
    }

    fn model_group_from_cache(
        &self,
        provider: &Provider,
        cached: CachedProviderModels,
    ) -> ProviderModelGroup {
        ProviderModelGroup {
            provider_id: provider.id.clone(),
            provider_name: provider.name.clone(),
            protocol: provider.protocol.clone(),
            enabled: provider.enabled,
            models: cached.models,
            error: cached.error,
            fetched_at: Some(cached.fetched_at),
        }
    }

    async fn fetch_provider_models(&self, provider: &Provider) -> Result<Vec<ProviderModel>> {
        let Some((_, secret)) = self.store.select_provider_key(&provider.id).await? else {
            anyhow::bail!("no ready API key");
        };
        let client = ProviderClient::new(&self.http, provider, &secret);
        let value = client.get_models().await?;
        let items = value
            .as_array()
            .or_else(|| value.get("data").and_then(serde_json::Value::as_array))
            .or_else(|| value.get("models").and_then(serde_json::Value::as_array));
        let Some(items) = items else {
            anyhow::bail!("models response is missing a data array");
        };

        let mut models: BTreeMap<String, ProviderModel> = BTreeMap::new();
        for item in items {
            let Some(id) = item
                .get("id")
                .or_else(|| item.get("name"))
                .and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            let display_name = item
                .get("display_name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    item.get("name")
                        .and_then(serde_json::Value::as_str)
                        .filter(|name| *name != id)
                        .map(str::to_string)
                });
            models
                .entry(id.to_string())
                .or_insert_with(|| ProviderModel {
                    id: id.to_string(),
                    display_name,
                    created_at: item
                        .get("created_at")
                        .or_else(|| item.get("created"))
                        .and_then(serde_json::Value::as_i64),
                    owned_by: item
                        .get("owned_by")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                });
        }
        Ok(models.into_values().collect())
    }

    pub fn allow_rate_limit(&self, key_id: &str, limit_rpm: Option<i64>) -> bool {
        let Some(limit_rpm) = limit_rpm.filter(|limit| *limit > 0) else {
            return true;
        };
        let minute = chrono::Utc::now().timestamp() / 60;
        let mut buckets = self.rate_buckets.lock();
        let bucket = buckets.entry(key_id.to_string()).or_insert((minute, 0));
        if bucket.0 != minute {
            *bucket = (minute, 0);
        }
        if bucket.1 >= limit_rpm as u32 {
            return false;
        }
        bucket.1 += 1;
        true
    }

    pub async fn resolve_route(&self, model: &str) -> Result<ResolvedRoute, CoreError> {
        if let Some((prefix, upstream)) = model.split_once('/')
            && !prefix.is_empty()
            && !upstream.is_empty()
            && let Some(provider) = self.store.find_provider(prefix).await?
        {
            return Ok(ResolvedRoute {
                provider,
                upstream_model: upstream.to_string(),
            });
        }
        if let Some(route) = self.store.find_route(model).await?
            && let Some(provider) = self.store.get_provider(&route.provider_id).await?
        {
            return Ok(ResolvedRoute {
                provider,
                upstream_model: route.upstream_model,
            });
        }
        let default = match self.store.get_setting("default_provider").await? {
            Some(id) => self.store.get_provider(&id).await?,
            None => None,
        };
        if let Some(provider) = default {
            return Ok(ResolvedRoute {
                provider,
                upstream_model: model.to_string(),
            });
        }
        if let Some(provider) = self
            .store
            .list_providers()
            .await?
            .into_iter()
            .find(|provider| provider.enabled)
        {
            return Ok(ResolvedRoute {
                provider,
                upstream_model: model.to_string(),
            });
        }
        Err(CoreError::NoProvider)
    }

    pub async fn send_upstream(
        &self,
        provider: &Provider,
        upstream_model: &str,
        mut body: serde_json::Value,
        stream: bool,
    ) -> Result<reqwest::Response, CoreError> {
        if let Some(object) = body.as_object_mut() {
            object.insert(
                "model".to_string(),
                serde_json::Value::String(upstream_model.to_string()),
            );
        }
        let mut last_error: Option<CoreError> = None;
        for _ in 0..8 {
            let Some((key, secret)) = self.store.select_provider_key(&provider.id).await? else {
                return Err(CoreError::NoKey(provider.name.clone()));
            };
            let client = ProviderClient::new(&self.http, provider, &secret);
            let response = match client.post_json(&body, stream).await {
                Ok(response) => response,
                Err(error) => {
                    let _ = self
                        .store
                        .record_key_result(&key.id, false, None, None)
                        .await;
                    last_error = Some(CoreError::Other(error));
                    continue;
                }
            };
            let status = response.status();
            if status.is_success() {
                let _ = self
                    .store
                    .record_key_result(&key.id, true, Some(status.as_u16()), None)
                    .await;
                return Ok(response);
            }
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let retryable = matches!(status.as_u16(), 401 | 403 | 429 | 500 | 502 | 503 | 504);
            let _ = self
                .store
                .record_key_result(&key.id, false, Some(status.as_u16()), retry_after)
                .await;
            if !retryable {
                return Ok(response);
            }
            last_error = Some(CoreError::Message(format!(
                "provider returned HTTP {}",
                status.as_u16()
            )));
        }
        Err(last_error
            .unwrap_or_else(|| CoreError::Message("all provider keys failed".to_string())))
    }

    pub async fn record_request(&self, log: NewRequestLog) -> Result<String> {
        self.store.insert_request_log(log).await
    }

    pub async fn overview(&self) -> Result<Overview> {
        let (host, port) = crate::runtime::read_runtime(&self.paths.runtime_file)
            .map(|runtime| (runtime.host, runtime.port))
            .unwrap_or_else(|| (self.config.host.clone(), self.config.port));
        self.store.overview(&host, port).await
    }

    pub async fn list_models(&self) -> Result<Vec<String>> {
        let routes: Vec<Route> = self.store.list_routes().await?;
        let providers = self.store.list_providers().await?;
        let enabled = providers
            .iter()
            .filter(|provider| provider.enabled)
            .map(|provider| (provider.id.as_str(), provider))
            .collect::<HashMap<_, _>>();
        let mut models = routes
            .into_iter()
            .filter(|route| route.enabled)
            .map(|route| route.model_alias)
            .collect::<Vec<_>>();
        for provider in enabled.values() {
            models.push(format!("{}/auto", provider.name));
        }
        for (provider_id, cached) in self.models_cache.lock().iter() {
            let Some(provider) = enabled.get(provider_id.as_str()) else {
                continue;
            };
            for model in &cached.models {
                models.push(format!("{}/{}", provider.name, model.id));
            }
        }
        models.sort();
        models.dedup();
        Ok(models)
    }

    pub fn admin_token(&self) -> Option<String> {
        crate::runtime::read_runtime(&self.paths.runtime_file).map(|runtime| runtime.admin_token)
    }
}

pub type SharedCore = Arc<Core>;
