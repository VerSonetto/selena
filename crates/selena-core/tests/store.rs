use selena_core::db::Database;
use selena_core::models::{
    CreateClientKey, CreateProvider, CreateProviderKey, NewRequestLog, RequestLogCompletion,
};
use selena_core::secrets::SecretBox;
use selena_core::store::Store;

#[tokio::test]
async fn provider_key_and_client_key_flow() {
    let dir = tempfile::tempdir().unwrap();
    let database = Database::connect(&dir.path().join("selena.db"))
        .await
        .unwrap();
    let secrets = SecretBox::load_or_create(&dir.path().join("master.key")).unwrap();
    let store = Store::new(database.pool, secrets);

    let provider = store
        .create_provider(CreateProvider {
            name: "test-provider".to_string(),
            protocol: "responses".to_string(),
            base_url: "https://example.com/v1".to_string(),
            auth_type: "bearer".to_string(),
            extra_headers: None,
            timeout_ms: None,
        })
        .await
        .unwrap();
    assert_eq!(provider.protocol, "responses");

    let key = store
        .create_provider_key(
            &provider.id,
            CreateProviderKey {
                label: "primary".to_string(),
                secret: "sk-test-secret-123456".to_string(),
            },
        )
        .await
        .unwrap();
    assert!(key.prefix.contains('…'));
    let selected = store
        .select_provider_key(&provider.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.1, "sk-test-secret-123456");

    let client = store
        .create_client_key(CreateClientKey {
            name: "laptop".to_string(),
            rate_limit_rpm: Some(60),
        })
        .await
        .unwrap();
    assert!(
        store
            .verify_client_key(&client.secret)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .verify_client_key("sk-selena-wrong")
            .await
            .unwrap()
            .is_none()
    );

    store
        .insert_request_log(NewRequestLog {
            ingress_protocol: "responses".to_string(),
            egress_protocol: "responses".to_string(),
            provider_id: Some(provider.id.clone()),
            provider_name: Some(provider.name.clone()),
            model_requested: "my-model".to_string(),
            model_upstream: "upstream-model".to_string(),
            client_key_id: Some(client.id.clone()),
            status_code: 200,
            error_code: None,
            stream: true,
            conversion_mode: "passthrough".to_string(),
            latency_ms: 12,
            ttft_ms: None,
            input_tokens: Some(10),
            output_tokens: Some(5),
            total_tokens: Some(15),
            cached_tokens: Some(2),
            cache_write_tokens: Some(1),
            reasoning_tokens: Some(0),
            conversion_warnings: vec![],
            upstream_request_id: Some("req_test".to_string()),
            error_message: None,
        })
        .await
        .unwrap();

    let overview = store.overview("127.0.0.1", 8787).await.unwrap();
    assert_eq!(overview.providers, 1);
    assert_eq!(overview.requests_today, 1);
    assert_eq!(overview.input_tokens_today, 10);
    assert_eq!(overview.output_tokens_today, 5);
    assert_eq!(overview.recent_requests.len(), 1);

    let logs = store.list_request_logs(1, 0).await.unwrap();
    store
        .update_request_log(
            &logs[0].id,
            RequestLogCompletion {
                status_code: 200,
                latency_ms: 240,
                ttft_ms: Some(80),
                input_tokens: Some(12),
                output_tokens: Some(7),
                total_tokens: Some(19),
                cached_tokens: Some(3),
                cache_write_tokens: Some(2),
                reasoning_tokens: Some(4),
                conversion_warnings: vec!["stream usage observed".to_string()],
                error_code: None,
                error_message: None,
            },
        )
        .await
        .unwrap();

    let report = store.usage_report(7).await.unwrap();
    assert_eq!(report.summary.requests, 1);
    assert_eq!(report.summary.usage_requests, 1);
    assert_eq!(report.summary.total_tokens, 19);
    assert_eq!(report.summary.cache_write_tokens, 2);
    assert_eq!(report.summary.avg_ttft_ms, Some(80.0));
    assert_eq!(report.by_provider[0].label, "test-provider");
    assert_eq!(report.daily.len(), 7);
}
