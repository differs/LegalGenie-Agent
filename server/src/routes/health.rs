use crate::api::ApiEnvelope;
use crate::state::AppState;
use axum::{extract::State, Json};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub database: &'static str,
}

pub async fn health(State(state): State<AppState>) -> Json<ApiEnvelope<HealthResponse>> {
    let db_ok = sqlx::query_scalar::<_, i64>("SELECT 1::bigint")
        .fetch_one(&state.pool)
        .await
        .is_ok();

    Json(ApiEnvelope::ok(HealthResponse {
        status: if db_ok { "ok" } else { "degraded" },
        version: env!("CARGO_PKG_VERSION"),
        database: if db_ok { "ok" } else { "error" },
    }))
}
