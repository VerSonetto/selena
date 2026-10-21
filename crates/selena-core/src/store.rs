use crate::models::*;
use crate::runtime::hex;
use crate::secrets::SecretBox;
use anyhow::{Context, Result};
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
    secrets: SecretBox,
}

impl Store {
    pub fn new(pool: SqlitePool, secrets: SecretBox) -> Self {
        Self { pool, secrets }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn list_providers(&self) -> Result<Vec<Provider>> {
        Ok(
            sqlx::query_as::<_, Provider>("SELECT * FROM providers ORDER BY created_at ASC")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn get_provider(&self, id: &str) -> Result<Option<Provider>> {
        Ok(
            sqlx::query_as::<_, Provider>("SELECT * FROM providers WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn find_provider(&self, id_or_name: &str) -> Result<Option<Provider>> {
        Ok(sqlx::query_as::<_, Provider>(
            "SELECT * FROM providers WHERE id = ? OR lower(name) = lower(?) LIMIT 1",
        )
        .bind(id_or_name)
        .bind(id_or_name)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn create_provider(&self, input: CreateProvider) -> Result<Provider> {
        let protocol = normalize_protocol(&input.protocol)?;
        let name = input.name.trim();
        anyhow::ensure!(!name.is_empty(), "provider name cannot be empty");
        anyhow::ensure!(
            !name.contains('/'),
            "provider name cannot contain '/' because it is used as the model prefix"
        );
        anyhow::ensure!(
            input.base_url.starts_with("http://") || input.base_url.starts_with("https://"),
            "provider base_url must use http or https"
        );
        let auth_type = normalize_auth(&input.auth_type)?;
        let headers = input.extra_headers.unwrap_or_else(|| serde_json::json!({}));
        anyhow::ensure!(headers.is_object(), "extra_headers must be an object");
        let now = now();
        let id = uuid::Uuid::now_v7().simple().to_string();
        sqlx::query(
            "INSERT INTO providers (id,name,protocol,base_url,auth_type,extra_headers,enabled,timeout_ms,created_at,updated_at)
             VALUES (?,?,?,?,?,?,1,?,?,?)",
        )
        .bind(&id)
        .bind(name)
        .bind(protocol)
        .bind(input.base_url.trim_end_matches('/'))
        .bind(auth_type)
        .bind(headers.to_string())
        .bind(input.timeout_ms.unwrap_or(300_000))
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .context("create provider")?;
        self.get_provider(&id)
            .await?
            .context("created provider not found")
    }

    pub async fn update_provider(&self, id: &str, input: UpdateProvider) -> Result<Provider> {
        let current = self.get_provider(id).await?.context("provider not found")?;
        let name = input.name.unwrap_or(current.name);
        let name = name.trim();
        anyhow::ensure!(!name.is_empty(), "provider name cannot be empty");
        anyhow::ensure!(
            !name.contains('/'),
            "provider name cannot contain '/' because it is used as the model prefix"
        );
        let base_url = input.base_url.unwrap_or(current.base_url);
        anyhow::ensure!(
            base_url.starts_with("http://") || base_url.starts_with("https://"),
            "provider base_url must use http or https"
        );
        let auth_type = match input.auth_type {
            Some(value) => normalize_auth(&value)?.to_string(),
            None => current.auth_type,
        };
        let extra_headers = input
            .extra_headers
            .map(|value| {
                anyhow::ensure!(value.is_object(), "extra_headers must be an object");
                Ok::<_, anyhow::Error>(value.to_string())
            })
            .transpose()?
            .unwrap_or(current.extra_headers);
        let enabled = input.enabled.unwrap_or(current.enabled);
        let timeout_ms = input.timeout_ms.unwrap_or(current.timeout_ms);
        sqlx::query(
            "UPDATE providers SET name=?, base_url=?, auth_type=?, extra_headers=?, enabled=?, timeout_ms=?, updated_at=? WHERE id=?",
        )
        .bind(name)
        .bind(base_url.trim_end_matches('/'))
        .bind(auth_type)
        .bind(extra_headers)
        .bind(enabled)
        .bind(timeout_ms)
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_provider(id)
            .await?
            .context("updated provider not found")
    }

    pub async fn delete_provider(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM providers WHERE id=?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_provider_keys(&self, provider_id: &str) -> Result<Vec<ProviderKey>> {
        Ok(sqlx::query_as::<_, ProviderKey>(
            "SELECT id,provider_id,label,prefix,enabled,status,cooldown_until,failure_count,last_used_at,created_at
             FROM provider_keys WHERE provider_id=? ORDER BY created_at ASC",
        )
        .bind(provider_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn create_provider_key(
        &self,
        provider_id: &str,
        input: CreateProviderKey,
    ) -> Result<ProviderKey> {
        anyhow::ensure!(!input.secret.trim().is_empty(), "secret cannot be empty");
        let encrypted = self.secrets.encrypt(input.secret.trim())?;
        let prefix = key_prefix(input.secret.trim());
        let id = uuid::Uuid::now_v7().simple().to_string();
        let now = now();
        sqlx::query(
            "INSERT INTO provider_keys (id,provider_id,label,secret_enc,prefix,enabled,status,failure_count,created_at)
             VALUES (?,?,?,?,?,1,'ready',0,?)",
        )
        .bind(&id)
        .bind(provider_id)
        .bind(input.label)
        .bind(encrypted)
        .bind(&prefix)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(sqlx::query_as::<_, ProviderKey>(
            "SELECT id,provider_id,label,prefix,enabled,status,cooldown_until,failure_count,last_used_at,created_at
             FROM provider_keys WHERE id=?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn delete_provider_key(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM provider_keys WHERE id=?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_provider_key_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        sqlx::query("UPDATE provider_keys SET enabled=? WHERE id=?")
            .bind(enabled)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn select_provider_key(
        &self,
        provider_id: &str,
    ) -> Result<Option<(ProviderKey, String)>> {
        let now = now();
        let rows = sqlx::query(
            "SELECT id,provider_id,label,prefix,enabled,status,cooldown_until,failure_count,last_used_at,created_at,secret_enc
             FROM provider_keys
             WHERE provider_id=? AND enabled=1 AND status='ready'
               AND (cooldown_until IS NULL OR cooldown_until <= ?)
             ORDER BY (last_used_at IS NOT NULL), last_used_at ASC, created_at ASC
             LIMIT 8",
        )
        .bind(provider_id)
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        for row in rows {
            let key = ProviderKey {
                id: row.try_get("id")?,
                provider_id: row.try_get("provider_id")?,
                label: row.try_get("label")?,
                prefix: row.try_get("prefix")?,
                enabled: row.try_get("enabled")?,
                status: row.try_get("status")?,
                cooldown_until: row.try_get("cooldown_until")?,
                failure_count: row.try_get("failure_count")?,
                last_used_at: row.try_get("last_used_at")?,
                created_at: row.try_get("created_at")?,
            };
            let encrypted: String = row.try_get("secret_enc")?;
            if let Ok(secret) = self.secrets.decrypt(&encrypted) {
                sqlx::query("UPDATE provider_keys SET last_used_at=? WHERE id=?")
                    .bind(now)
                    .bind(&key.id)
                    .execute(&self.pool)
                    .await?;
                return Ok(Some((key, secret)));
            }
        }
        Ok(None)
    }

    pub async fn record_key_result(
        &self,
        key_id: &str,
        success: bool,
        status_code: Option<u16>,
        retry_after_secs: Option<u64>,
    ) -> Result<()> {
        if success {
            sqlx::query(
                "UPDATE provider_keys SET failure_count=0,status='ready',cooldown_until=NULL WHERE id=?",
            )
            .bind(key_id)
            .execute(&self.pool)
            .await?;
            return Ok(());
        }
        match status_code {
            Some(401 | 403) => {
                sqlx::query(
                    "UPDATE provider_keys SET status='disabled',failure_count=failure_count+1 WHERE id=?",
                )
                .bind(key_id)
                .execute(&self.pool)
                .await?;
            }
            Some(429) => {
                let cooldown = now() + retry_after_secs.unwrap_or(30) as i64;
                sqlx::query(
                    "UPDATE provider_keys SET status='cooling',cooldown_until=?,failure_count=failure_count+1 WHERE id=?",
                )
                .bind(cooldown)
                .bind(key_id)
                .execute(&self.pool)
                .await?;
            }
            _ => {
                sqlx::query("UPDATE provider_keys SET failure_count=failure_count+1 WHERE id=?")
                    .bind(key_id)
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn list_routes(&self) -> Result<Vec<Route>> {
        Ok(
            sqlx::query_as::<_, Route>("SELECT * FROM routes ORDER BY model_alias ASC")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn create_route(&self, input: CreateRoute) -> Result<Route> {
        let id = uuid::Uuid::now_v7().simple().to_string();
        sqlx::query(
            "INSERT INTO routes (id,model_alias,provider_id,upstream_model,enabled,created_at) VALUES (?,?,?,?,1,?)",
        )
        .bind(&id)
        .bind(input.model_alias)
        .bind(input.provider_id)
        .bind(input.upstream_model)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(
            sqlx::query_as::<_, Route>("SELECT * FROM routes WHERE id=?")
                .bind(id)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn delete_route(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM routes WHERE id=?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn find_route(&self, alias: &str) -> Result<Option<Route>> {
        Ok(sqlx::query_as::<_, Route>(
            "SELECT * FROM routes WHERE lower(model_alias)=lower(?) AND enabled=1 LIMIT 1",
        )
        .bind(alias)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_client_keys(&self) -> Result<Vec<ClientKey>> {
        Ok(sqlx::query_as::<_, ClientKey>(
            "SELECT id,name,key_prefix,enabled,rate_limit_rpm,created_at FROM client_keys ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn client_keys_present(&self) -> Result<bool> {
        let row = sqlx::query("SELECT COUNT(*) AS count FROM client_keys")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get::<i64, _>("count")? > 0)
    }

    pub async fn create_client_key(&self, input: CreateClientKey) -> Result<CreatedClientKey> {
        let mut random = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut random);
        let secret = format!("sk-selena-{}", hex(&random));
        let hash = hash_key(&secret);
        let prefix = key_prefix(&secret);
        let id = uuid::Uuid::now_v7().simple().to_string();
        let now = now();
        sqlx::query(
            "INSERT INTO client_keys (id,name,key_hash,key_prefix,enabled,rate_limit_rpm,created_at) VALUES (?,?,?,?,1,?,?)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(hash)
        .bind(&prefix)
        .bind(input.rate_limit_rpm)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(CreatedClientKey {
            id,
            name: input.name,
            key_prefix: prefix,
            secret,
            created_at: now,
        })
    }

    pub async fn delete_client_key(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM client_keys WHERE id=?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_client_key_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        sqlx::query("UPDATE client_keys SET enabled=? WHERE id=?")
            .bind(enabled)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn verify_client_key(&self, secret: &str) -> Result<Option<ClientKey>> {
        let hash = hash_key(secret);
        let row = sqlx::query_as::<_, ClientKeyWithHash>(
            "SELECT id,name,key_hash,key_prefix,enabled,rate_limit_rpm,created_at FROM client_keys WHERE key_hash=? AND enabled=1",
        )
        .bind(&hash)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        use subtle::ConstantTimeEq;
        if row.key_hash.as_bytes().ct_eq(hash.as_bytes()).into() {
            Ok(Some(ClientKey {
                id: row.id,
                name: row.name,
                key_prefix: row.key_prefix,
                enabled: row.enabled,
                rate_limit_rpm: row.rate_limit_rpm,
                created_at: row.created_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn insert_request_log(&self, log: NewRequestLog) -> Result<String> {
        let id = uuid::Uuid::now_v7().simple().to_string();
        sqlx::query(
            "INSERT INTO request_logs
             (id,ts,ingress_protocol,egress_protocol,provider_id,provider_name,model_requested,model_upstream,client_key_id,
              status_code,error_code,stream,conversion_mode,latency_ms,ttft_ms,input_tokens,output_tokens,cached_tokens,
              reasoning_tokens,conversion_warnings,upstream_request_id,error_message)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(&id)
        .bind(now())
        .bind(log.ingress_protocol)
        .bind(log.egress_protocol)
        .bind(log.provider_id)
        .bind(log.provider_name)
        .bind(log.model_requested)
        .bind(log.model_upstream)
        .bind(log.client_key_id)
        .bind(log.status_code)
        .bind(log.error_code)
        .bind(log.stream)
        .bind(log.conversion_mode)
        .bind(log.latency_ms)
        .bind(log.ttft_ms)
        .bind(log.input_tokens)
        .bind(log.output_tokens)
        .bind(log.cached_tokens)
        .bind(log.reasoning_tokens)
        .bind(serde_json::to_string(&log.conversion_warnings)?)
        .bind(log.upstream_request_id)
        .bind(log.error_message)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn list_request_logs(&self, limit: i64, offset: i64) -> Result<Vec<RequestLogRow>> {
        Ok(sqlx::query_as::<_, RequestLogRow>(
            "SELECT * FROM request_logs ORDER BY ts DESC LIMIT ? OFFSET ?",
        )
        .bind(limit.clamp(1, 200))
        .bind(offset.max(0))
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn overview(&self, host: &str, port: u16) -> Result<Overview> {
        let providers: i64 = sqlx::query("SELECT COUNT(*) AS count FROM providers")
            .fetch_one(&self.pool)
            .await?
            .try_get("count")?;
        let enabled_providers: i64 =
            sqlx::query("SELECT COUNT(*) AS count FROM providers WHERE enabled=1")
                .fetch_one(&self.pool)
                .await?
                .try_get("count")?;
        let routes: i64 = sqlx::query("SELECT COUNT(*) AS count FROM routes")
            .fetch_one(&self.pool)
            .await?
            .try_get("count")?;
        let client_keys: i64 = sqlx::query("SELECT COUNT(*) AS count FROM client_keys")
            .fetch_one(&self.pool)
            .await?
            .try_get("count")?;
        let since = now() - 86_400;
        let row = sqlx::query(
            "SELECT COUNT(*) AS requests,
                    COALESCE(SUM(CASE WHEN status_code >= 400 THEN 1 ELSE 0 END),0) AS errors,
                    COALESCE(SUM(input_tokens),0) AS input_tokens,
                    COALESCE(SUM(output_tokens),0) AS output_tokens,
                    CAST(COALESCE(AVG(latency_ms),0) AS REAL) AS avg_latency
             FROM request_logs WHERE ts >= ?",
        )
        .bind(since)
        .fetch_one(&self.pool)
        .await?;
        Ok(Overview {
            version: env!("CARGO_PKG_VERSION").to_string(),
            host: host.to_string(),
            port,
            providers,
            enabled_providers,
            routes,
            client_keys,
            requests_today: row.try_get("requests")?,
            errors_today: row.try_get("errors")?,
            input_tokens_today: row.try_get("input_tokens")?,
            output_tokens_today: row.try_get("output_tokens")?,
            avg_latency_ms: row.try_get("avg_latency")?,
            recent_requests: self.list_request_logs(8, 0).await?,
        })
    }

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        Ok(sqlx::query("SELECT value FROM settings WHERE key=?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?
            .map(|row| row.get::<String, _>("value")))
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO settings (key,value) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn normalize_protocol(value: &str) -> Result<&'static str> {
    match value {
        "chat" | "chat_completions" | "openai_chat" => Ok("chat_completions"),
        "responses" | "openai_responses" => Ok("responses"),
        "messages" | "anthropic_messages" | "anthropic" => Ok("anthropic_messages"),
        _ => anyhow::bail!("unsupported provider protocol `{value}`"),
    }
}

fn normalize_auth(value: &str) -> Result<&'static str> {
    match value {
        "bearer" => Ok("bearer"),
        "x_api_key" | "x-api-key" => Ok("x_api_key"),
        _ => anyhow::bail!("unsupported auth_type `{value}`"),
    }
}

fn key_prefix(secret: &str) -> String {
    let chars: Vec<char> = secret.chars().collect();
    if chars.len() <= 12 {
        return format!("{}…", chars.iter().take(4).collect::<String>());
    }
    format!(
        "{}…{}",
        chars.iter().take(8).collect::<String>(),
        chars.iter().skip(chars.len() - 4).collect::<String>()
    )
}

fn hash_key(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hex(&hasher.finalize())
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
