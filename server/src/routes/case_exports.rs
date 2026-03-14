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
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::NaiveDate;
use printpdf::{Base64OrRaw, GeneratePdfOptions, PdfDocument, PdfSaveOptions, PdfWarnMsg};
use resvg::{tiny_skia, usvg};
use rust_xlsxwriter::Workbook;
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Sqlite};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/:case_id/exports/evidence-list", get(export_evidence_list))
        .route("/:case_id/exports/timeline", get(export_timeline))
        .route(
            "/:case_id/exports/timeline-report",
            get(export_timeline_report),
        )
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
    #[serde(default)]
    start_date: Option<String>,
    #[serde(default)]
    end_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TimelineReportQuery {
    #[serde(default)]
    format: Option<String>, // pdf/html
    #[serde(default)]
    start_date: Option<String>,
    #[serde(default)]
    end_date: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    template_name: Option<String>,
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
    template_name: Option<String>,
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
          evidence_ids,
          template_name
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
            template_name: r.template_name,
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

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    let _ = worksheet.set_name("Evidence List");

    let headers = [
        "evidence_id",
        "original_name",
        "file_type",
        "file_size",
        "parse_status",
        "page_count",
        "duration",
        "created_at",
        "related_nodes",
    ];

    for (col, h) in headers.iter().enumerate() {
        worksheet
            .write_string(0, col as u16, *h)
            .map_err(|e| AppError::internal(format!("xlsx write failed: {e}")))?;
    }

    for (idx, r) in rows.iter().enumerate() {
        let row = (idx + 1) as u32;
        worksheet
            .write_string(row, 0, &r.id)
            .map_err(|e| AppError::internal(format!("xlsx write failed: {e}")))?;
        worksheet
            .write_string(row, 1, &r.original_name)
            .map_err(|e| AppError::internal(format!("xlsx write failed: {e}")))?;
        worksheet
            .write_string(row, 2, &r.file_type)
            .map_err(|e| AppError::internal(format!("xlsx write failed: {e}")))?;
        worksheet
            .write_string(row, 3, &r.file_size.to_string())
            .map_err(|e| AppError::internal(format!("xlsx write failed: {e}")))?;
        worksheet
            .write_string(row, 4, &r.parse_status)
            .map_err(|e| AppError::internal(format!("xlsx write failed: {e}")))?;
        worksheet
            .write_string(
                row,
                5,
                &r.page_count.map(|v| v.to_string()).unwrap_or_default(),
            )
            .map_err(|e| AppError::internal(format!("xlsx write failed: {e}")))?;
        worksheet
            .write_string(
                row,
                6,
                &r.duration.map(|v| v.to_string()).unwrap_or_default(),
            )
            .map_err(|e| AppError::internal(format!("xlsx write failed: {e}")))?;
        worksheet
            .write_string(row, 7, &r.created_at)
            .map_err(|e| AppError::internal(format!("xlsx write failed: {e}")))?;
        worksheet
            .write_string(row, 8, r.related_nodes.as_deref().unwrap_or(""))
            .map_err(|e| AppError::internal(format!("xlsx write failed: {e}")))?;
    }

    let bytes = workbook
        .save_to_buffer()
        .map_err(|e| AppError::internal(format!("xlsx save failed: {e}")))?;
    let file_name = format!(
        "{}_evidence_list_{}.xlsx",
        &case_id,
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
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
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
    meta: RequestMeta,
    Path((case_id, export_id)): Path<(String, String)>,
) -> AppResult<Response> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    let export_id = normalize_uuid(&export_id, "invalid export id")?;

    let row: Option<(String, String, String, Option<i64>)> = sqlx::query_as(
        "SELECT export_type, file_name, storage_path, file_size FROM export_records WHERE id = ?1 AND case_id = ?2 LIMIT 1",
    )
    .bind(&export_id)
    .bind(&case_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some((export_type, file_name, storage_path, file_size)) = row else {
        return Err(AppError::not_found("export not found"));
    };

    let full_path = PathBuf::from(&state.config.storage_path).join(&storage_path);
    let file = tokio::fs::File::open(&full_path)
        .await
        .map_err(|_| AppError::not_found("export not found"))?;

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: Some(case_id.clone()),
            action: "DOWNLOAD".to_string(),
            module: "export".to_string(),
            target_type: "export_record".to_string(),
            target_id: Some(export_id.clone()),
            target_title: Some(file_name.clone()),
            old_value: None,
            new_value: Some(serde_json::json!({
                "export_type": export_type,
                "file_name": file_name.clone(),
                "storage_path": storage_path.clone(),
                "file_size": file_size,
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

    let file_name_lc = file_name.to_ascii_lowercase();
    let content_type = if file_name_lc.ends_with(".csv") {
        HeaderValue::from_static("text/csv; charset=utf-8")
    } else if file_name_lc.ends_with(".html") || file_name_lc.ends_with(".htm") {
        HeaderValue::from_static("text/html; charset=utf-8")
    } else if file_name_lc.ends_with(".xlsx") {
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
    } else if file_name_lc.ends_with(".png") {
        HeaderValue::from_static("image/png")
    } else if file_name_lc.ends_with(".pdf") {
        HeaderValue::from_static("application/pdf")
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
    meta: RequestMeta,
    Path(case_id): Path<String>,
    Query(q): Query<TimelineExportQuery>,
) -> AppResult<Response> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    let fmt = q
        .format
        .as_deref()
        .unwrap_or("png")
        .trim()
        .to_ascii_lowercase();

    if fmt != "png" {
        return Err(AppError::bad_request("only format=png is supported"));
    }

    let (start, end) = parse_date_range(q.start_date.as_deref(), q.end_date.as_deref())?;

    let case_name: Option<String> = sqlx::query_scalar("SELECT name FROM cases WHERE id = ?1")
        .bind(&case_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;
    let Some(case_name) = case_name else {
        return Err(AppError::not_found("case not found"));
    };

    const MAX_NODES: usize = 200;
    let nodes: Vec<TimelineNodeExportRow> = {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
            r#"
            SELECT id, title, description, event_time
            FROM event_nodes
            WHERE case_id = 
            "#,
        );
        qb.push_bind(&case_id);
        qb.push(" AND status != 'deleted'");
        if let Some(start) = start {
            qb.push(" AND event_time >= ");
            qb.push_bind(start.to_string());
        }
        if let Some(end) = end {
            qb.push(" AND event_time <= ");
            qb.push_bind(end.to_string());
        }
        qb.push(" ORDER BY event_time ASC, sort_order ASC LIMIT ");
        // Load one extra row so we can return a nice error message.
        qb.push_bind((MAX_NODES + 1) as i64);

        qb.build_query_as::<TimelineNodeExportRow>()
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    };

    if nodes.len() > MAX_NODES {
        return Err(AppError::bad_request(
            "too many nodes to export; please narrow by start_date/end_date",
        ));
    }

    let node_ids = nodes.iter().map(|n| n.id.clone()).collect::<Vec<_>>();
    let evidence_ids = fetch_evidence_ids_for_nodes(&state, &node_ids).await?;

    const WIDTH: u32 = 1600;
    const HEADER_H: u32 = 140;
    const ROW_H: u32 = 72;
    const FOOTER_H: u32 = 80;
    const MAX_HEIGHT: u32 = 20_000;
    let rows = (nodes.len().max(1)) as u32;
    let height = HEADER_H + (ROW_H * rows) + FOOTER_H;
    if height > MAX_HEIGHT {
        return Err(AppError::bad_request(
            "export result too large; please narrow by start_date/end_date",
        ));
    }

    let svg = build_timeline_svg(
        WIDTH,
        height,
        &case_name,
        &user.username,
        &nodes,
        start,
        end,
    );
    let png = render_svg_png(&svg, WIDTH, height)?;

    let file_name = format!(
        "{}_timeline_{}.png",
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

    tokio::fs::write(&full_path, &png)
        .await
        .map_err(|e| AppError::internal(format!("write export failed: {e}")))?;

    let record_id = Uuid::new_v4().to_string();
    let node_ids_json =
        serde_json::to_string(&node_ids).map_err(|e| AppError::internal(format!("{e}")))?;
    let evidence_ids_json = if evidence_ids.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&evidence_ids).map_err(|e| AppError::internal(format!("{e}")))?)
    };

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
    .bind("timeline")
    .bind(&file_name)
    .bind(&storage_path)
    .bind(png.len() as i64)
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
                "export_type": "timeline",
                "file_name": file_name,
                "storage_path": storage_path,
                "file_size": png.len(),
                "node_count": node_ids.len(),
                "evidence_count": evidence_ids.len(),
                "start_date": start.map(|d| d.to_string()),
                "end_date": end.map(|d| d.to_string()),
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

    resp.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));

    let cd = format!("attachment; filename=\"{}\"", sanitize_filename(&file_name));
    if let Ok(v) = cd.parse::<HeaderValue>() {
        resp.headers_mut().insert(header::CONTENT_DISPOSITION, v);
    }

    Ok(resp)
}

