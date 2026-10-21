use anyhow::{Context, Result};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS providers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    protocol TEXT NOT NULL,
    base_url TEXT NOT NULL,
    auth_type TEXT NOT NULL DEFAULT 'bearer',
    extra_headers TEXT NOT NULL DEFAULT '{}',
    enabled INTEGER NOT NULL DEFAULT 1,
    timeout_ms INTEGER NOT NULL DEFAULT 300000,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS provider_keys (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    secret_enc TEXT NOT NULL,
    prefix TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'ready',
    cooldown_until INTEGER,
    failure_count INTEGER NOT NULL DEFAULT 0,
    last_used_at INTEGER,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_provider_keys_provider ON provider_keys(provider_id);

CREATE TABLE IF NOT EXISTS routes (
    id TEXT PRIMARY KEY,
    model_alias TEXT NOT NULL UNIQUE,
    provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    upstream_model TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS client_keys (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    key_hash TEXT NOT NULL UNIQUE,
    key_prefix TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    rate_limit_rpm INTEGER,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS request_logs (
    id TEXT PRIMARY KEY,
    ts INTEGER NOT NULL,
    ingress_protocol TEXT NOT NULL,
    egress_protocol TEXT NOT NULL,
    provider_id TEXT,
    provider_name TEXT,
    model_requested TEXT NOT NULL,
    model_upstream TEXT NOT NULL,
    client_key_id TEXT,
    status_code INTEGER NOT NULL,
    error_code TEXT,
    stream INTEGER NOT NULL DEFAULT 0,
    conversion_mode TEXT NOT NULL,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    ttft_ms INTEGER,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cached_tokens INTEGER,
    reasoning_tokens INTEGER,
    conversion_warnings TEXT NOT NULL DEFAULT '[]',
    upstream_request_id TEXT,
    error_message TEXT
);
CREATE INDEX IF NOT EXISTS idx_request_logs_ts ON request_logs(ts DESC);
CREATE INDEX IF NOT EXISTS idx_request_logs_provider ON request_logs(provider_id);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

pub struct Database {
    pub pool: SqlitePool,
}

impl Database {
    pub async fn connect(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let options = SqliteConnectOptions::from_str(&format!(
            "sqlite://{}",
            path.to_string_lossy().replace('\\', "/")
        ))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .context("connect Selena SQLite database")?;
        sqlx::raw_sql(SCHEMA)
            .execute(&pool)
            .await
            .context("initialize Selena schema")?;
        Ok(Self { pool })
    }
}
