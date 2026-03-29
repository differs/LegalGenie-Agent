use crate::errors::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::{header, request::Parts};
use std::net::SocketAddr;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RequestMeta {
    pub request_id: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[axum::async_trait]
impl FromRequestParts<AppState> for RequestMeta {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> AppResult<Self> {
        let user_agent = parts
            .headers
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let _ = state;
        let ip_address = extract_connect_ip(parts);

        Ok(Self {
            request_id: Uuid::new_v4().to_string(),
            ip_address,
            user_agent,
        })
    }
}

fn extract_connect_ip(parts: &Parts) -> Option<String> {
    parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip().to_string())
}
