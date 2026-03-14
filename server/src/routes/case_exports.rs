use crate::access::ensure_case_access;
use crate::api::ApiEnvelope;
use crate::context::RequestMeta;
use crate::errors::{AppError, AppResult};
use crate::oplog::{spawn_operation_log, OperationLogNew};
use crate::routes::auth::AuthUser;
use crate::state::AppState;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderValue},
    response::Response,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/:case_id/exports/evidence-list", get(export_evidence_list))
        .route("/:case_id/exports/timeline", get(export_timeline))
        .route("/:case_id/exports/history", get(export_history))
        .route(
            "/:case_id/exports/:export_id/download",
            get(download_export),
        )
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

#[derive(Debug, Deserialize)]
struct TimelineExportQuery {
    #[serde(default)]
    format: Option<String>,
}

#[derive(Debug, Serialize)]
struct ExportRecordItem {
    id: String,
    case_id: String,
    export_type: String,
    file_name: String,
    storage_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_size: Option<i64>,
    generated_by: String,
    generated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    node_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct ExportHistoryData {
    records: Vec<ExportRecordItem>,
    total: i64,
    page: i64,
    page_size: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct ExportRecordRow {
    id: String,
    case_id: String,
    export_type: String,
    file_name: String,
    storage_path: String,
    file_size: Option<i64>,
    generated_by: String,
    generated_at: String,
    node_ids: Option<String>,
    evidence_ids: Option<String>,
}

async fn export_history(
    State(state): State<AppState>,
    user: AuthUser,
    Path(case_id): Path<String>,
    Query(p): Query<Pagination>,
) -> AppResult<Json<ApiEnvelope<ExportHistoryData>>> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    let page = p.page.max(1);
    let page_size = p.page_size.clamp(1, 200);
    let offset = (page - 1) * page_size;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(1) FROM export_records WHERE case_id = ?1")
        .bind(&case_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let rows: Vec<ExportRecordRow> = sqlx::query_as(
        r#"
        SELECT
          id,
          case_id,
          export_type,
          file_name,
          storage_path,
          file_size,
          generated_by,
          generated_at,
          node_ids,
          evidence_ids
        FROM export_records
        WHERE case_id = ?1
        ORDER BY generated_at DESC
        LIMIT ?2 OFFSET ?3
        "#,
    )
    .bind(&case_id)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let records = rows
        .into_iter()
        .map(|r| ExportRecordItem {
            id: r.id,
            case_id: r.case_id,
            export_type: r.export_type,
            file_name: r.file_name,
            storage_path: r.storage_path,
            file_size: r.file_size,
            generated_by: r.generated_by,
            generated_at: r.generated_at,
            node_ids: r
                .node_ids
                .as_deref()
                .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok()),
            evidence_ids: r
                .evidence_ids
                .as_deref()
                .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok()),
        })
        .collect::<Vec<_>>();

    Ok(Json(ApiEnvelope::ok(ExportHistoryData {
        records,
        total: total.0,
        page,
        page_size,
    })))
}

#[derive(Debug, sqlx::FromRow)]
struct EvidenceExportRow {
    id: String,
    original_name: String,
    file_type: String,
    file_size: i64,
    parse_status: String,
    page_count: Option<i64>,
    duration: Option<i64>,
    created_at: String,
    related_nodes: Option<String>,
}

