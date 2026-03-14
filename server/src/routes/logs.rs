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
use sqlx::{QueryBuilder, Sqlite};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_logs))
        .route("/export", get(export_logs))
        .route("/:target_type/:target_id/history", get(target_history))
}

#[derive(Debug, Deserialize)]
struct LogsQuery {
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    case_id: Option<String>,
    #[serde(default)]
    module: Option<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    start_time: Option<String>,
    #[serde(default)]
    end_time: Option<String>,
    #[serde(default)]
    keyword: Option<String>,
    #[serde(default)]
    format: Option<String>,
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
struct OperationLogItem {
    id: String,
    user_id: String,
    user_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    case_id: Option<String>,
    action: String,
    module: String,
    target_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    old_value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new_value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    changed_fields: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ip_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct LogsListData {
    logs: Vec<OperationLogItem>,
    total: i64,
    page: i64,
    page_size: i64,
}

async fn export_logs(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Query(q): Query<LogsQuery>,
) -> AppResult<Response> {
    let is_admin = user.roles.iter().any(|r| r == "admin");

    let user_id_filter = q
        .user_id
        .as_deref()
        .map(|s| normalize_uuid(s, "invalid user_id"))
        .transpose()?;
    if let Some(uid) = user_id_filter.as_deref() {
        if uid != user.user_id.to_string() && !is_admin {
            return Err(AppError::forbidden("no permission to query other users"));
        }
    }

    let case_id_filter = q
        .case_id
        .as_deref()
        .map(|s| normalize_uuid(s, "invalid case_id"))
        .transpose()?;
    if let Some(cid) = case_id_filter.as_deref() {
        ensure_case_access(&state.pool, user.user_id, cid).await?;
    }

    let module_filter = q
        .module
        .as_deref()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());
    let action_filter = q
        .action
        .as_deref()
        .map(|s| s.trim().to_ascii_uppercase())
        .filter(|s| !s.is_empty());
    let keyword_filter = q
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let start_time = q
        .start_time
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let end_time = q
        .end_time
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    // We only support CSV for now. `format=excel` is accepted for compatibility.
    let _format = q
        .format
        .as_deref()
        .unwrap_or("csv")
        .trim()
        .to_ascii_lowercase();

    const MAX_EXPORT_ROWS: i64 = 10_000;

    let rows: Vec<LogRow> = {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
            r#"
            SELECT
              l.id,
              l.user_id,
              l.user_name,
              l.case_id,
              l.action,
              l.module,
              l.target_type,
              l.target_id,
              l.target_title,
              l.old_value,
              l.new_value,
              l.changed_fields,
              l.ip_address,
              l.user_agent,
              l.request_id,
              l.created_at
            FROM operation_logs l
            WHERE
            "#,
        );
        apply_filters(
            &mut qb,
            &user,
            is_admin,
            user_id_filter.as_deref(),
            case_id_filter.as_deref(),
            module_filter.as_deref(),
            action_filter.as_deref(),
            start_time,
            end_time,
            keyword_filter,
        );
        qb.push(" ORDER BY l.created_at DESC LIMIT ");
        qb.push_bind(MAX_EXPORT_ROWS);

        qb.build_query_as::<LogRow>()
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    };

    let mut csv = String::new();
    csv.push_str("id,user_id,user_name,case_id,action,module,target_type,target_id,target_title,changed_fields,ip_address,created_at\n");
    for r in &rows {
        csv.push_str(&csv_row(&[
            &r.id,
            &r.user_id,
            &r.user_name,
            r.case_id.as_deref().unwrap_or(""),
            &r.action,
            &r.module,
            &r.target_type,
            r.target_id.as_deref().unwrap_or(""),
            r.target_title.as_deref().unwrap_or(""),
            r.changed_fields.as_deref().unwrap_or(""),
            r.ip_address.as_deref().unwrap_or(""),
            &r.created_at,
        ]));
        csv.push('\n');
    }

    // Audit log for export action (does not include exported content).
    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: case_id_filter.clone(),
            action: "EXPORT".to_string(),
            module: "logs".to_string(),
            target_type: "operation_logs".to_string(),
            target_id: None,
            target_title: None,
            old_value: None,
            new_value: Some(serde_json::json!({
                "row_count": rows.len(),
                "max_rows": MAX_EXPORT_ROWS,
            })),
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    let file_name = format!(
        "operation_logs_{}.csv",
        chrono::Utc::now().format("%Y%m%d_%H%M%S")
    );

