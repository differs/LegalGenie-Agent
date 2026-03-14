use crate::api::ApiEnvelope;
use crate::context::RequestMeta;
use crate::errors::{AppError, AppResult};
use crate::oplog::{spawn_operation_log, OperationLogNew};
use crate::routes::auth::AuthUser;
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_cases).post(create_case))
        .route("/:id", get(get_case).put(update_case).delete(delete_case))
        .merge(super::case_files::router())
        .merge(super::case_members::router())
        .merge(super::case_persons::router())
        .merge(super::case_exports::router())
        .merge(super::case_timeline::router())
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
struct CreateCaseRequest {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct UpdateCaseRequest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    status: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct CaseRow {
    id: String,
    name: String,
    description: Option<String>,
    status: String,
    tags: Option<String>,
    owner_id: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct CaseSummary {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    status: String,
    created_at: String,
    member_count: i64,
    evidence_count: i64,
    node_count: i64,
}

#[derive(Debug, Serialize)]
struct CaseListData {
    cases: Vec<CaseSummary>,
    total: i64,
    page: i64,
    page_size: i64,
}

#[derive(Debug, Serialize)]
struct CaseDetail {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
    owner_id: String,
    created_at: String,
    updated_at: String,
}

async fn list_cases(
    State(state): State<AppState>,
    user: AuthUser,
    Query(p): Query<Pagination>,
) -> AppResult<Json<ApiEnvelope<CaseListData>>> {
    let page = p.page.max(1);
    let page_size = p.page_size.clamp(1, 200);
    let offset = (page - 1) * page_size;

    let total: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(1)
        FROM cases c
        WHERE c.status != 'deleted'
          AND (c.owner_id = ?1 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = ?1
          ))
        "#,
    )
    .bind(user.user_id.to_string())
    .fetch_one(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let rows: Vec<(String, String, Option<String>, String, String, i64, i64, i64)> = sqlx::query_as(
        r#"
        SELECT
            c.id,
            c.name,
            c.description,
            c.status,
            c.created_at,
            (SELECT COUNT(1) FROM case_members m2 WHERE m2.case_id = c.id) as member_count,
            (SELECT COUNT(1) FROM evidence_files ef WHERE ef.case_id = c.id AND ef.status != 'deleted') as evidence_count,
            (SELECT COUNT(1) FROM event_nodes n WHERE n.case_id = c.id AND n.status != 'deleted') as node_count
        FROM cases c
        WHERE c.status != 'deleted'
          AND (c.owner_id = ?1 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = ?1
          ))
        ORDER BY c.created_at DESC
        LIMIT ?2 OFFSET ?3
        "#,
    )
    .bind(user.user_id.to_string())
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let cases = rows
        .into_iter()
        .map(
            |(
                id,
                name,
                description,
                status,
                created_at,
                member_count,
                evidence_count,
                node_count,
            )| CaseSummary {
                id,
                name,
                description,
                status,
                created_at,
                member_count,
                evidence_count,
                node_count,
            },
        )
        .collect::<Vec<_>>();

    Ok(Json(ApiEnvelope::ok(CaseListData {
        cases,
        total: total.0,
        page,
        page_size,
    })))
}

async fn create_case(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Json(req): Json<CreateCaseRequest>,
) -> AppResult<Json<ApiEnvelope<CaseDetail>>> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(AppError::bad_request("case name required"));
    }

    let case_id = Uuid::new_v4();
    let tags_json = req
        .tags
        .as_ref()
        .map(|t| serde_json::to_string(t))
        .transpose()
        .map_err(|e| AppError::bad_request(format!("invalid tags: {e}")))?;

    sqlx::query(
        "INSERT INTO cases (id, name, description, owner_id, status, tags) VALUES (?1, ?2, ?3, ?4, 'active', ?5)",
    )
    .bind(case_id.to_string())
    .bind(name)
    .bind(req.description.clone())
    .bind(user.user_id.to_string())
    .bind(tags_json.clone())
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    // Ensure owner is also a member.
    sqlx::query(
        "INSERT OR IGNORE INTO case_members (id, case_id, user_id, role_in_case, joined_by) VALUES (?1, ?2, ?3, 'owner', ?3)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(case_id.to_string())
    .bind(user.user_id.to_string())
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let detail = fetch_case_detail(&state, &user, &case_id.to_string()).await?;

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: Some(detail.id.clone()),
            action: "CREATE".to_string(),
            module: "case".to_string(),
            target_type: "case".to_string(),
            target_id: Some(detail.id.clone()),
            target_title: Some(detail.name.clone()),
            old_value: None,
            new_value: Some(serde_json::json!({
                "id": detail.id.clone(),
                "name": detail.name.clone(),
                "description": detail.description.clone(),
                "status": detail.status.clone(),
                "tags": detail.tags.clone(),
                "owner_id": detail.owner_id.clone(),
            })),
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    Ok(Json(ApiEnvelope::ok(detail)))
}

