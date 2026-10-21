use anyhow::{Context, Result};
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::RngCore;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct SecretBox {
    key: Arc<[u8; 32]>,
}

impl SecretBox {
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if let Ok(raw) = std::env::var("SELENA_MASTER_KEY") {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(raw.trim())
                .context("SELENA_MASTER_KEY must be base64")?;
            let key: [u8; 32] = bytes
                .try_into()
                .map_err(|_| anyhow::anyhow!("SELENA_MASTER_KEY must decode to 32 bytes"))?;
            return Ok(Self { key: Arc::new(key) });
        }

        if path.exists() {
            let raw = std::fs::read(path)?;
            let key: [u8; 32] = raw
                .try_into()
                .map_err(|_| anyhow::anyhow!("master.key must contain 32 bytes"))?;
            return Ok(Self { key: Arc::new(key) });
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        std::fs::write(path, key)?;
        crate::config::set_private_file(path);
        Ok(Self { key: Arc::new(key) })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let cipher = XChaCha20Poly1305::new((&*self.key).into());
        let mut nonce = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(XNonce::from_slice(&nonce), plaintext.as_bytes())
            .map_err(|_| anyhow::anyhow!("secret encryption failed"))?;
        let mut combined = Vec::with_capacity(24 + ciphertext.len());
        combined.extend_from_slice(&nonce);
        combined.extend_from_slice(&ciphertext);
        Ok(base64::engine::general_purpose::STANDARD.encode(combined))
    }

    pub fn decrypt(&self, encoded: &str) -> Result<String> {
        let combined = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .context("invalid encrypted secret")?;
        if combined.len() < 25 {
            anyhow::bail!("invalid encrypted secret length");
        }
        let (nonce, ciphertext) = combined.split_at(24);
        let cipher = XChaCha20Poly1305::new((&*self.key).into());
        let plaintext = cipher
            .decrypt(XNonce::from_slice(nonce), ciphertext)
            .map_err(|_| anyhow::anyhow!("secret decryption failed"))?;
        String::from_utf8(plaintext).context("secret is not UTF-8")
    }
}