    let bytes = csv.into_bytes();
    let body = Body::from(bytes);
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

#[derive(Debug, sqlx::FromRow)]
struct LogRow {
    id: String,
    user_id: String,
    user_name: String,
    case_id: Option<String>,
    action: String,
    module: String,
    target_type: String,
    target_id: Option<String>,
    target_title: Option<String>,
    old_value: Option<String>,
    new_value: Option<String>,
    changed_fields: Option<String>,
    ip_address: Option<String>,
    user_agent: Option<String>,
    request_id: Option<String>,
    created_at: String,
}

async fn list_logs(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<LogsQuery>,
) -> AppResult<Json<ApiEnvelope<LogsListData>>> {
    let is_admin = user.roles.iter().any(|r| r == "admin");

    let user_id_filter = q
        .user_id
        .as_deref()
        .map(|s| normalize_uuid(s, "invalid user_id"))
        .transpose()?;
    if let Some(uid) = user_id_filter.as_deref() {
        if uid != user.user_id.to_string() && !is_admin {
            return Err(AppError::forbidden("no permission to query other users"));
        }
    }

    let case_id_filter = q
        .case_id
        .as_deref()
        .map(|s| normalize_uuid(s, "invalid case_id"))
        .transpose()?;
    if let Some(cid) = case_id_filter.as_deref() {
        ensure_case_access(&state.pool, user.user_id, cid).await?;
    }

    let module_filter = q
        .module
        .as_deref()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());
    let action_filter = q
        .action
        .as_deref()
        .map(|s| s.trim().to_ascii_uppercase())
        .filter(|s| !s.is_empty());
    let keyword_filter = q
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let start_time = q
        .start_time
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let end_time = q
        .end_time
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let page = q.page.max(1);
    let page_size = q.page_size.clamp(1, 200);
    let offset = (page - 1) * page_size;

    let total: (i64,) = {
        let mut qb: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT COUNT(1) FROM operation_logs l WHERE ");
        apply_filters(
            &mut qb,
            &user,
            is_admin,
            user_id_filter.as_deref(),
            case_id_filter.as_deref(),
            module_filter.as_deref(),
            action_filter.as_deref(),
            start_time,
            end_time,
            keyword_filter,
        );

        qb.build_query_as::<(i64,)>()
            .fetch_one(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    };

    let rows: Vec<LogRow> = {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
            r#"
            SELECT
              l.id,
              l.user_id,
              l.user_name,
              l.case_id,
              l.action,
              l.module,
              l.target_type,
              l.target_id,
              l.target_title,
              l.old_value,
              l.new_value,
              l.changed_fields,
              l.ip_address,
              l.user_agent,
              l.request_id,
              l.created_at
            FROM operation_logs l
            WHERE
            "#,
        );
        apply_filters(
            &mut qb,
            &user,
            is_admin,
            user_id_filter.as_deref(),
            case_id_filter.as_deref(),
            module_filter.as_deref(),
            action_filter.as_deref(),
            start_time,
            end_time,
            keyword_filter,
        );
        qb.push(" ORDER BY l.created_at DESC LIMIT ");
        qb.push_bind(page_size);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        qb.build_query_as::<LogRow>()
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    };

    let logs = rows.into_iter().map(row_to_item).collect::<Vec<_>>();

    Ok(Json(ApiEnvelope::ok(LogsListData {
        logs,
        total: total.0,
        page,
        page_size,
    })))
}

#[derive(Debug, Serialize)]
struct HistoryItem {
    action: String,
    user_name: String,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    changes: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct TargetHistoryData {
    target_type: String,
    target_id: String,
    history: Vec<HistoryItem>,
}

#[derive(Debug, sqlx::FromRow)]
struct HistoryRow {
    user_id: String,
    user_name: String,
    case_id: Option<String>,
    action: String,
    old_value: Option<String>,
    new_value: Option<String>,
    created_at: String,
}

async fn target_history(
    State(state): State<AppState>,
    user: AuthUser,
    Path((target_type, target_id)): Path<(String, String)>,
) -> AppResult<Json<ApiEnvelope<TargetHistoryData>>> {
    let is_admin = user.roles.iter().any(|r| r == "admin");

    let tt = target_type.trim();
    let tid = target_id.trim();
    if tt.is_empty() || tid.is_empty() {
        return Err(AppError::bad_request("target_type/target_id required"));
    }

    let rows: Vec<HistoryRow> = sqlx::query_as(
        r#"
        SELECT user_id, user_name, case_id, action, old_value, new_value, created_at
        FROM operation_logs
        WHERE target_type = ?1 AND target_id = ?2
        ORDER BY created_at ASC
        "#,
    )
    .bind(tt)
    .bind(tid)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    if let Some(first) = rows.first() {
        if !is_admin {
            if let Some(case_id) = first.case_id.as_deref() {
                ensure_case_access(&state.pool, user.user_id, case_id).await?;
            } else if first.user_id != user.user_id.to_string() {
                return Err(AppError::forbidden("no permission to view history"));
            }
        }
    }

    let history = rows
        .into_iter()
        .map(|r| {
            let old_v = r
                .old_value
                .as_deref()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok());
            let new_v = r
                .new_value
                .as_deref()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok());
            let changes = diff_json(&old_v, &new_v);

            HistoryItem {
                action: r.action,
                user_name: r.user_name,
                created_at: r.created_at,
                changes,
            }
        })
        .collect::<Vec<_>>();

    Ok(Json(ApiEnvelope::ok(TargetHistoryData {
        target_type: tt.to_string(),
        target_id: tid.to_string(),
        history,
    })))
}