async fn export_timeline_report(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(case_id): Path<String>,
    Query(q): Query<TimelineReportQuery>,
) -> AppResult<Response> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    let fmt = q
        .format
        .as_deref()
        .unwrap_or("pdf")
        .trim()
        .to_ascii_lowercase();

    if fmt != "pdf" && fmt != "html" {
        return Err(AppError::bad_request(
            "only format=pdf or format=html is supported",
        ));
    }

    let (start, end) = parse_date_range(q.start_date.as_deref(), q.end_date.as_deref())?;

    let case_name: Option<String> = sqlx::query_scalar("SELECT name FROM cases WHERE id = ?1")
        .bind(&case_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;
    let Some(case_name) = case_name else {
        return Err(AppError::not_found("case not found"));
    };

    const MAX_NODES: usize = 200;
    let nodes: Vec<TimelineReportNodeRow> = {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
            r#"
            SELECT
              n.id,
              n.title,
              n.description,
              n.event_time,
              (
                SELECT COUNT(1)
                FROM node_evidence_links l
                JOIN evidence_files e ON e.id = l.evidence_id AND e.status != 'deleted'
                WHERE l.node_id = n.id
              ) AS evidence_count
            FROM event_nodes n
            WHERE n.case_id =
            "#,
        );
        qb.push_bind(&case_id);
        qb.push(" AND n.status != 'deleted'");
        if let Some(start) = start {
            qb.push(" AND n.event_time >= ");
            qb.push_bind(start.to_string());
        }
        if let Some(end) = end {
            qb.push(" AND n.event_time <= ");
            qb.push_bind(end.to_string());
        }
        qb.push(" ORDER BY n.event_time ASC, n.sort_order ASC LIMIT ");
        qb.push_bind((MAX_NODES + 1) as i64);

        qb.build_query_as::<TimelineReportNodeRow>()
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    };

    if nodes.len() > MAX_NODES {
        return Err(AppError::bad_request(
            "too many nodes to export; please narrow by start_date/end_date",
        ));
    }

    let node_ids = nodes.iter().map(|n| n.id.clone()).collect::<Vec<_>>();
    let evidence_ids = fetch_evidence_ids_for_nodes(&state, &node_ids).await?;

    let timeline_nodes = nodes
        .iter()
        .map(|n| TimelineNodeExportRow {
            id: n.id.clone(),
            title: n.title.clone(),
            description: n.description.clone(),
            event_time: n.event_time.clone(),
        })
        .collect::<Vec<_>>();

    // Reuse the same SVG timeline renderer for the report cover image.
    const WIDTH: u32 = 1400;
    const HEADER_H: u32 = 140;
    const ROW_H: u32 = 72;
    const FOOTER_H: u32 = 80;
    const MAX_HEIGHT: u32 = 20_000;
    let rows = (timeline_nodes.len().max(1)) as u32;
    let height = HEADER_H + (ROW_H * rows) + FOOTER_H;
    if height > MAX_HEIGHT {
        return Err(AppError::bad_request(
            "export result too large; please narrow by start_date/end_date",
        ));
    }

    let svg = build_timeline_svg(
        WIDTH,
        height,
        &case_name,
        &user.username,
        &timeline_nodes,
        start,
        end,
    );
    let timeline_png = render_svg_png(&svg, WIDTH, height)?;

    let generated_at = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string();
    let report_title = format!("{} - 时间轴报告", case_name);

    const TEMPLATE_NAME: &str = "templates/timeline_report.html";
    let (template, template_source) = load_timeline_report_template();

    let (bytes, content_type, file_ext) = if fmt == "html" {
        let data_url = format!("data:image/png;base64,{}", STANDARD.encode(&timeline_png));
        let html = render_timeline_report_html(
            &template,
            &report_title,
            &generated_at,
            &user.username,
            start,
            end,
            &nodes,
            &data_url,
        );
        (
            html.into_bytes(),
            HeaderValue::from_static("text/html; charset=utf-8"),
            "html",
        )
    } else {
        let html = render_timeline_report_html(
            &template,
            &report_title,
            &generated_at,
            &user.username,
            start,
            end,
            &nodes,
            "timeline.png",
        );
        let mut images = BTreeMap::new();
        images.insert(
            "timeline.png".to_string(),
            Base64OrRaw::Raw(timeline_png.clone()),
        );
        let pdf = render_html_to_pdf(&html, &images)?;
        (pdf, HeaderValue::from_static("application/pdf"), "pdf")
    };

    let file_name = format!(
        "{}_timeline_report_{}.{}",
        case_id,
        chrono::Utc::now().format("%Y%m%d_%H%M%S"),
        file_ext
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
    let evidence_ids_json = if evidence_ids.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&evidence_ids).map_err(|e| AppError::internal(format!("{e}")))?)
    };

    sqlx::query(
        r#"
        INSERT INTO export_records (
          id, case_id, export_type, file_name, storage_path, file_size, generated_by, node_ids, evidence_ids, template_name
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(&record_id)
    .bind(&case_id)
    .bind("report")
    .bind(&file_name)
    .bind(&storage_path)
    .bind(bytes.len() as i64)
    .bind(user.user_id.to_string())
    .bind(node_ids_json)
    .bind(evidence_ids_json)
    .bind(TEMPLATE_NAME)
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
                "export_type": "report",
                "report_kind": "timeline_report",
                "format": fmt,
                "template_name": TEMPLATE_NAME,
                "template_source": template_source.as_str(),
                "file_name": file_name,
                "storage_path": storage_path,
                "file_size": bytes.len(),
                "node_count": node_ids.len(),
                "evidence_count": evidence_ids.len(),
                "start_date": start.map(|d| d.to_string()),
                "end_date": end.map(|d| d.to_string()),
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

    resp.headers_mut()
        .insert(header::CONTENT_TYPE, content_type);

    let cd = format!("attachment; filename=\"{}\"", sanitize_filename(&file_name));
    if let Ok(v) = cd.parse::<HeaderValue>() {
        resp.headers_mut().insert(header::CONTENT_DISPOSITION, v);
    }

    Ok(resp)
}

#[derive(Debug, sqlx::FromRow)]
struct TimelineNodeExportRow {
    id: String,
    title: String,
    description: Option<String>,
    event_time: String,
}

#[derive(Debug, sqlx::FromRow)]
struct TimelineReportNodeRow {
    id: String,
    title: String,
    description: Option<String>,
    event_time: String,
    evidence_count: i64,
}

async fn fetch_evidence_ids_for_nodes(
    state: &AppState,
    node_ids: &[String],
) -> AppResult<Vec<String>> {
    if node_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        r#"
        SELECT DISTINCT l.evidence_id
        FROM node_evidence_links l
        JOIN evidence_files e ON e.id = l.evidence_id
        WHERE e.status != 'deleted' AND l.node_id IN (
        "#,
    );

    let mut separated = qb.separated(", ");
    for id in node_ids {
        separated.push_bind(id);
    }
    separated.push_unseparated(")");

    qb.build_query_scalar::<String>()
        .fetch_all(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))
}

