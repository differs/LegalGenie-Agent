use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::path::Path;
use std::str::FromStr;

pub type DbPool = PgPool;

/// Applies the PostgreSQL schema migrations.
pub async fn run_migrations(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations_pg")
        .run(pool)
        .await
        .map_err(|e| anyhow::anyhow!("postgres migrations failed: {e}"))
}

pub async fn create_pool(database_url: &str) -> anyhow::Result<PgPool> {
    let mut options = PgConnectOptions::from_str(database_url)
        .map_err(|e| anyhow::anyhow!("invalid DATABASE_URL: {e}"))?;
    options = options.application_name("legalgenie-server");

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .idle_timeout(std::time::Duration::from_secs(600))
        .connect_with(options)
        .await
        .map_err(|e| anyhow::anyhow!("postgres connect failed: {e}"))?;

    run_migrations(&pool).await?;

    Ok(pool)
}

/// Optional helper kept for migration tooling; not used by the server.
#[allow(dead_code)]
fn _sqlite_hint() {
    let _ = Path::new("data/legal_minds.db");
}
