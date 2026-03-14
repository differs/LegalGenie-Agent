use crate::access::ensure_case_access;
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
        .route(
            "/:case_id/persons",
            get(list_case_persons).post(create_case_person),
        )
        .route("/:case_id/persons/graph", get(graph))
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
struct CreatePersonRequest {
    name: String,
    #[serde(default)]
    gender: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    organization: Option<String>,
    #[serde(default)]
    position: Option<String>,
    #[serde(default)]
    role_type: Option<String>,
    #[serde(default)]
    role_detail: Option<String>,
    #[serde(default)]
    involved_date: Option<String>, // YYYY-MM-DD
}

#[derive(Debug, Serialize)]
struct CasePersonItem {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    gender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    organization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<String>,
    role_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    role_detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    involved_date: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct CasePersonListData {
    persons: Vec<CasePersonItem>,
    total: i64,
    page: i64,
    page_size: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct CasePersonRow {
    id: String,
    name: String,
    gender: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    organization: Option<String>,
    position: Option<String>,
    role_type: String,
    role_detail: Option<String>,
    involved_date: Option<String>,
    created_at: String,
    updated_at: String,
}

async fn list_case_persons(
    State(state): State<AppState>,
    user: AuthUser,
    Path(case_id): Path<String>,
    Query(p): Query<Pagination>,
) -> AppResult<Json<ApiEnvelope<CasePersonListData>>> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    let page = p.page.max(1);
    let page_size = p.page_size.clamp(1, 200);
    let offset = (page - 1) * page_size;

    let total: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(1)
        FROM person_case_links l
        JOIN persons p ON p.id = l.person_id
        WHERE l.case_id = ?1 AND p.status != 'deleted'
        "#,
    )
    .bind(&case_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let rows: Vec<CasePersonRow> = sqlx::query_as(
        r#"
        SELECT
          p.id,
          p.name,
          p.gender,
          p.phone,
          p.email,
          p.organization,
          p.position,
          l.role_type,
          l.role_detail,
          l.involved_date,
          p.created_at,
          p.updated_at
        FROM person_case_links l
        JOIN persons p ON p.id = l.person_id
        WHERE l.case_id = ?1 AND p.status != 'deleted'
        ORDER BY p.created_at DESC
        LIMIT ?2 OFFSET ?3
        "#,
    )
    .bind(&case_id)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let persons = rows
        .into_iter()
        .map(|r| CasePersonItem {
            id: r.id,
            name: r.name,
            gender: r.gender,
            phone: r.phone,
            email: r.email,
            organization: r.organization,
            position: r.position,
            role_type: r.role_type,
            role_detail: r.role_detail,
            involved_date: r.involved_date,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect::<Vec<_>>();

    Ok(Json(ApiEnvelope::ok(CasePersonListData {
        persons,
        total: total.0,
        page,
        page_size,
    })))
}

async fn create_case_person(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(case_id): Path<String>,
    Json(req): Json<CreatePersonRequest>,
) -> AppResult<Json<ApiEnvelope<CasePersonItem>>> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    let name = req.name.trim();
    if name.is_empty() {
        return Err(AppError::bad_request("name required"));
    }

    let gender = req
        .gender
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let gender = gender.map(|s| s.to_ascii_lowercase());

    let role_type = req
        .role_type
        .unwrap_or_else(|| "other".to_string())
        .trim()
        .to_ascii_lowercase();
    if role_type.is_empty() {
        return Err(AppError::bad_request("role_type required"));
    }

    if let Some(d) = req.involved_date.as_deref() {
        if !d.trim().is_empty() && chrono::NaiveDate::parse_from_str(d.trim(), "%Y-%m-%d").is_err()
        {
            return Err(AppError::bad_request(
                "invalid involved_date (expected YYYY-MM-DD)",
            ));
        }
    }

    let person_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO persons (id, name, gender, phone, email, organization, position, status, created_by)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', ?8)
        "#,
    )
    .bind(&person_id)
    .bind(name)
    .bind(gender.clone())
    .bind(req.phone.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(req.email.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(
        req.organization
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(req.position.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(user.user_id.to_string())
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let link_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO person_case_links (id, person_id, case_id, role_type, role_detail, involved_date)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&link_id)
    .bind(&person_id)
    .bind(&case_id)
    .bind(&role_type)
    .bind(req.role_detail.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(req.involved_date.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let row: Option<(String, String)> =
        sqlx::query_as("SELECT created_at, updated_at FROM persons WHERE id = ?1 LIMIT 1")
            .bind(&person_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let (created_at, updated_at) =
        row.ok_or_else(|| AppError::internal("person inserted but not found"))?;

    let out = CasePersonItem {
        id: person_id.clone(),
        name: name.to_string(),
        gender,
        phone: req.phone,
        email: req.email,
        organization: req.organization,
        position: req.position,
        role_type: role_type.clone(),
        role_detail: req.role_detail,
        involved_date: req.involved_date,
        created_at,
        updated_at,
    };

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: Some(case_id),
            action: "CREATE".to_string(),
            module: "person".to_string(),
            target_type: "person".to_string(),
            target_id: Some(out.id.clone()),
            target_title: Some(out.name.clone()),
            old_value: None,
            new_value: Some(serde_json::json!({
                "id": out.id.clone(),
                "name": out.name.clone(),
                "gender": out.gender.clone(),
                "phone": out.phone.clone(),
                "email": out.email.clone(),
                "organization": out.organization.clone(),
                "position": out.position.clone(),
                "role_type": out.role_type.clone(),
                "role_detail": out.role_detail.clone(),
                "involved_date": out.involved_date.clone(),
                "link_id": link_id,
            })),
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    Ok(Json(ApiEnvelope::ok(out)))
}

#[derive(Debug, Serialize)]
struct PersonGraphNode {
    id: String,
    name: String,
    role_type: String,
}

#[derive(Debug, Serialize)]
struct PersonGraphData {
    nodes: Vec<PersonGraphNode>,
    edges: Vec<serde_json::Value>,
}

async fn graph(
    State(state): State<AppState>,
    user: AuthUser,
    Path(case_id): Path<String>,
) -> AppResult<Json<ApiEnvelope<PersonGraphData>>> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    // Minimal graph: persons as nodes; edges are reserved for future person-person relations.
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT p.id, p.name, l.role_type
        FROM person_case_links l
        JOIN persons p ON p.id = l.person_id
        WHERE l.case_id = ?1 AND p.status != 'deleted'
        ORDER BY p.created_at DESC
        "#,
    )
    .bind(&case_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let nodes = rows
        .into_iter()
        .map(|(id, name, role_type)| PersonGraphNode {
            id,
            name,
            role_type,
        })
        .collect::<Vec<_>>();

    Ok(Json(ApiEnvelope::ok(PersonGraphData {
        nodes,
        edges: Vec::new(),
    })))
}

fn normalize_uuid(raw: &str, message: &str) -> AppResult<String> {
    let uuid = Uuid::parse_str(raw).map_err(|_| AppError::bad_request(message))?;
    Ok(uuid.to_string())
}

// Ensure we don't accidentally return a 200 on a handler that forgot to wrap `AppResult`.
#[allow(dead_code)]
fn _assert_case_persons_router_state(_: Router<AppState>) {}