fn apply_filters(
    qb: &mut QueryBuilder<Sqlite>,
    user: &AuthUser,
    is_admin: bool,
    user_id: Option<&str>,
    case_id: Option<&str>,
    module: Option<&str>,
    action: Option<&str>,
    start_time: Option<&str>,
    end_time: Option<&str>,
    keyword: Option<&str>,
) {
    // Access control: if not admin, return (own non-case logs) OR (case logs for accessible cases).
    if is_admin {
        qb.push("1 = 1");
    } else {
        qb.push("(");
        qb.push("(l.case_id IS NULL AND l.user_id = ");
        qb.push_bind(user.user_id.to_string());
        qb.push(")");
        qb.push(" OR ");
        qb.push("(l.case_id IS NOT NULL AND EXISTS (");
        qb.push("SELECT 1 FROM cases c ");
        qb.push("WHERE c.id = l.case_id AND c.status != 'deleted' AND (c.owner_id = ");
        qb.push_bind(user.user_id.to_string());
        qb.push(" OR EXISTS (SELECT 1 FROM case_members m WHERE m.case_id = c.id AND m.user_id = ");
        qb.push_bind(user.user_id.to_string());
        qb.push("))");
        qb.push("))");
        qb.push(")");
    }

    if let Some(user_id) = user_id {
        qb.push(" AND l.user_id = ");
        qb.push_bind(user_id.to_string());
    }

    if let Some(case_id) = case_id {
        qb.push(" AND l.case_id = ");
        qb.push_bind(case_id.to_string());
    }

    if let Some(module) = module {
        qb.push(" AND l.module = ");
        qb.push_bind(module.to_string());
    }

    if let Some(action) = action {
        qb.push(" AND l.action = ");
        qb.push_bind(action.to_string());
    }

    if let Some(start) = start_time {
        qb.push(" AND l.created_at >= datetime(");
        qb.push_bind(start.to_string());
        qb.push(")");
    }

    if let Some(end) = end_time {
        qb.push(" AND l.created_at <= datetime(");
        qb.push_bind(end.to_string());
        qb.push(")");
    }

    if let Some(kw) = keyword {
        let like = format!("%{}%", kw);
        qb.push(" AND (l.user_name LIKE ");
        qb.push_bind(like.clone());
        qb.push(" OR l.target_title LIKE ");
        qb.push_bind(like.clone());
        qb.push(" OR l.changed_fields LIKE ");
        qb.push_bind(like);
        qb.push(")");
    }
}

fn row_to_item(r: LogRow) -> OperationLogItem {
    OperationLogItem {
        id: r.id,
        user_id: r.user_id,
        user_name: r.user_name,
        case_id: r.case_id,
        action: r.action,
        module: r.module,
        target_type: r.target_type,
        target_id: r.target_id,
        target_title: r.target_title,
        old_value: r
            .old_value
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok()),
        new_value: r
            .new_value
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok()),
        changed_fields: r.changed_fields,
        ip_address: r.ip_address,
        user_agent: r.user_agent,
        request_id: r.request_id,
        created_at: r.created_at,
    }
}

fn diff_json(
    old_value: &Option<serde_json::Value>,
    new_value: &Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    let (Some(serde_json::Value::Object(old)), Some(serde_json::Value::Object(new))) =
        (old_value, new_value)
    else {
        return None;
    };

    let mut keys = old.keys().chain(new.keys()).cloned().collect::<Vec<_>>();
    keys.sort();
    keys.dedup();

    let mut out = serde_json::Map::new();
    for k in keys {
        let o = old.get(&k);
        let n = new.get(&k);
        if o != n {
            out.insert(
                k,
                serde_json::json!({
                    "old": o.cloned().unwrap_or(serde_json::Value::Null),
                    "new": n.cloned().unwrap_or(serde_json::Value::Null),
                }),
            );
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(out))
    }
}

fn normalize_uuid(raw: &str, message: &str) -> AppResult<String> {
    let uuid = Uuid::parse_str(raw).map_err(|_| AppError::bad_request(message))?;
    Ok(uuid.to_string())
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

// Ensure we don't accidentally return a 200 on a handler that forgot to wrap `AppResult`.
#[allow(dead_code)]
fn _assert_logs_router_state(_: Router<AppState>) {}
