use crate::api::ApiEnvelope;
use crate::context::RequestMeta;
use crate::errors::{AppError, AppResult};
use crate::oplog::{spawn_operation_log, OperationLogNew};
use crate::routes::auth::AuthUser;
use crate::state::AppState;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/:id", get(get_file).delete(delete_file))
        .route("/:id/chunks", get(list_file_chunks))
        .route("/:id/download", get(download_file))
        .route("/:id/parsed", get(download_parsed))
        .route("/:id/preview", get(preview_file))
        .route("/:id/parse", post(parse_file))
        .route("/:id/translation", get(get_file_translation))
        .route("/:id/translate/retry", post(retry_file_translation))
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
    translation_status: String,
    translation_error: Option<String>,
    source_language: Option<String>,
    target_language: Option<String>,
    chunk_count: i64,
    translated_chunk_count: i64,
    failed_chunk_count: i64,
    translation_provider: Option<String>,
    translation_model: Option<String>,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct EvidenceFileTranslationSummary {
    translation_status: String,
    translation_error: Option<String>,
    source_language: Option<String>,
    target_language: Option<String>,
    chunk_count: i64,
    translated_chunk_count: i64,
    failed_chunk_count: i64,
    translation_provider: Option<String>,
    translation_model: Option<String>,
    translation_incomplete: bool,
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
    #[serde(flatten)]
    translation: EvidenceFileTranslationSummary,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct EvidenceFileTranslationDetail {
    id: String,
    #[serde(flatten)]
    translation: EvidenceFileTranslationSummary,
}

#[derive(Debug, Deserialize)]
struct ChunkListQuery {
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_page_size")]
    page_size: i64,
    #[serde(default = "default_view_mode")]
    view_mode: String,
}

#[derive(Debug, Copy, Clone)]
enum ChunkViewMode {
    Bilingual,
    Source,
    Zh,
}

#[derive(Debug, Serialize)]
struct EvidenceFileChunkListData {
    items: Vec<EvidenceFileChunkItem>,
    total: i64,
    page: i64,
    page_size: i64,
    view_mode: String,
}

#[derive(Debug, Serialize)]
struct EvidenceFileChunkItem {
    id: String,
    chunk_index: i64,
    page_number: i64,
    segment_number: i64,
    chunk_kind: String,
    display_label: String,
    source_text: Option<String>,
    translated_text: Option<String>,
    translation_status: String,
    translation_error: Option<String>,
    anchor_json: serde_json::Value,
}

#[derive(Debug, sqlx::FromRow)]
struct EvidenceFileChunkRow {
    id: String,
    chunk_index: i64,
    page_number: i64,
    segment_number: i64,
    chunk_kind: String,
    display_label: String,
    source_text: String,
    translated_text: Option<String>,
    translation_status: String,
    translation_error: Option<String>,
    anchor_json: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RetryTranslationRequest {
    scope: String,
    #[serde(default)]
    chunk_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct RetryTranslationResult {
    file_id: String,
    scope: String,
    retried_count: i64,
    skipped_count: i64,
    retried_chunk_ids: Vec<String>,
    skipped_chunk_ids: Vec<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct RetryChunkCandidateRow {
    id: String,
}

async fn get_file(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<EvidenceFileDetail>>> {
    let id = normalize_file_id(&id)?;
    let row = fetch_file_row(&state, &user, &id, false).await?;

    Ok(Json(ApiEnvelope::ok(evidence_file_detail_from_row(row))))
}

async fn get_file_translation(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<EvidenceFileTranslationDetail>>> {
    let id = normalize_file_id(&id)?;
    let row = fetch_file_row(&state, &user, &id, false).await?;

    Ok(Json(ApiEnvelope::ok(EvidenceFileTranslationDetail {
        id: row.id.clone(),
        translation: translation_summary_from_row(&row),
    })))
}

async fn list_file_chunks(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Query(query): Query<ChunkListQuery>,
) -> AppResult<Json<ApiEnvelope<EvidenceFileChunkListData>>> {
    let id = normalize_file_id(&id)?;
    let row = fetch_file_row(&state, &user, &id, false).await?;

    if row.parse_status != "done" {
        return Err(AppError::conflict("file parsing is not complete"));
    }

    let page = query.page.max(1);
    let page_size = query.page_size.clamp(1, 200);
    let offset = (page - 1) * page_size;
    let view_mode = parse_chunk_view_mode(&query.view_mode)?;

    let rows: Vec<EvidenceFileChunkRow> = sqlx::query_as(
        r#"
        SELECT
            id,
            chunk_index,
            page_number,
            segment_number,
            chunk_kind,
            display_label,
            source_text,
            translated_text,
            translation_status,
            translation_error,
            anchor_json
        FROM evidence_file_chunks
        WHERE evidence_id = ?1
        ORDER BY chunk_index ASC
        LIMIT ?2 OFFSET ?3
        "#,
    )
    .bind(&row.id)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let items = rows
        .into_iter()
        .map(|chunk| evidence_file_chunk_item_from_row(chunk, view_mode))
        .collect::<AppResult<Vec<_>>>()?;

    Ok(Json(ApiEnvelope::ok(EvidenceFileChunkListData {
        items,
        total: row.chunk_count,
        page,
        page_size,
        view_mode: view_mode.as_str().to_string(),
    })))
}

async fn delete_file(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<serde_json::Value>>> {
    let id = normalize_file_id(&id)?;
    let row = fetch_file_row(&state, &user, &id, true).await?;

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
    let row = fetch_file_row(&state, &user, &id, false).await?;

    let full_path = PathBuf::from(&state.config.storage_path).join(&row.storage_path);
    let file = tokio::fs::File::open(&full_path)
        .await
        .map_err(|_| AppError::not_found_code(420101, "file not found"))?;

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

async fn download_parsed(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let id = normalize_file_id(&id)?;
    let row = fetch_file_row(&state, &user, &id, false).await?;

    let parsed_rel = format!("parsed/{}/{}.json", row.case_id, row.id);
    let full_path = PathBuf::from(&state.config.storage_path).join(&parsed_rel);
    let file = tokio::fs::File::open(&full_path)
        .await
        .map_err(|_| AppError::not_found_code(420101, "parsed result not found"))?;
    let parsed_size = tokio::fs::metadata(&full_path).await.ok().map(|m| m.len());

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: Some(row.case_id.clone()),
            action: "DOWNLOAD".to_string(),
            module: "file".to_string(),
            target_type: "evidence_file_parsed".to_string(),
            target_id: Some(row.id.clone()),
            target_title: Some(format!("{} (parsed)", row.original_name)),
            old_value: None,
            new_value: Some(serde_json::json!({
                "id": row.id.clone(),
                "case_id": row.case_id.clone(),
                "original_name": row.original_name.clone(),
                "parsed_path": parsed_rel,
                "parsed_size": parsed_size,
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

    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );

    let filename = parsed_json_filename(&row.original_name);
    let cd = format!("attachment; filename=\"{}\"", filename);
    if let Ok(v) = cd.parse::<HeaderValue>() {
        resp.headers_mut().insert(header::CONTENT_DISPOSITION, v);
    }

    Ok(resp)
}

async fn retry_file_translation(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(id): Path<String>,
    Json(payload): Json<RetryTranslationRequest>,
) -> AppResult<Response> {
    let id = normalize_file_id(&id)?;
    let row = fetch_file_row(&state, &user, &id, true).await?;

    if row.parse_status != "done" {
        return Err(AppError::conflict("file parsing is not complete"));
    }

    let scope = payload.scope.trim().to_ascii_lowercase();
    let selected_ids = match scope.as_str() {
        "all" => None,
        "selected" => {
            let raw_ids = payload.chunk_ids.unwrap_or_default();
            if raw_ids.is_empty() {
                return Ok(error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    422000,
                    "chunk_ids required when scope=selected",
                ));
            }

            let ids = raw_ids
                .into_iter()
                .map(|chunk_id| normalize_chunk_id(&chunk_id))
                .collect::<AppResult<HashSet<_>>>()?;
            Some(ids)
        }
        _ => return Err(AppError::bad_request("invalid retry scope")),
    };

    let chunk_rows: Vec<RetryChunkCandidateRow> = sqlx::query_as(
        r#"
        SELECT id
        FROM evidence_file_chunks
        WHERE evidence_id = ?1
        ORDER BY chunk_index ASC
        "#,
    )
    .bind(&row.id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let candidate_chunk_ids = chunk_rows
        .into_iter()
        .filter_map(|chunk| {
            if let Some(selected_ids) = selected_ids.as_ref() {
                if !selected_ids.contains(&chunk.id) {
                    return None;
                }
            }
            Some(chunk.id)
        })
        .collect::<Vec<_>>();

    let mut retried_chunk_ids = Vec::new();
    let mut skipped_chunk_ids = Vec::new();

    if !candidate_chunk_ids.is_empty() {
        let mut tx = state
            .pool
            .begin()
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        for chunk_id in candidate_chunk_ids {
            let updated = sqlx::query(
                r#"
                UPDATE evidence_file_chunks
                SET
                    translation_status = 'pending',
                    retry_count = 0,
                    last_attempt_at = NULL,
                    next_retry_at = CURRENT_TIMESTAMP,
                    translation_error = NULL,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = ?1
                  AND evidence_id = ?2
                  AND translation_status IN ('pending', 'failed')
                "#,
            )
            .bind(&chunk_id)
            .bind(&row.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

            if updated.rows_affected() == 1 {
                retried_chunk_ids.push(chunk_id);
            } else {
                skipped_chunk_ids.push(chunk_id);
            }
        }

        tx.commit()
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;
    }

    refresh_file_translation_snapshot(&state, &row.id).await?;

    if !retried_chunk_ids.is_empty() {
        crate::translation::start_file_translation(state.clone(), row.id.clone())
            .await
            .map_err(|e| AppError::internal(format!("start translation failed: {e}")))?;
    }

    let result = RetryTranslationResult {
        file_id: row.id.clone(),
        scope: scope.clone(),
        retried_count: retried_chunk_ids.len() as i64,
        skipped_count: skipped_chunk_ids.len() as i64,
        retried_chunk_ids: retried_chunk_ids.clone(),
        skipped_chunk_ids: skipped_chunk_ids.clone(),
    };

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: Some(row.case_id.clone()),
            action: "RETRY_TRANSLATION".to_string(),
            module: "file".to_string(),
            target_type: "evidence_file_translation".to_string(),
            target_id: Some(row.id.clone()),
            target_title: Some(row.original_name.clone()),
            old_value: Some(serde_json::json!({
                "translation_status": row.translation_status,
                "translation_error": row.translation_error,
            })),
            new_value: Some(serde_json::json!({
                "scope": scope,
                "retried_count": result.retried_count,
                "skipped_count": result.skipped_count,
                "retried_chunk_ids": result.retried_chunk_ids,
                "skipped_chunk_ids": result.skipped_chunk_ids,
            })),
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    Ok(Json(ApiEnvelope::ok(result)).into_response())
}

async fn preview_file(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let id = normalize_file_id(&id)?;
    let row = fetch_file_row(&state, &user, &id, false).await?;

    let full_path = PathBuf::from(&state.config.storage_path).join(&row.storage_path);
    let file = tokio::fs::File::open(&full_path)
        .await
        .map_err(|_| AppError::not_found_code(420101, "file not found"))?;

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
    let row = fetch_file_row(&state, &user, &id, true).await?;

    if row.parse_status == "processing" {
        return Err(AppError::conflict("file is already processing"));
    }

    let started = crate::parser::enqueue_parse(state.clone(), row.id.clone(), true)
    .await
    .map_err(|e| AppError::internal(format!("enqueue parse failed: {e}")))?;

    if !started {
        return Err(AppError::not_found_code(420101, "file not found"));
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

fn normalize_chunk_id(raw: &str) -> AppResult<String> {
    let uuid = Uuid::parse_str(raw).map_err(|_| AppError::bad_request("invalid chunk id"))?;
    Ok(uuid.to_string())
}

async fn fetch_file_row(
    state: &AppState,
    user: &AuthUser,
    file_id: &str,
    require_write: bool,
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
            translation_status,
            translation_error,
            source_language,
            target_language,
            chunk_count,
            translated_chunk_count,
            failed_chunk_count,
            translation_provider,
            translation_model,
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
        return Err(AppError::not_found_code(420101, "file not found"));
    };

    if require_write {
        crate::access::ensure_case_write_access(&state.pool, user.user_id, &row.case_id).await?;
    } else {
        crate::access::ensure_case_access(&state.pool, user.user_id, &row.case_id).await?;
    }
    Ok(row)
}

async fn refresh_file_translation_snapshot(state: &AppState, file_id: &str) -> AppResult<()> {
    #[derive(Debug, sqlx::FromRow)]
    struct AggregateRow {
        total_count: i64,
        pending_count: i64,
        processing_count: i64,
        done_count: i64,
        failed_count: i64,
        latest_error: Option<String>,
    }

    let aggregate: AggregateRow = sqlx::query_as(
        r#"
        SELECT
            COUNT(1) AS total_count,
            COALESCE(SUM(CASE WHEN translation_status = 'pending' THEN 1 ELSE 0 END), 0) AS pending_count,
            COALESCE(SUM(CASE WHEN translation_status = 'processing' THEN 1 ELSE 0 END), 0) AS processing_count,
            COALESCE(SUM(CASE WHEN translation_status = 'done' THEN 1 ELSE 0 END), 0) AS done_count,
            COALESCE(SUM(CASE WHEN translation_status = 'failed' THEN 1 ELSE 0 END), 0) AS failed_count,
            (
                SELECT translation_error
                FROM evidence_file_chunks
                WHERE evidence_id = ?1
                  AND translation_status = 'failed'
                  AND translation_error IS NOT NULL
                ORDER BY updated_at DESC, chunk_index DESC
                LIMIT 1
            ) AS latest_error
        FROM evidence_file_chunks
        WHERE evidence_id = ?1
        "#,
    )
    .bind(file_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let status = if aggregate.total_count == 0 || aggregate.done_count == aggregate.total_count {
        "done"
    } else if aggregate.pending_count > 0 || aggregate.processing_count > 0 {
        "processing"
    } else if aggregate.failed_count > 0 && aggregate.done_count > 0 {
        "partial"
    } else if aggregate.failed_count > 0 {
        "failed"
    } else {
        "processing"
    };

    let translation_error = match status {
        "done" | "processing" => None,
        _ => aggregate.latest_error,
    };

    sqlx::query(
        r#"
        UPDATE evidence_files
        SET
            translation_status = ?1,
            translation_error = ?2,
            translated_chunk_count = ?3,
            failed_chunk_count = ?4
        WHERE id = ?5 AND status != 'deleted'
        "#,
    )
    .bind(status)
    .bind(translation_error)
    .bind(aggregate.done_count)
    .bind(aggregate.failed_count)
    .bind(file_id)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    Ok(())
}

fn evidence_file_detail_from_row(row: EvidenceFileRow) -> EvidenceFileDetail {
    let translation = translation_summary_from_row(&row);
    EvidenceFileDetail {
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
        translation,
        created_at: row.created_at,
    }
}

fn translation_summary_from_row(row: &EvidenceFileRow) -> EvidenceFileTranslationSummary {
    EvidenceFileTranslationSummary {
        translation_status: row.translation_status.clone(),
        translation_error: row.translation_error.clone(),
        source_language: row.source_language.clone(),
        target_language: row.target_language.clone(),
        chunk_count: row.chunk_count,
        translated_chunk_count: row.translated_chunk_count,
        failed_chunk_count: row.failed_chunk_count,
        translation_provider: row.translation_provider.clone(),
        translation_model: row.translation_model.clone(),
        translation_incomplete: row.translation_status != "done"
            || (row.translated_chunk_count + row.failed_chunk_count) < row.chunk_count,
    }
}

fn default_page() -> i64 {
    1
}

fn default_page_size() -> i64 {
    20
}

fn default_view_mode() -> String {
    "bilingual".to_string()
}

fn parse_chunk_view_mode(raw: &str) -> AppResult<ChunkViewMode> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "bilingual" => Ok(ChunkViewMode::Bilingual),
        "source" => Ok(ChunkViewMode::Source),
        "zh" => Ok(ChunkViewMode::Zh),
        _ => Err(AppError::bad_request("invalid view_mode")),
    }
}

impl ChunkViewMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Bilingual => "bilingual",
            Self::Source => "source",
            Self::Zh => "zh",
        }
    }
}

fn evidence_file_chunk_item_from_row(
    row: EvidenceFileChunkRow,
    view_mode: ChunkViewMode,
) -> AppResult<EvidenceFileChunkItem> {
    let anchor_json = match row.anchor_json {
        Some(value) => serde_json::from_str(&value)
            .map_err(|e| AppError::internal(format!("invalid chunk anchor_json: {e}")))?,
        None => serde_json::Value::Null,
    };

    let (source_text, translated_text) = match view_mode {
        ChunkViewMode::Bilingual => (Some(row.source_text), row.translated_text),
        ChunkViewMode::Source => (Some(row.source_text), None),
        ChunkViewMode::Zh => (None, row.translated_text),
    };

    Ok(EvidenceFileChunkItem {
        id: row.id,
        chunk_index: row.chunk_index,
        page_number: row.page_number,
        segment_number: row.segment_number,
        chunk_kind: row.chunk_kind,
        display_label: row.display_label,
        source_text,
        translated_text,
        translation_status: row.translation_status,
        translation_error: row.translation_error,
        anchor_json,
    })
}

fn error_response(status: StatusCode, error_code: i32, message: impl Into<String>) -> Response {
    let body = ApiEnvelope::err(status.as_u16(), error_code, message);
    (status, Json(body)).into_response()
}

fn sanitize_filename(raw: &str) -> String {
    // Very small sanitization for Content-Disposition.
    let mut s = raw.replace('\\', "_").replace('"', "_");
    if s.is_empty() {
        s = "download".to_string();
    }
    s
}

fn parsed_json_filename(original_name: &str) -> String {
    let mut base = sanitize_filename(original_name);
    if let Some(idx) = base.rfind('.') {
        base.truncate(idx);
    }
    let base = base.trim();
    if base.is_empty() {
        return "parsed.json".to_string();
    }
    format!("{base}.json")
}

// Ensure we don't accidentally return a 200 on a handler that forgot to wrap `AppResult`.
#[allow(dead_code)]
fn _assert_files_router_state(_: Router<AppState>) {}
