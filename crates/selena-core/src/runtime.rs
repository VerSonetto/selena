use anyhow::Result;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub pid: u32,
    pub host: String,
    pub port: u16,
    pub admin_token: String,
    pub started_at: i64,
    pub version: String,
}

impl RuntimeInfo {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            pid: std::process::id(),
            host: host.into(),
            port,
            admin_token: generate_admin_token(),
            started_at: chrono::Utc::now().timestamp(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn base_url(&self) -> String {
        let host = if self.host == "0.0.0.0" || self.host == "::" {
            "127.0.0.1"
        } else {
            &self.host
        };
        format!("http://{}:{}", host, self.port)
    }

    pub fn admin_url(&self) -> String {
        format!("{}/?token={}", self.base_url(), self.admin_token)
    }
}

pub fn generate_admin_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("sk-admin-{}", hex(&bytes))
}

pub fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

pub async fn write_runtime(path: &Path, info: &RuntimeInfo) -> Result<()> {
    let raw = serde_json::to_vec_pretty(info)?;
    tokio::fs::write(path, raw).await?;
    crate::config::set_private_file(path);
    Ok(())
}

pub fn read_runtime(path: &Path) -> Option<RuntimeInfo> {
    let raw = std::fs::read(path).ok()?;
    serde_json::from_slice(&raw).ok()
}

pub async fn remove_runtime(path: &Path) {
    let _ = tokio::fs::remove_file(path).await;
}

pub fn process_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(windows)]
    {
        let output = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output();
        output
            .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        std::path::Path::new("/proc").join(pid.to_string()).exists()
    }
}
