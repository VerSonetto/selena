use crate::models::Provider;
use anyhow::{Context, Result};
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use std::str::FromStr;

pub struct ProviderClient<'a> {
    http: &'a reqwest::Client,
    provider: &'a Provider,
    key: &'a str,
}

impl<'a> ProviderClient<'a> {
    pub fn new(http: &'a reqwest::Client, provider: &'a Provider, key: &'a str) -> Self {
        Self {
            http,
            provider,
            key,
        }
    }

    pub fn endpoint(&self) -> String {
        format!(
            "{}{}",
            self.provider.base_url.trim_end_matches('/'),
            self.provider.protocol().provider_path()
        )
    }

    fn headers(&self, stream: bool) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if stream {
            headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        }
        match self.provider.auth_type.as_str() {
            "x_api_key" => {
                headers.insert(
                    HeaderName::from_static("x-api-key"),
                    HeaderValue::from_str(self.key)?,
                );
            }
            _ => {
                headers.insert(
                    reqwest::header::AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {}", self.key))?,
                );
            }
        }
        if self.provider.protocol() == selena_protocol::Protocol::Messages {
            headers.insert(
                HeaderName::from_static("anthropic-version"),
                HeaderValue::from_static("2023-06-01"),
            );
        }
        if let Ok(extra) = serde_json::from_str::<Value>(&self.provider.extra_headers)
            && let Some(extra) = extra.as_object()
        {
            for (name, value) in extra {
                if let (Ok(name), Some(value)) = (
                    HeaderName::from_str(name),
                    value.as_str().map(ToString::to_string),
                ) && let Ok(value) = HeaderValue::from_str(&value)
                {
                    headers.insert(name, value);
                }
            }
        }
        Ok(headers)
    }

    pub async fn post_json(&self, body: &Value, stream: bool) -> Result<reqwest::Response> {
        let request = self
            .http
            .post(self.endpoint())
            .headers(self.headers(stream)?)
            .timeout(std::time::Duration::from_millis(
                self.provider.timeout_ms.max(1) as u64,
            ))
            .json(body)
            .build()
            .context("build upstream request")?;
        self.http
            .execute(request)
            .await
            .context("send upstream request")
    }

    pub async fn get_models(&self) -> Result<Value> {
        let url = format!("{}/models", self.provider.base_url.trim_end_matches('/'));
        let response = self
            .http
            .get(url)
            .headers(self.headers(false)?)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .context("request provider models")?;
        let status = response.status();
        let bytes = response.bytes().await?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            anyhow::bail!(
                "models endpoint returned HTTP {status}: {}",
                body.chars().take(300).collect::<String>()
            );
        }
        serde_json::from_slice(&bytes).context("parse provider models response")
    }
}
