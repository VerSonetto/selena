#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use selena_core::Core;
use selena_core::config::{ConfigPaths, load_or_create};
use selena_core::runtime::{process_is_alive, read_runtime};
use selena_server::host;
use std::sync::Arc;
use std::time::Duration;
use tauri::{WebviewUrl, WebviewWindowBuilder};

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let result = tauri::async_runtime::block_on(async {
                let paths = ConfigPaths::resolve()?;
                let config = load_or_create(&paths).await?;

                if let Some(runtime) = read_runtime(&paths.runtime_file)
                    && process_is_alive(runtime.pid)
                {
                    let healthy = reqwest::Client::new()
                        .get(format!("{}/health", runtime.base_url()))
                        .timeout(Duration::from_millis(900))
                        .send()
                        .await
                        .map(|response| response.status().is_success())
                        .unwrap_or(false);
                    if healthy {
                        return url::Url::parse(&runtime.admin_url()).map_err(Into::into);
                    }
                }

                let core = Arc::new(Core::new(config, paths).await?);
                let host_name = core.config.host.clone();
                let port = core.config.port;
                let prepared = match host::prepare(core.clone(), &host_name, port).await {
                    Ok(prepared) => prepared,
                    Err(_) => host::prepare(core, &host_name, 0).await?,
                };
                let url = url::Url::parse(&prepared.runtime.admin_url())?;
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = host::serve_prepared(prepared).await {
                        tracing::error!("Selena desktop server stopped: {error}");
                    }
                });
                Ok::<_, anyhow::Error>(url)
            })?;

            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(result))
                .title("Selena")
                .inner_size(1240.0, 780.0)
                .min_inner_size(1000.0, 680.0)
                .decorations(false)
                .resizable(true)
                .center()
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("run Selena desktop");
}