async fn export_evidence_list(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(case_id): Path<String>,
) -> AppResult<Response> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    let rows: Vec<EvidenceExportRow> = sqlx::query_as(
        r#"
        SELECT
          e.id,
          e.original_name,
          e.file_type,
          e.file_size,
          e.parse_status,
          e.page_count,
          e.duration,
          e.created_at,
          group_concat(n.title, ' | ') AS related_nodes
        FROM evidence_files e
        LEFT JOIN node_evidence_links l ON l.evidence_id = e.id
        LEFT JOIN event_nodes n ON n.id = l.node_id AND n.status != 'deleted'
        WHERE e.case_id = ?1 AND e.status != 'deleted'
        GROUP BY e.id
        ORDER BY e.created_at DESC
        "#,
    )
    .bind(&case_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let evidence_ids = rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
    let node_ids: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT l.node_id
        FROM node_evidence_links l
        JOIN event_nodes n ON n.id = l.node_id
        WHERE n.case_id = ?1 AND n.status != 'deleted'
        "#,
    )
    .bind(&case_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let mut csv = String::new();
    csv.push_str("evidence_id,original_name,file_type,file_size,parse_status,page_count,duration,created_at,related_nodes\n");
    for r in &rows {
        csv.push_str(&csv_row(&[
            &r.id,
            &r.original_name,
            &r.file_type,
            &r.file_size.to_string(),
            &r.parse_status,
            &r.page_count.map(|v| v.to_string()).unwrap_or_default(),
            &r.duration.map(|v| v.to_string()).unwrap_or_default(),
            &r.created_at,
            r.related_nodes.as_deref().unwrap_or(""),
        ]));
        csv.push('\n');
    }

    let bytes = csv.into_bytes();
    let file_name = format!(
        "evidence_list_{}_{}.csv",
        case_id,
        chrono::Utc::now().format("%Y%m%d_%H%M%S")
    );
    let storage_path = format!("exports/{}/{}", case_id, file_name);
    let full_path = PathBuf::from(&state.config.storage_path).join(&storage_path);

    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::internal(format!("create dir failed: {e}")))?;
    }

    tokio::fs::write(&full_path, &bytes)
        .await
        .map_err(|e| AppError::internal(format!("write export failed: {e}")))?;

    let record_id = Uuid::new_v4().to_string();
    let node_ids_json =
        serde_json::to_string(&node_ids).map_err(|e| AppError::internal(format!("{e}")))?;
    let evidence_ids_json =
        serde_json::to_string(&evidence_ids).map_err(|e| AppError::internal(format!("{e}")))?;

    sqlx::query(
        r#"
        INSERT INTO export_records (
          id, case_id, export_type, file_name, storage_path, file_size, generated_by, node_ids, evidence_ids
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(&record_id)
    .bind(&case_id)
    .bind("evidence_list")
    .bind(&file_name)
    .bind(&storage_path)
    .bind(bytes.len() as i64)
    .bind(user.user_id.to_string())
    .bind(node_ids_json)
    .bind(evidence_ids_json)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: Some(case_id.clone()),
            action: "EXPORT".to_string(),
            module: "export".to_string(),
            target_type: "export_record".to_string(),
            target_id: Some(record_id),
            target_title: Some(file_name.clone()),
            old_value: None,
            new_value: Some(serde_json::json!({
                "export_type": "evidence_list",
                "file_name": file_name,
                "storage_path": storage_path,
                "file_size": bytes.len(),
                "evidence_count": rows.len(),
            })),
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    let file = tokio::fs::File::open(&full_path)
        .await
        .map_err(|_| AppError::not_found("export not found"))?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    let mut resp = Response::new(body);

    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );

    let cd = format!("attachment; filename=\"{}\"", sanitize_filename(&file_name));
    if let Ok(v) = cd.parse::<HeaderValue>() {
        resp.headers_mut().insert(header::CONTENT_DISPOSITION, v);
    }

    Ok(resp)
}

async fn download_export(
    State(state): State<AppState>,
    user: AuthUser,
    Path((case_id, export_id)): Path<(String, String)>,
) -> AppResult<Response> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    let export_id = normalize_uuid(&export_id, "invalid export id")?;

    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT file_name, storage_path FROM export_records WHERE id = ?1 AND case_id = ?2 LIMIT 1",
    )
    .bind(&export_id)
    .bind(&case_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some((file_name, storage_path)) = row else {
        return Err(AppError::not_found("export not found"));
    };

    let full_path = PathBuf::from(&state.config.storage_path).join(&storage_path);
    let file = tokio::fs::File::open(&full_path)
        .await
        .map_err(|_| AppError::not_found("export not found"))?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    let mut resp = Response::new(body);

    let content_type = if file_name.to_ascii_lowercase().ends_with(".csv") {
        HeaderValue::from_static("text/csv; charset=utf-8")
    } else {
        HeaderValue::from_static("application/octet-stream")
    };
    resp.headers_mut()
        .insert(header::CONTENT_TYPE, content_type);

    let cd = format!("attachment; filename=\"{}\"", sanitize_filename(&file_name));
    if let Ok(v) = cd.parse::<HeaderValue>() {
        resp.headers_mut().insert(header::CONTENT_DISPOSITION, v);
    }

    Ok(resp)
}

async fn export_timeline(
    State(state): State<AppState>,
    user: AuthUser,
    Path(case_id): Path<String>,
    Query(q): Query<TimelineExportQuery>,
) -> AppResult<Json<ApiEnvelope<serde_json::Value>>> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    let fmt = q
        .format
        .as_deref()
        .unwrap_or("png")
        .trim()
        .to_ascii_lowercase();

    // Not implemented in current stage; reserved for future (PNG rendering/export).
    Err(AppError::bad_request(format!(
        "timeline export not implemented (requested format: {fmt})"
    )))
}

fn csv_row(fields: &[&str]) -> String {
    fields
        .iter()
        .map(|f| csv_escape(f))
        .collect::<Vec<_>>()
        .join(",")
}

fn csv_escape(raw: &str) -> String {
    let needs_quotes =
        raw.contains(',') || raw.contains('"') || raw.contains('\n') || raw.contains('\r');
    if !needs_quotes {
        return raw.to_string();
    }
    let escaped = raw.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

fn sanitize_filename(raw: &str) -> String {
    let mut s = raw.replace('\\', "_").replace('"', "_");
    if s.is_empty() {
        s = "download".to_string();
    }
    s
}

fn normalize_uuid(raw: &str, message: &str) -> AppResult<String> {
    let uuid = Uuid::parse_str(raw).map_err(|_| AppError::bad_request(message))?;
    Ok(uuid.to_string())
}

// Ensure we don't accidentally return a 200 on a handler that forgot to wrap `AppResult`.
#[allow(dead_code)]
fn _assert_case_exports_router_state(_: Router<AppState>) {}
