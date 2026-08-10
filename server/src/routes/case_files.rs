use crate::api::ApiEnvelope;
use crate::context::RequestMeta;
use crate::errors::{AppError, AppResult};
use crate::oplog::{spawn_operation_log, OperationLogNew};
use crate::routes::auth::AuthUser;
use crate::state::AppState;
use axum::{
    extract::DefaultBodyLimit,
    extract::{Multipart, Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::{Path as FsPath, PathBuf};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

pub fn router(max_upload_bytes: usize) -> Router<AppState> {
    Router::new()
        .route(
            "/:case_id/files",
            get(list_case_files).post(upload_case_file),
        )
        // Axum's default body limit is 2MB. File uploads are expected to be much larger.
        .layer(DefaultBodyLimit::max(max_upload_bytes))
}

#[derive(Debug, Deserialize)]
struct Pagination {
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_page_size")]
    page_size: i64,
}

fn default_page() -> i64 {
    1
}

fn default_page_size() -> i64 {
    20
}

#[derive(Debug, Serialize)]
struct EvidenceFileSummary {
    id: String,
    original_name: String,
    file_type: String,
    file_size: i64,
    storage_path: String,
    parse_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_error: Option<String>,
    #[serde(flatten)]
    translation: EvidenceFileTranslationSummary,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct EvidenceFileListData {
    files: Vec<EvidenceFileSummary>,
    total: i64,
    page: i64,
    page_size: i64,
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

async fn list_case_files(
    State(state): State<AppState>,
    user: AuthUser,
    Path(case_id): Path<String>,
    Query(p): Query<Pagination>,
) -> AppResult<Json<ApiEnvelope<EvidenceFileListData>>> {
    let case_id = normalize_case_id(&case_id)?;
    crate::access::ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    let page = p.page.max(1);
    let page_size = p.page_size.clamp(1, 200);
    let offset = (page - 1) * page_size;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(1) FROM evidence_files WHERE case_id = $1 AND status != 'deleted'",
    )
    .bind(&case_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let rows: Vec<EvidenceFileRow> = sqlx::query_as(
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
        WHERE case_id = $1 AND status != 'deleted'
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(&case_id)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let files = rows
        .into_iter()
        .map(evidence_file_summary_from_row)
        .collect();

    Ok(Json(ApiEnvelope::ok(EvidenceFileListData {
        files,
        total: total.0,
        page,
        page_size,
    })))
}

async fn upload_case_file(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(case_id): Path<String>,
    mut multipart: Multipart,
) -> AppResult<Json<ApiEnvelope<EvidenceFileDetail>>> {
    let case_id = normalize_case_id(&case_id)?;
    crate::access::ensure_case_write_access(&state.pool, user.user_id, &case_id).await?;

    let mut field = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("multipart error: {e}")))?
        .ok_or_else(|| AppError::bad_request("no file field"))?;

    let original_name = field
        .file_name()
        .map(str::to_string)
        .ok_or_else(|| AppError::bad_request("missing filename"))?;

    let stored_name = generate_stored_name(&original_name);
    let storage_path = format!("files/{}/{}", case_id, stored_name);
    let full_path = PathBuf::from(&state.config.storage_path).join(&storage_path);

    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::internal(format!("create dir failed: {e}")))?;
    }

    // Stream upload to disk to avoid holding large files in memory.
    // We write to a temp file first, validate the header, then atomically rename.
    const SNIFF_LIMIT: usize = 16 * 1024;
    const VALIDATE_AT: usize = 512;

    let uploading_path = full_path.with_file_name(format!("{stored_name}.uploading"));
    let mut f = tokio::fs::File::create(&uploading_path)
        .await
        .map_err(|e| AppError::internal(format!("write file failed: {e}")))?;

    let mut total: usize = 0;
    let mut sniff: Vec<u8> = Vec::new();
    let mut file_type: Option<String> = None;

    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|e| AppError::bad_request(format!("read upload failed: {e}")))?
    {
        total = total.saturating_add(chunk.len());
        if total as u64 > state.config.max_file_size {
            drop(f);
            let _ = tokio::fs::remove_file(&uploading_path).await;
            return Err(AppError::bad_request_code(420102, "file too large"));
        }

        if sniff.len() < SNIFF_LIMIT {
            let remain = SNIFF_LIMIT - sniff.len();
            let take = remain.min(chunk.len());
            sniff.extend_from_slice(&chunk.as_ref()[..take]);
        }

        if file_type.is_none() && sniff.len() >= VALIDATE_AT {
            match crate::file_security::validate_upload(
                &sniff,
                &original_name,
                &state.config.allowed_file_types,
            ) {
                Ok(t) => file_type = Some(t),
                Err(e) => {
                    drop(f);
                    let _ = tokio::fs::remove_file(&uploading_path).await;
                    return Err(e);
                }
            }
        }

        f.write_all(chunk.as_ref())
            .await
            .map_err(|e| AppError::internal(format!("write file failed: {e}")))?;
    }

    if total == 0 {
        drop(f);
        let _ = tokio::fs::remove_file(&uploading_path).await;
        return Err(AppError::bad_request("empty file"));
    }

    let file_type = match file_type {
        Some(t) => t,
        None => match crate::file_security::validate_upload(
            &sniff,
            &original_name,
            &state.config.allowed_file_types,
        ) {
            Ok(t) => t,
            Err(e) => {
                drop(f);
                let _ = tokio::fs::remove_file(&uploading_path).await;
                return Err(e);
            }
        },
    };

    f.flush()
        .await
        .map_err(|e| AppError::internal(format!("write file failed: {e}")))?;
    drop(f);

    tokio::fs::rename(&uploading_path, &full_path)
        .await
        .map_err(|e| AppError::internal(format!("write file failed: {e}")))?;

    let file_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO evidence_files (
            id, case_id, original_name, stored_name, file_type, file_size, storage_path,
            status, uploaded_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'active', $8)
        "#,
    )
    .bind(file_id.to_string())
    .bind(&case_id)
    .bind(&original_name)
    .bind(&stored_name)
    .bind(&file_type)
    .bind(total as i64)
    .bind(&storage_path)
    .bind(user.user_id.to_string())
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    crate::parser::enqueue_parse(state.clone(), file_id.to_string(), true)
        .await
        .map_err(|e| AppError::internal(format!("enqueue parse failed: {e}")))?;

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
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(file_id.to_string())
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some(row) = row else {
        return Err(AppError::internal("file inserted but not found"));
    };

    let detail = evidence_file_detail_from_row(row);

    // Audit log (avoid storing parsed_text).
    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: Some(detail.case_id.clone()),
            action: "UPLOAD".to_string(),
            module: "file".to_string(),
            target_type: "evidence_file".to_string(),
            target_id: Some(detail.id.clone()),
            target_title: Some(detail.original_name.clone()),
            old_value: None,
            new_value: Some(serde_json::json!({
                "id": detail.id.clone(),
                "case_id": detail.case_id.clone(),
                "original_name": detail.original_name.clone(),
                "file_type": detail.file_type.clone(),
                "file_size": detail.file_size,
                "storage_path": detail.storage_path.clone(),
                "parse_status": detail.parse_status.clone(),
                "parse_error": detail.parse_error.clone(),
                "page_count": detail.page_count,
                "duration": detail.duration,
                "created_at": detail.created_at.clone(),
            })),
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    Ok(Json(ApiEnvelope::ok(detail)))
}

fn normalize_case_id(raw: &str) -> AppResult<String> {
    let uuid = Uuid::parse_str(raw).map_err(|_| AppError::bad_request("invalid case_id"))?;
    Ok(uuid.to_string())
}

fn generate_stored_name(original: &str) -> String {
    let uuid = Uuid::new_v4();
    let ext = FsPath::new(original)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    let ext = ext.trim().to_ascii_lowercase();
    let ext = if ext.is_empty() || !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        None
    } else {
        Some(ext)
    };

    match ext {
        Some(ext) => format!("{}.{}", uuid, ext),
        None => uuid.to_string(),
    }
}

fn evidence_file_summary_from_row(row: EvidenceFileRow) -> EvidenceFileSummary {
    let translation = translation_summary_from_row(&row);
    EvidenceFileSummary {
        id: row.id,
        original_name: row.original_name,
        file_type: row.file_type,
        file_size: row.file_size,
        storage_path: row.storage_path,
        parse_status: row.parse_status,
        parse_error: row.parse_error,
        translation,
        created_at: row.created_at,
    }
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

// Ensure we don't accidentally return a 200 on a handler that forgot to wrap `AppResult`.
#[allow(dead_code)]
fn _assert_case_files_router_state(_: Router<AppState>) {}