fn build_timeline_svg(
    width: u32,
    height: u32,
    case_name: &str,
    username: &str,
    nodes: &[TimelineNodeExportRow],
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
) -> String {
    const MARGIN_X: u32 = 60;
    const LINE_X: u32 = 260;
    const HEADER_H: u32 = 140;
    const ROW_H: u32 = 72;

    let content_x = LINE_X + 30;
    let content_w = width.saturating_sub(content_x + MARGIN_X);
    let title_max_chars = ((content_w as f32) / (18.0 * 0.95)).floor().max(10.0) as usize;
    let desc_max_chars = ((content_w as f32) / (14.0 * 0.95)).floor().max(10.0) as usize;

    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#
    ));
    svg.push_str(
        r#"<style>text{font-family:"Noto Sans CJK SC","Noto Sans CJK","DejaVu Sans",sans-serif;}</style>"#,
    );
    svg.push_str(r##"<rect x="0" y="0" width="100%" height="100%" fill="#ffffff"/>"##);

    // Header.
    svg.push_str(&format!(
        r##"<text x="{x}" y="60" font-size="28" font-weight="700" fill="#111">{t}</text>"##,
        x = MARGIN_X,
        t = escape_xml(case_name),
    ));

    let mut subtitle = format!("Timeline Export  Generated by {username}");
    if let Some(s) = start {
        subtitle.push_str(&format!("  start={}", s.format("%Y-%m-%d")));
    }
    if let Some(e) = end {
        subtitle.push_str(&format!("  end={}", e.format("%Y-%m-%d")));
    }
    svg.push_str(&format!(
        r##"<text x="{x}" y="92" font-size="14" fill="#555">{t}</text>"##,
        x = MARGIN_X,
        t = escape_xml(&subtitle),
    ));

    let generated_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    svg.push_str(&format!(
        r##"<text x="{x}" y="114" font-size="12" fill="#777">Generated at {t}</text>"##,
        x = MARGIN_X,
        t = escape_xml(&generated_at.to_string()),
    ));

    // Timeline spine.
    let start_y = HEADER_H;
    let end_y = height.saturating_sub(60);
    svg.push_str(&format!(
        r##"<line x1="{x}" y1="{y1}" x2="{x}" y2="{y2}" stroke="#c8cdd3" stroke-width="4"/>"##,
        x = LINE_X,
        y1 = start_y,
        y2 = end_y
    ));

    for (i, n) in nodes.iter().enumerate() {
        let row_top = start_y + (i as u32) * ROW_H;
        let y = row_top + (ROW_H / 2);

        // Node marker.
        svg.push_str(&format!(
            r##"<circle cx="{x}" cy="{y}" r="8" fill="#2f5d8a"/>"##,
            x = LINE_X,
            y = y
        ));
        svg.push_str(&format!(
            r##"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="#2f5d8a" stroke-width="3"/>"##,
            x1 = LINE_X + 8,
            x2 = LINE_X + 20,
            y = y
        ));

        // Date.
        svg.push_str(&format!(
            r##"<text x="{x}" y="{y}" font-size="14" fill="#333">{t}</text>"##,
            x = MARGIN_X,
            y = y + 5,
            t = escape_xml(&n.event_time)
        ));

        // Title.
        let title_lines = wrap_text_lines(&n.title, title_max_chars, 2);
        svg.push_str(&svg_multiline_text(
            content_x,
            y + 5,
            18,
            "#111",
            &title_lines,
        ));

        // Description (optional).
        if let Some(desc) = n
            .description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let desc_lines = wrap_text_lines(desc, desc_max_chars, 3);
            svg.push_str(&svg_multiline_text(
                content_x,
                y + 28,
                14,
                "#555",
                &desc_lines,
            ));
        }
    }

    svg.push_str("</svg>");
    svg
}

