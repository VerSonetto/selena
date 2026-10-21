use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub theme: String,
    pub default_max_tokens: u32,
    pub log_retention_days: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 13697,
            theme: "ivory".to_string(),
            default_max_tokens: 4096,
            log_retention_days: 30,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigPaths {
    pub home: PathBuf,
    pub config_file: PathBuf,
    pub database_file: PathBuf,
    pub master_key_file: PathBuf,
    pub runtime_file: PathBuf,
    pub web_dist: PathBuf,
}

impl ConfigPaths {
    pub fn resolve() -> Result<Self> {
        let home = std::env::var_os("SELENA_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".selena")))
            .context("unable to resolve Selena home directory")?;
        let config_file = home.join("config.toml");
        let web_dist = std::env::var_os("SELENA_WEB_DIST")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/dist"));
        Ok(Self {
            database_file: home.join("selena.db"),
            master_key_file: home.join("master.key"),
            runtime_file: home.join("runtime.json"),
            config_file,
            web_dist,
            home,
        })
    }

    pub fn ensure_home(&self) -> Result<()> {
        std::fs::create_dir_all(&self.home)
            .with_context(|| format!("create Selena home {}", self.home.display()))?;
        set_private_dir(&self.home);
        Ok(())
    }
}

pub async fn load_file(paths: &ConfigPaths) -> Result<AppConfig> {
    if paths.config_file.exists() {
        let raw = tokio::fs::read_to_string(&paths.config_file).await?;
        toml::from_str::<AppConfig>(&raw).context("parse config.toml")
    } else {
        Ok(AppConfig::default())
    }
}

pub async fn set_port(paths: &ConfigPaths, port: u16) -> Result<()> {
    let mut config = load_file(paths).await?;
    config.port = port;
    save(paths, &config).await
}

pub async fn load_or_create(paths: &ConfigPaths) -> Result<AppConfig> {
    paths.ensure_home()?;
    let mut config = if paths.config_file.exists() {
        load_file(paths).await?
    } else {
        let config = AppConfig::default();
        save(paths, &config).await?;
        config
    };

    if let Ok(host) = std::env::var("SELENA_HOST") {
        config.host = host;
    }
    if let Ok(port) = std::env::var("SELENA_PORT")
        && let Ok(port) = port.parse::<u16>()
    {
        config.port = port;
    }
    Ok(config)
}

pub async fn save(paths: &ConfigPaths, config: &AppConfig) -> Result<()> {
    paths.ensure_home()?;
    let raw = toml::to_string_pretty(config)?;
    tokio::fs::write(&paths.config_file, raw).await?;
    Ok(())
}

pub fn set_private_file(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

fn set_private_dir(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
    }
    #[cfg(not(unix))]
    let _ = path;
}
