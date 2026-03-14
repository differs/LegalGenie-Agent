#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server_host: String,
    pub server_port: u16,
    pub database_url: String,
    pub cors_origins: CorsOrigins,
    pub jwt_secret: String,
    pub access_token_expire_minutes: i64,
    pub refresh_token_expire_days: i64,
    pub storage_path: String,
    pub max_file_size: u64,
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

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let server_host = env_string("SERVER_HOST", "127.0.0.1");
        let server_port = env_u16("SERVER_PORT", 8000)?;
        let database_url = env_string("DATABASE_URL", "sqlite://data/legal_minds.db");
        let jwt_secret = env_string("JWT_SECRET", "change-me-to-a-long-random-secret");
        let access_token_expire_minutes = env_i64("ACCESS_TOKEN_EXPIRE_MINUTES", 60)?;
        let refresh_token_expire_days = env_i64("REFRESH_TOKEN_EXPIRE_DAYS", 7)?;
        let storage_path = env_string("STORAGE_PATH", "./storage");
        let max_file_size = env_u64("MAX_FILE_SIZE", 104_857_600)?;
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

        Ok(Self {
            server_host,
            server_port,
            database_url,
            cors_origins,
            jwt_secret,
            access_token_expire_minutes,
            refresh_token_expire_days,
            storage_path,
            max_file_size,
            temp_path,
            tessdata_dir,
            whisper_model_path,
            asr_language,
            asr_threads,
        })
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }
}

fn env_string(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_u16(key: &str, default: u16) -> anyhow::Result<u16> {
    match std::env::var(key) {
        Ok(v) => Ok(v.parse::<u16>()?),
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