fn svg_multiline_text(x: u32, y: u32, font_size: u32, fill: &str, lines: &[String]) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str(&format!(
        r#"<text x="{x}" y="{y}" font-size="{font_size}" fill="{fill}">"#,
        x = x,
        y = y,
        font_size = font_size,
        fill = fill
    ));

    for (idx, line) in lines.iter().enumerate() {
        if idx == 0 {
            out.push_str(&format!("<tspan>{}</tspan>", escape_xml(line)));
        } else {
            out.push_str(&format!(
                r#"<tspan x="{x}" dy="{dy}">{t}</tspan>"#,
                x = x,
                dy = (font_size as i32 + 4).max(12),
                t = escape_xml(line)
            ));
        }
    }

    out.push_str("</text>");
    out
}

fn wrap_text_lines(raw: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let text = raw.trim();
    if text.is_empty() || max_chars == 0 || max_lines == 0 {
        return Vec::new();
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if current.chars().count() >= max_chars {
            lines.push(current);
            current = String::new();
        }
        current.push(ch);
        if lines.len() >= max_lines {
            break;
        }
    }
    if !current.is_empty() && lines.len() < max_lines {
        lines.push(current);
    }

    if lines.len() == max_lines
        && text.chars().count() > lines.iter().map(|l| l.chars().count()).sum()
    {
        // Indicate truncation on the last line.
        let last = lines.last_mut().expect("last line exists");
        let mut truncated = last
            .chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>();
        truncated.push_str("...");
        *last = truncated;
    }

    lines
}