async fn get_case(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<CaseDetail>>> {
    let row: Option<CaseRow> = sqlx::query_as(
        "SELECT id, name, description, status, tags, owner_id, created_at, updated_at FROM cases WHERE id = ?1 LIMIT 1",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some(row) = row else {
        return Err(AppError::not_found("case not found"));
    };

    ensure_case_access(&state, &user, &row).await?;

    let tags = row
        .tags
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok());

    Ok(Json(ApiEnvelope::ok(CaseDetail {
        id: row.id,
        name: row.name,
        description: row.description,
        status: row.status,
        tags,
        owner_id: row.owner_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })))
}

async fn update_case(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(id): Path<String>,
    Json(req): Json<UpdateCaseRequest>,
) -> AppResult<Json<ApiEnvelope<CaseDetail>>> {
    let existing: Option<CaseRow> = sqlx::query_as(
        "SELECT id, name, description, status, tags, owner_id, created_at, updated_at FROM cases WHERE id = ?1 LIMIT 1",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some(existing) = existing else {
        return Err(AppError::not_found("case not found"));
    };

    ensure_case_access(&state, &user, &existing).await?;

    let old_tags = existing
        .tags
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok());
    let old_value = serde_json::json!({
        "id": existing.id.clone(),
        "name": existing.name.clone(),
        "description": existing.description.clone(),
        "status": existing.status.clone(),
        "tags": old_tags,
        "owner_id": existing.owner_id.clone(),
    });

    let new_name = req
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&existing.name)
        .to_string();

    let new_description = req.description.or(existing.description);

    let new_tags_json = match req.tags {
        Some(tags) => Some(
            serde_json::to_string(&tags)
                .map_err(|e| AppError::bad_request(format!("invalid tags: {e}")))?,
        ),
        None => existing.tags,
    };

    let new_status = req.status.unwrap_or(existing.status);

    sqlx::query(
        "UPDATE cases SET name = ?1, description = ?2, tags = ?3, status = ?4, updated_at = CURRENT_TIMESTAMP WHERE id = ?5",
    )
    .bind(&new_name)
    .bind(&new_description)
    .bind(&new_tags_json)
    .bind(&new_status)
    .bind(&id)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let detail = fetch_case_detail(&state, &user, &id).await?;

    let new_value = serde_json::json!({
        "id": detail.id.clone(),
        "name": detail.name.clone(),
        "description": detail.description.clone(),
        "status": detail.status.clone(),
        "tags": detail.tags.clone(),
        "owner_id": detail.owner_id.clone(),
    });
    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: Some(detail.id.clone()),
            action: "UPDATE".to_string(),
            module: "case".to_string(),
            target_type: "case".to_string(),
            target_id: Some(detail.id.clone()),
            target_title: Some(detail.name.clone()),
            old_value: Some(old_value),
            new_value: Some(new_value),
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    Ok(Json(ApiEnvelope::ok(detail)))
}

async fn delete_case(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<serde_json::Value>>> {
    crate::access::ensure_case_owner(&state.pool, user.user_id, &id).await?;

    let row: Option<(
        String,
        String,
        Option<String>,
        String,
        Option<String>,
        String,
    )> = sqlx::query_as(
        "SELECT id, name, description, status, tags, owner_id FROM cases WHERE id = ?1 LIMIT 1",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    sqlx::query(
        "UPDATE cases SET status = 'deleted', updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
    )
    .bind(&id)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    if let Some((case_id, name, description, status, tags_raw, owner_id)) = row {
        let tags = tags_raw
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok());
        let old_value = serde_json::json!({
            "id": case_id.clone(),
            "name": name.clone(),
            "description": description,
            "status": status,
            "tags": tags,
            "owner_id": owner_id,
        });

        spawn_operation_log(
            state.pool.clone(),
            OperationLogNew {
                user_id: user.user_id.to_string(),
                user_name: user.username.clone(),
                case_id: Some(case_id.clone()),
                action: "DELETE".to_string(),
                module: "case".to_string(),
                target_type: "case".to_string(),
                target_id: Some(case_id),
                target_title: Some(name),
                old_value: Some(old_value),
                new_value: None,
                changed_fields: None,
                ip_address: meta.ip_address,
                user_agent: meta.user_agent,
                request_id: Some(meta.request_id),
            },
        );
    }

    Ok(Json(ApiEnvelope::ok(serde_json::json!({}))))
}

async fn ensure_case_access(
    state: &AppState,
    user: &AuthUser,
    case_row: &CaseRow,
) -> AppResult<()> {
    if case_row.owner_id == user.user_id.to_string() {
        return Ok(());
    }

    let member: Option<(String,)> = sqlx::query_as(
        "SELECT role_in_case FROM case_members WHERE case_id = ?1 AND user_id = ?2 LIMIT 1",
    )
    .bind(&case_row.id)
    .bind(user.user_id.to_string())
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    if member.is_some() {
        Ok(())
    } else {
        Err(AppError::forbidden("no case access"))
    }
}

async fn fetch_case_detail(
    state: &AppState,
    user: &AuthUser,
    case_id: &str,
) -> AppResult<CaseDetail> {
    let row: Option<CaseRow> = sqlx::query_as(
        "SELECT id, name, description, status, tags, owner_id, created_at, updated_at FROM cases WHERE id = ?1 LIMIT 1",
    )
    .bind(case_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some(row) = row else {
        return Err(AppError::not_found("case not found"));
    };
    ensure_case_access(state, user, &row).await?;

    let tags = row
        .tags
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok());

    Ok(CaseDetail {
        id: row.id,
        name: row.name,
        description: row.description,
        status: row.status,
        tags,
        owner_id: row.owner_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}
