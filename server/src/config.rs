#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEnv {
    Development,
    Test,
    Production,
}

impl AppEnv {
    fn from_str(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "prod" | "production" => Self::Production,
            "test" => Self::Test,
            _ => Self::Development,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub app_env: AppEnv,
    pub server_host: String,
    pub server_port: u16,
    pub database_url: String,
    pub cors_origins: CorsOrigins,
    pub force_https: bool,
    pub jwt_secret: String,
    pub access_token_expire_minutes: i64,
    pub refresh_token_expire_days: i64,
    pub storage_path: String,
    pub max_file_size: u64,
    pub allowed_file_types: Vec<String>,
    pub temp_path: String,
    pub tessdata_dir: String,
    pub whisper_model_path: String,
    pub asr_language: String,
    pub asr_threads: u16,
}

#[derive(Debug, Clone)]
pub enum CorsOrigins {
    Any,
    AllowList(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct TranslationConfig {
    pub provider: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub target_language: String,
    pub max_concurrency: u16,
    pub chunk_size_limit: u32,
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            provider: "disabled".to_string(),
            base_url: None,
            api_key: None,
            model: None,
            target_language: "en".to_string(),
            max_concurrency: 2,
            chunk_size_limit: 2_000,
        }
    }
}

impl TranslationConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let defaults = Self::default();
        let provider = env_string("TRANSLATION_PROVIDER", &defaults.provider);
        let base_url = env_optional_string("TRANSLATION_BASE_URL");
        let api_key = env_optional_string("TRANSLATION_API_KEY");
        let model = env_optional_string("TRANSLATION_MODEL");
        let target_language =
            env_string("TRANSLATION_TARGET_LANGUAGE", &defaults.target_language);
        let max_concurrency = env_u16("TRANSLATION_MAX_CONCURRENCY", defaults.max_concurrency)?;
        let chunk_size_limit =
            env_u32("TRANSLATION_CHUNK_SIZE_LIMIT", defaults.chunk_size_limit)?;

        if max_concurrency == 0 {
            anyhow::bail!("TRANSLATION_MAX_CONCURRENCY must be >= 1");
        }
        if chunk_size_limit == 0 {
            anyhow::bail!("TRANSLATION_CHUNK_SIZE_LIMIT must be >= 1");
        }

        Ok(Self {
            provider,
            base_url,
            api_key,
            model,
            target_language,
            max_concurrency,
            chunk_size_limit,
        })
    }
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let app_env = AppEnv::from_str(&env_string("APP_ENV", "development"));
        let server_host = env_string("SERVER_HOST", "127.0.0.1");
        let server_port = env_u16("SERVER_PORT", 8000)?;
        let database_url = env_string("DATABASE_URL", "sqlite://data/legal_minds.db");
        let force_https = env_bool("FORCE_HTTPS", false)?;
        let jwt_secret = env_string("JWT_SECRET", "change-me-to-a-long-random-secret");
        let access_token_expire_minutes = env_i64("ACCESS_TOKEN_EXPIRE_MINUTES", 60)?;
        let refresh_token_expire_days = env_i64("REFRESH_TOKEN_EXPIRE_DAYS", 7)?;
        let storage_path = env_string("STORAGE_PATH", "./storage");
        let max_file_size = env_u64("MAX_FILE_SIZE", 104_857_600)?;
        let allowed_file_types = parse_allowed_file_types(&env_string(
            "ALLOWED_FILE_TYPES",
            DEFAULT_ALLOWED_FILE_TYPES,
        ));
        let temp_path = env_string(
            "TEMP_PATH",
            &format!("{}/temp", storage_path.trim_end_matches('/')),
        );
        let tessdata_dir = env_string("TESSDATA_DIR", "./data/tessdata");
        let whisper_model_path =
            env_string("WHISPER_MODEL_PATH", "./data/models/whisper/ggml-tiny.bin");
        let asr_language = env_string("ASR_LANGUAGE", "zh");
        let asr_threads = env_u16("ASR_THREADS", default_asr_threads())?;

        let cors_origins = match std::env::var("CORS_ORIGINS") {
            Ok(v) => parse_cors_origins(&v),
            Err(_) => CorsOrigins::Any,
        };

        let cfg = Self {
            app_env,
            server_host,
            server_port,
            database_url,
            cors_origins,
            force_https,
            jwt_secret,
            access_token_expire_minutes,
            refresh_token_expire_days,
            storage_path,
            max_file_size,
            allowed_file_types,
            temp_path,
            tessdata_dir,
            whisper_model_path,
            asr_language,
            asr_threads,
        };

        cfg.validate()?;
        Ok(cfg)
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.app_env == AppEnv::Production {
            let secret = self.jwt_secret.trim();
            if secret == "change-me-to-a-long-random-secret" || secret.len() < 32 {
                anyhow::bail!(
                    "JWT_SECRET must be at least 32 chars and not the default value in production"
                );
            }

            if matches!(self.cors_origins, CorsOrigins::Any) {
                anyhow::bail!("CORS_ORIGINS must be an allowlist (not '*') in production");
            }

            if !self.force_https {
                anyhow::bail!("FORCE_HTTPS must be enabled in production");
            }
        }

        Ok(())
    }
}

fn env_string(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_optional_string(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn env_u16(key: &str, default: u16) -> anyhow::Result<u16> {
    match std::env::var(key) {
        Ok(v) => Ok(v.parse::<u16>()?),
        Err(_) => Ok(default),
    }
}

fn env_u32(key: &str, default: u32) -> anyhow::Result<u32> {
    match std::env::var(key) {
        Ok(v) => Ok(v.parse::<u32>()?),
        Err(_) => Ok(default),
    }
}

fn env_i64(key: &str, default: i64) -> anyhow::Result<i64> {
    match std::env::var(key) {
        Ok(v) => Ok(v.parse::<i64>()?),
        Err(_) => Ok(default),
    }
}

fn env_u64(key: &str, default: u64) -> anyhow::Result<u64> {
    match std::env::var(key) {
        Ok(v) => Ok(v.parse::<u64>()?),
        Err(_) => Ok(default),
    }
}

fn env_bool(key: &str, default: bool) -> anyhow::Result<bool> {
    match std::env::var(key) {
        Ok(v) => match v.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            _ => Err(anyhow::anyhow!("invalid bool for {key}: {v}")),
        },
        Err(_) => Ok(default),
    }
}

fn default_asr_threads() -> u16 {
    std::thread::available_parallelism()
        .map(|n| n.get().min(u16::MAX as usize) as u16)
        .unwrap_or(4)
}

fn parse_cors_origins(raw: &str) -> CorsOrigins {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "*" {
        return CorsOrigins::Any;
    }

    let origins = trimmed
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    if origins.is_empty() {
        CorsOrigins::Any
    } else {
        CorsOrigins::AllowList(origins)
    }
}

const DEFAULT_ALLOWED_FILE_TYPES: &str = "pdf,doc,docx,xls,xlsx,jpg,jpeg,png,mp3,wav,txt,json";

fn parse_allowed_file_types(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return DEFAULT_ALLOWED_FILE_TYPES
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }

    let mut out = trimmed
        .split(',')
        .map(|s| s.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .filter(|s| s.chars().all(|c| c.is_ascii_alphanumeric()))
        .collect::<Vec<_>>();

    if out.is_empty() {
        out = DEFAULT_ALLOWED_FILE_TYPES
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }

    out
}