fn escape_xml(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn render_svg_png(svg: &str, width: u32, height: u32) -> AppResult<Vec<u8>> {
    let mut opt = usvg::Options::default();
    // Text rendering depends on fonts. Load system fonts when available.
    opt.fontdb_mut().load_system_fonts();

    let tree = usvg::Tree::from_str(svg, &opt)
        .map_err(|e| AppError::internal(format!("invalid svg: {e:?}")))?;

    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| AppError::internal("failed to create pixmap"))?;

    // Render with identity transform. The SVG uses explicit pixel sizes.
    let mut pm = pixmap.as_mut();
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pm);

    pixmap
        .encode_png()
        .map_err(|e| AppError::internal(format!("encode png failed: {e}")))
}

const EMBEDDED_TIMELINE_REPORT_TEMPLATE: &str =
    include_str!("../../../templates/timeline_report.html");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TemplateSource {
    File,
    Embedded,
}

impl TemplateSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Embedded => "embedded",
        }
    }
}

fn load_timeline_report_template() -> (String, TemplateSource) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("templates")
        .join("timeline_report.html");

    match std::fs::read_to_string(path) {
        Ok(s) if !s.trim().is_empty() => (s, TemplateSource::File),
        _ => (
            EMBEDDED_TIMELINE_REPORT_TEMPLATE.to_string(),
            TemplateSource::Embedded,
        ),
    }
}

