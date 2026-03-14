use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::path::Path;

pub async fn create_pool(database_url: &str) -> anyhow::Result<SqlitePool> {
    ensure_sqlite_dir(database_url).await?;

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .idle_timeout(std::time::Duration::from_secs(600))
        .connect(database_url)
        .await?;

    // Reasonable defaults for a single-file SQLite DB in a web server.
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA synchronous = NORMAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA cache_size = -64000")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA temp_store = MEMORY")
        .execute(&pool)
        .await?;

    Ok(pool)
}

async fn ensure_sqlite_dir(database_url: &str) -> anyhow::Result<()> {
    // sqlx uses URLs like:
    // - sqlite::memory:
    // - sqlite://relative/path.db
    // - sqlite:///absolute/path.db
    //
    // For the common local dev case, ensure the parent directory exists.
    if database_url == "sqlite::memory:" {
        return Ok(());
    }

    let Some(path) = database_url.strip_prefix("sqlite://") else {
        return Ok(());
    };

    let path = path.split('?').next().unwrap_or(path);

    // Absolute paths are `sqlite:///...` -> `path` starts with `/`.
    // If it's absolute and points outside the project, we should not try to create dirs.
    if path.starts_with('/') {
        return Ok(());
    }

    let db_path = Path::new(path);
    let Some(parent) = db_path.parent() else {
        return Ok(());
    };

    if parent.as_os_str().is_empty() {
        return Ok(());
    }

    tokio::fs::create_dir_all(parent).await?;

    // Some SQLx configurations don't create the DB file implicitly. Create an
    // empty file so opening with SQLite succeeds.
    if !tokio::fs::try_exists(db_path).await? {
        tokio::fs::File::create(db_path).await?;
    }
    Ok(())
}
