use anyhow::Context;
use legalminds_server::{
    create_pool, router, translation, AppConfig, AppState, TranslationConfig,
};
use std::net::SocketAddr;

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

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("run migrations")?;

    let state = AppState::try_new_with_translation(config, pool, translation)
        .context("build app state with translation provider")?;
    translation::start_retry_poller_once(state.clone());
    let app = router(state.clone());

    let addr = state.config.bind_addr();
    tracing::info!(%addr, "legalminds-server listening");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("bind {addr}"))?;

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("serve")?;
    Ok(())
}
