use crate::config::{AppConfig, TranslationConfig};
use crate::rate_limit::RateLimiter;
use crate::translation::{self, TranslationProvider};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub translation: TranslationConfig,
    pub translation_provider: Arc<dyn TranslationProvider>,
    pub pool: PgPool,
    pub rate_limiter: Arc<RateLimiter>,
}

impl AppState {
    pub fn new(config: AppConfig, pool: PgPool) -> Self {
        Self::new_with_translation(config, pool, TranslationConfig::default())
    }

    pub fn new_with_translation(
        config: AppConfig,
        pool: PgPool,
        translation: TranslationConfig,
    ) -> Self {
        Self::try_new_with_translation(config, pool, translation)
            .expect("build translation provider from config")
    }

    pub fn try_new_with_translation(
        config: AppConfig,
        pool: PgPool,
        translation: TranslationConfig,
    ) -> anyhow::Result<Self> {
        let provider = translation::provider_from_config(&translation)?;
        Ok(Self::new_with_translation_provider(
            config,
            pool,
            translation,
            provider,
        ))
    }

    pub fn new_with_translation_provider(
        config: AppConfig,
        pool: PgPool,
        translation: TranslationConfig,
        translation_provider: Arc<dyn TranslationProvider>,
    ) -> Self {
        Self {
            config,
            translation,
            translation_provider,
            pool,
            rate_limiter: Arc::new(RateLimiter::new()),
        }
    }
}
