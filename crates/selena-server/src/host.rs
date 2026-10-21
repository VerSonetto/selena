use crate::{AppState, build_router};
use anyhow::{Context, Result};
use selena_core::runtime::{RuntimeInfo, remove_runtime, write_runtime};
use selena_core::{Core, SharedCore};
use std::sync::Arc;
use tokio::net::TcpListener;

pub struct PreparedServer {
    pub listener: TcpListener,
    pub runtime: RuntimeInfo,
    pub state: AppState,
    pub core: SharedCore,
}

pub async fn prepare(core: Arc<Core>, host: &str, port: u16) -> Result<PreparedServer> {
    let listener = TcpListener::bind((host, port))
        .await
        .with_context(|| format!("bind Selena server on {host}:{port}"))?;
    let address = listener.local_addr()?;
    let runtime = RuntimeInfo::new(host, address.port());
    write_runtime(&core.paths.runtime_file, &runtime).await?;
    let state = AppState {
        core: core.clone(),
        admin_token: runtime.admin_token.clone(),
        runtime: runtime.clone(),
    };
    Ok(PreparedServer {
        listener,
        runtime,
        state,
        core,
    })
}

pub async fn serve_prepared(prepared: PreparedServer) -> Result<()> {
    let app = build_router(prepared.state);
    let result = axum::serve(prepared.listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await;
    remove_runtime(&prepared.core.paths.runtime_file).await;
    result.context("serve Selena HTTP")?;
    Ok(())
}

pub async fn serve(core: Arc<Core>, host: &str, port: u16) -> Result<RuntimeInfo> {
    let prepared = prepare(core, host, port).await?;
    let runtime = prepared.runtime.clone();
    tracing::info!("Selena listening on {}", runtime.base_url());
    serve_prepared(prepared).await?;
    Ok(runtime)
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("Selena shutting down");
}