fn render_timeline_report_html(
    template: &str,
    title: &str,
    generated_at: &str,
    username: &str,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
    nodes: &[TimelineReportNodeRow],
    timeline_img_src: &str,
) -> String {
    let range = build_range_label(start, end);
    let node_rows = build_timeline_report_rows(nodes);

    render_template(
        template,
        &[
            ("title", escape_html(title)),
            ("generated_at", escape_html(generated_at)),
            ("generated_by", escape_html(username)),
            ("range", escape_html(&range)),
            ("node_count", nodes.len().to_string()),
            ("timeline_img_src", escape_html(timeline_img_src)),
            // Pre-escaped per-cell; safe to inject as raw table rows.
            ("node_rows", node_rows),
        ],
    )
}

fn build_range_label(start: Option<NaiveDate>, end: Option<NaiveDate>) -> String {
    let mut range = String::new();
    if let Some(s) = start {
        range.push_str(&format!("start={}", s.format("%Y-%m-%d")));
    }
    if let Some(e) = end {
        if !range.is_empty() {
            range.push_str("  ");
        }
        range.push_str(&format!("end={}", e.format("%Y-%m-%d")));
    }
    if range.is_empty() {
        range = "range=all".to_string();
    }
    range
}

fn build_timeline_report_rows(nodes: &[TimelineReportNodeRow]) -> String {
    let mut rows = String::new();
    for n in nodes {
        let desc = n.description.as_deref().unwrap_or("");
        rows.push_str("<tr>");
        rows.push_str(&format!(
            "<td class=\"col-date\">{}</td>",
            escape_html(&n.event_time)
        ));
        rows.push_str(&format!("<td>{}</td>", escape_html(&n.title)));
        rows.push_str(&format!("<td>{}</td>", escape_html(desc)));
        rows.push_str(&format!("<td class=\"col-ev\">{}</td>", n.evidence_count));
        rows.push_str("</tr>");
    }
    rows
}

fn render_template(template: &str, vars: &[(&str, String)]) -> String {
    let mut out = template.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{k}}}}}"), v);
    }
    out
}

fn render_html_to_pdf(html: &str, images: &BTreeMap<String, Base64OrRaw>) -> AppResult<Vec<u8>> {
    let fonts: BTreeMap<String, Base64OrRaw> = BTreeMap::new();
    let options = GeneratePdfOptions {
        margin_top: Some(12.0),
        margin_right: Some(12.0),
        margin_bottom: Some(12.0),
        margin_left: Some(12.0),
        show_page_numbers: Some(true),
        ..Default::default()
    };

    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let doc = PdfDocument::from_html(html, images, &fonts, &options, &mut warnings)
        .map_err(|e| AppError::internal(format!("pdf generation failed: {e}")))?;

    let mut save_warnings: Vec<PdfWarnMsg> = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut save_warnings);
    Ok(bytes)
}

fn escape_html(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

fn parse_date_range(
    start: Option<&str>,
    end: Option<&str>,
) -> AppResult<(Option<NaiveDate>, Option<NaiveDate>)> {
    let start = start.map(parse_date).transpose()?;
    let end = end.map(parse_date).transpose()?;
    if let (Some(s), Some(e)) = (start, end) {
        if s > e {
            return Err(AppError::bad_request("start_date must be <= end_date"));
        }
    }
    Ok((start, end))
}

fn parse_date(raw: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map_err(|_| AppError::bad_request("invalid date (expected YYYY-MM-DD)"))
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
