use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiEnvelope<T>
where
    T: Serialize,
{
    /// HTTP status code (mirrors the response HTTP status).
    pub code: u16,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    pub timestamp: i64,
    /// Optional domain/business error code (see docs/错误处理规范.md).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<i32>,
}

impl<T> ApiEnvelope<T>
where
    T: Serialize,
{
    pub fn ok(data: T) -> Self {
        Self {
            code: 200,
            message: "success".to_string(),
            data: Some(data),
            timestamp: now_ts(),
            error_code: None,
        }
    }
}

impl ApiEnvelope<()> {
    pub fn err(code: u16, error_code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
            timestamp: now_ts(),
            error_code: Some(error_code),
        }
    }
}

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}
