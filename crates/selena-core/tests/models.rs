use selena_core::Core;
use selena_core::config::{AppConfig, ConfigPaths};
use selena_core::models::CreateProvider;

fn test_paths(dir: &std::path::Path) -> ConfigPaths {
    ConfigPaths {
        home: dir.to_path_buf(),
        config_file: dir.join("config.toml"),
        database_file: dir.join("selena.db"),
        master_key_file: dir.join("master.key"),
        runtime_file: dir.join("runtime.json"),
        web_dist: dir.join("web"),
    }
}

#[tokio::test]
async fn model_prefix_uses_provider_name() {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::new(AppConfig::default(), test_paths(dir.path()))
        .await
        .unwrap();
    let provider = core
        .store
        .create_provider(CreateProvider {
            name: "QWork".to_string(),
            protocol: "chat_completions".to_string(),
            base_url: "https://example.com/v1".to_string(),
            auth_type: "bearer".to_string(),
            extra_headers: None,
            timeout_ms: None,
        })
        .await
        .unwrap();

    let models = core.list_models().await.unwrap();
    assert!(models.contains(&"QWork/auto".to_string()));
    assert!(!models.contains(&format!("{}/auto", provider.id)));
    assert!(core.store.find_provider("qwork").await.unwrap().is_some());
}

#[tokio::test]
async fn plain_model_uses_default_provider_and_prefix_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::new(AppConfig::default(), test_paths(dir.path()))
        .await
        .unwrap();
    let alpha = core
        .store
        .create_provider(CreateProvider {
            name: "alpha".to_string(),
            protocol: "chat_completions".to_string(),
            base_url: "https://alpha.example.com/v1".to_string(),
            auth_type: "bearer".to_string(),
            extra_headers: None,
            timeout_ms: None,
        })
        .await
        .unwrap();
    let beta = core
        .store
        .create_provider(CreateProvider {
            name: "beta".to_string(),
            protocol: "anthropic_messages".to_string(),
            base_url: "https://beta.example.com/v1".to_string(),
            auth_type: "bearer".to_string(),
            extra_headers: None,
            timeout_ms: None,
        })
        .await
        .unwrap();
    core.store
        .set_setting("default_provider", &beta.id)
        .await
        .unwrap();

    let prefixed = core.resolve_route("alpha/gpt-4.1").await.unwrap();
    assert_eq!(prefixed.provider.id, alpha.id);
    assert_eq!(prefixed.upstream_model, "gpt-4.1");

    let plain = core.resolve_route("claude-sonnet").await.unwrap();
    assert_eq!(plain.provider.id, beta.id);
    assert_eq!(plain.upstream_model, "claude-sonnet");
}

#[tokio::test]
async fn provider_name_cannot_contain_slash() {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::new(AppConfig::default(), test_paths(dir.path()))
        .await
        .unwrap();
    let error = core
        .store
        .create_provider(CreateProvider {
            name: "OpenAI/Prod".to_string(),
            protocol: "chat_completions".to_string(),
            base_url: "https://example.com/v1".to_string(),
            auth_type: "bearer".to_string(),
            extra_headers: None,
            timeout_ms: None,
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("cannot contain"));
}
