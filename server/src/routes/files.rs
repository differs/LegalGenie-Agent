use crate::api::ApiEnvelope;
use crate::context::RequestMeta;
use crate::errors::{AppError, AppResult};
use crate::oplog::{spawn_operation_log, OperationLogNew};
use crate::routes::auth::AuthUser;
use crate::state::AppState;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use std::path::PathBuf;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/:id", get(get_file).delete(delete_file))
        .route("/:id/download", get(download_file))
        .route("/:id/preview", get(preview_file))
        .route("/:id/parse", post(parse_file))
}

#[derive(Debug, sqlx::FromRow)]
struct EvidenceFileRow {
    id: String,
    case_id: String,
    original_name: String,
    file_type: String,
    file_size: i64,
    storage_path: String,
    parse_status: String,
    parse_error: Option<String>,
    parsed_text: Option<String>,
    page_count: Option<i64>,
    duration: Option<i64>,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct EvidenceFileDetail {
    id: String,
    case_id: String,
    original_name: String,
    file_type: String,
    file_size: i64,
    storage_path: String,
    parse_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parsed_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<i64>,
    created_at: String,
}

async fn get_file(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<EvidenceFileDetail>>> {
    let id = normalize_file_id(&id)?;
    let row = fetch_file_row(&state, &user, &id).await?;

    Ok(Json(ApiEnvelope::ok(EvidenceFileDetail {
        id: row.id,
        case_id: row.case_id,
        original_name: row.original_name,
        file_type: row.file_type,
        file_size: row.file_size,
        storage_path: row.storage_path,
        parse_status: row.parse_status,
        parse_error: row.parse_error,
        parsed_text: row.parsed_text,
        page_count: row.page_count,
        duration: row.duration,
        created_at: row.created_at,
    })))
}

async fn delete_file(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<serde_json::Value>>> {
    let id = normalize_file_id(&id)?;
    let row = fetch_file_row(&state, &user, &id).await?;

    let old_value = serde_json::json!({
        "id": row.id.clone(),
        "case_id": row.case_id.clone(),
        "original_name": row.original_name.clone(),
        "file_type": row.file_type.clone(),
        "file_size": row.file_size,
        "storage_path": row.storage_path.clone(),
        "parse_status": row.parse_status.clone(),
        "parse_error": row.parse_error.clone(),
        "page_count": row.page_count,
        "duration": row.duration,
        "created_at": row.created_at.clone(),
    });

    // Soft delete only. Keep physical files for auditability (can be cleaned up later).
    sqlx::query(
        "UPDATE evidence_files SET status = 'deleted' WHERE id = ?1 AND status != 'deleted'",
    )
    .bind(&row.id)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: Some(row.case_id.clone()),
            action: "DELETE".to_string(),
            module: "file".to_string(),
            target_type: "evidence_file".to_string(),
            target_id: Some(row.id.clone()),
            target_title: Some(row.original_name.clone()),
            old_value: Some(old_value),
            new_value: None,
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    Ok(Json(ApiEnvelope::ok(serde_json::json!({}))))
}

async fn download_file(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let id = normalize_file_id(&id)?;
    let row = fetch_file_row(&state, &user, &id).await?;

    let full_path = PathBuf::from(&state.config.storage_path).join(&row.storage_path);
    let file = tokio::fs::File::open(&full_path)
        .await
        .map_err(|_| AppError::not_found("file not found"))?;

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: Some(row.case_id.clone()),
            action: "DOWNLOAD".to_string(),
            module: "file".to_string(),
            target_type: "evidence_file".to_string(),
            target_id: Some(row.id.clone()),
            target_title: Some(row.original_name.clone()),
            old_value: None,
            new_value: Some(serde_json::json!({
                "id": row.id.clone(),
                "case_id": row.case_id.clone(),
                "original_name": row.original_name.clone(),
                "file_type": row.file_type.clone(),
                "file_size": row.file_size,
                "storage_path": row.storage_path.clone(),
            })),
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    let mut resp = Response::new(body);

    let content_type = row
        .file_type
        .parse::<HeaderValue>()
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    resp.headers_mut()
        .insert(header::CONTENT_TYPE, content_type);

    let filename = sanitize_filename(&row.original_name);
    let cd = format!("attachment; filename=\"{}\"", filename);
    if let Ok(v) = cd.parse::<HeaderValue>() {
        resp.headers_mut().insert(header::CONTENT_DISPOSITION, v);
    }

    Ok(resp)
}

async fn preview_file(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let id = normalize_file_id(&id)?;
    let row = fetch_file_row(&state, &user, &id).await?;

    let full_path = PathBuf::from(&state.config.storage_path).join(&row.storage_path);
    let file = tokio::fs::File::open(&full_path)
        .await
        .map_err(|_| AppError::not_found("file not found"))?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    let mut resp = Response::new(body);

    let content_type = row
        .file_type
        .parse::<HeaderValue>()
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    resp.headers_mut()
        .insert(header::CONTENT_TYPE, content_type);

    let filename = sanitize_filename(&row.original_name);
    let cd = format!("inline; filename=\"{}\"", filename);
    if let Ok(v) = cd.parse::<HeaderValue>() {
        resp.headers_mut().insert(header::CONTENT_DISPOSITION, v);
    }

    Ok(resp)
}

async fn parse_file(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<serde_json::Value>>> {
    let id = normalize_file_id(&id)?;
    let row = fetch_file_row(&state, &user, &id).await?;

    if row.parse_status == "processing" {
        return Err(AppError::conflict("file is already processing"));
    }

    let started = crate::parser::enqueue_parse(
        state.pool.clone(),
        state.config.clone(),
        row.id.clone(),
        true,
    )
    .await
    .map_err(|e| AppError::internal(format!("enqueue parse failed: {e}")))?;

    if !started {
        return Err(AppError::not_found("file not found"));
    }

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: Some(row.case_id.clone()),
            action: "PARSE".to_string(),
            module: "file".to_string(),
            target_type: "evidence_file".to_string(),
            target_id: Some(row.id.clone()),
            target_title: Some(row.original_name.clone()),
            old_value: Some(serde_json::json!({
                "parse_status": row.parse_status.clone(),
                "parse_error": row.parse_error.clone(),
            })),
            new_value: Some(serde_json::json!({
                "parse_status": "processing",
            })),
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    Ok(Json(ApiEnvelope::ok(serde_json::json!({}))))
}

fn normalize_file_id(raw: &str) -> AppResult<String> {
    let uuid = Uuid::parse_str(raw).map_err(|_| AppError::bad_request("invalid file id"))?;
    Ok(uuid.to_string())
}

async fn fetch_file_row(
    state: &AppState,
    user: &AuthUser,
    file_id: &str,
) -> AppResult<EvidenceFileRow> {
    let row: Option<EvidenceFileRow> = sqlx::query_as(
        r#"
        SELECT
            id,
            case_id,
            original_name,
            file_type,
            file_size,
            storage_path,
            parse_status,
            parse_error,
            parsed_text,
            page_count,
            duration,
            created_at
        FROM evidence_files
        WHERE id = ?1 AND status != 'deleted'
        LIMIT 1
        "#,
    )
    .bind(file_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some(row) = row else {
        return Err(AppError::not_found("file not found"));
    };

    crate::access::ensure_case_access(&state.pool, user.user_id, &row.case_id).await?;
    Ok(row)
}

fn sanitize_filename(raw: &str) -> String {
    // Very small sanitization for Content-Disposition.
    let mut s = raw.replace('\\', "_").replace('"', "_");
    if s.is_empty() {
        s = "download".to_string();
    }
    s
}

// Ensure we don't accidentally return a 200 on a handler that forgot to wrap `AppResult`.
#[allow(dead_code)]
fn _assert_files_router_state(_: Router<AppState>) {}
