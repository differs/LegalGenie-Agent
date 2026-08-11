use anyhow::Context;
use legalminds_server::{create_pool, router, translation, AppConfig, AppState, TranslationConfig};
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let config = AppConfig::from_env().context("load config")?;
    let translation = TranslationConfig::from_env().context("load translation config")?;

    tokio::fs::create_dir_all(&config.storage_path)
        .await
        .context("init storage dir")?;

    tokio::fs::create_dir_all(&config.temp_path)
        .await
        .context("init temp dir")?;

    tokio::fs::create_dir_all(&config.tessdata_dir)
        .await
        .context("init tessdata dir")?;

    if let Some(parent) = std::path::Path::new(&config.whisper_model_path).parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("init whisper model dir")?;
        }
    }

    let pool = create_pool(&config.database_url)
        .await
        .context("connect database")?;

    let state = AppState::try_new_with_translation(config, pool, translation)
        .context("build app state with translation provider")?;

    // ---------- Graceful shutdown ----------
    let shutdown = CancellationToken::new();
    let shutdown_for_signal = shutdown.clone();
    tokio::spawn(async move {
        let ctrl_c = tokio::signal::ctrl_c();
        #[cfg(unix)]
        let terminate = {
            use tokio::signal::unix::{signal, SignalKind};
            let mut stream = signal(SignalKind::terminate()).expect("install SIGTERM handler");
            async move { stream.recv().await }
        };
        #[cfg(not(unix))]
        let terminate = std::future::pending::<Option<()>>();

        tokio::select! {
            _ = ctrl_c => tracing::info!("SIGINT received, draining…"),
            _ = terminate => tracing::info!("SIGTERM received, draining…"),
        }
        shutdown_for_signal.cancel();
    });

    // ---------- Background workers ----------
    // Recover jobs left by a crashed previous instance, then run workers.
    if let Err(err) = legalminds_server::jobs::reset_stale_jobs(&state.pool).await {
        tracing::warn!(error = %err, "initial stale job sweep failed");
    }
    let worker_count = std::env::var("JOB_WORKERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2usize)
        .max(1);
    let workers = legalminds_server::job_worker::spawn_job_workers(
        state.clone(),
        shutdown.clone(),
        worker_count,
    );

    translation::start_retry_poller_once(state.clone());

    // WORKER_ONLY=1: dedicated worker process without the HTTP server
    // (docker-compose `worker` service / k8s deployment pattern).
    if std::env::var("WORKER_ONLY").ok().as_deref() == Some("1") {
        tracing::info!(worker_count, "worker-only mode; no HTTP server");
        shutdown.cancelled().await;
        tracing::info!("worker-only shutdown; waiting for in-flight jobs…");
        let _ = tokio::time::timeout(std::time::Duration::from_secs(60), workers).await;
        tracing::info!("worker-only shutdown complete");
        return Ok(());
    }

    let app = router(state.clone());

    let addr = state.config.bind_addr();
    tracing::info!(%addr, worker_count, "legalminds-server listening");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("bind {addr}"))?;

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown.cancelled().await;
    })
    .await
    .context("serve")?;

    tracing::info!("http server drained; waiting for workers to finish in-flight jobs…");
    let _ = tokio::time::timeout(std::time::Duration::from_secs(60), workers).await;
    tracing::info!("shutdown complete");
    Ok(())
}
