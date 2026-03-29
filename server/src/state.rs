use crate::config::{AppConfig, TranslationConfig};
use crate::rate_limit::RateLimiter;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub translation: TranslationConfig,
    pub pool: SqlitePool,
    pub rate_limiter: Arc<RateLimiter>,
}

impl AppState {
    pub fn new(config: AppConfig, pool: SqlitePool) -> Self {
        Self::new_with_translation(config, pool, TranslationConfig::default())
    }

    pub fn new_with_translation(
        config: AppConfig,
        pool: SqlitePool,
        translation: TranslationConfig,
    ) -> Self {
        Self {
            config,
            translation,
            pool,
            rate_limiter: Arc::new(RateLimiter::new()),
        }
    }
}
