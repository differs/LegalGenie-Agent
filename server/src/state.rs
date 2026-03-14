use crate::config::AppConfig;
use crate::rate_limit::RateLimiter;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub pool: SqlitePool,
    pub rate_limiter: Arc<RateLimiter>,
}

impl AppState {
    pub fn new(config: AppConfig, pool: SqlitePool) -> Self {
        Self {
            config,
            pool,
            rate_limiter: Arc::new(RateLimiter::new()),
        }
    }
}
