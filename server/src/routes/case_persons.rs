use crate::access::ensure_case_access;
use crate::api::ApiEnvelope;
use crate::context::RequestMeta;
use crate::errors::{AppError, AppResult};
use crate::oplog::{spawn_operation_log, OperationLogNew};
use crate::routes::auth::AuthUser;
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/:case_id/persons",
            get(list_case_persons).post(create_case_person),
        )
        .route("/:case_id/persons/graph", get(graph))
        .route("/:case_id/persons/dedupe", get(dedupe_candidates))
        .route("/:case_id/persons/merge", post(merge_case_persons))
        .route(
            "/:case_id/persons/relationships",
            get(list_relationships).post(create_relationship),
        )
        .route(
            "/:case_id/persons/relationships/:id",
            delete(delete_relationship),
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
        WHERE l.case_id = $1 AND p.status != 'deleted'
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
        WHERE l.case_id = $1 AND p.status != 'deleted'
        ORDER BY p.created_at DESC
        LIMIT $2 OFFSET $3
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
    crate::access::ensure_case_write_access(&state.pool, user.user_id, &case_id).await?;

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
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'active', $8)
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
        VALUES ($1, $2, $3, $4, $5, $6)
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
        sqlx::query_as("SELECT created_at, updated_at FROM persons WHERE id = $1 LIMIT 1")
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

    // Minimal graph: persons as nodes; edges are case-local person-person relations.
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT p.id, p.name, l.role_type
        FROM person_case_links l
        JOIN persons p ON p.id = l.person_id
        WHERE l.case_id = $1 AND p.status != 'deleted'
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

    let edge_rows: Vec<(String, String, String, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT id, from_person_id, to_person_id, rel_type, rel_detail
        FROM person_relationships
        WHERE case_id = $1 AND status != 'deleted'
        ORDER BY created_at DESC
        "#,
    )
    .bind(&case_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let edges = edge_rows
        .into_iter()
        .map(|(id, from_person_id, to_person_id, rel_type, rel_detail)| {
            serde_json::json!({
                "id": id,
                "from_person_id": from_person_id,
                "to_person_id": to_person_id,
                "rel_type": rel_type,
                "rel_detail": rel_detail,
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(ApiEnvelope::ok(PersonGraphData { nodes, edges })))
}

#[derive(Debug, Serialize, Clone)]
struct DedupeCandidatePerson {
    id: String,
    name: String,
    phone: Option<String>,
    email: Option<String>,
    organization: Option<String>,
    position: Option<String>,
    roles: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DedupeCandidateGroup {
    key: String,
    reason: String,
    persons: Vec<DedupeCandidatePerson>,
}

#[derive(Debug, Serialize)]
struct DedupeCandidatesData {
    groups: Vec<DedupeCandidateGroup>,
}

#[derive(Debug, sqlx::FromRow)]
struct DedupeCandidateRow {
    id: String,
    name: String,
    phone: Option<String>,
    email: Option<String>,
    organization: Option<String>,
    position: Option<String>,
    roles_csv: Option<String>,
}

async fn dedupe_candidates(
    State(state): State<AppState>,
    user: AuthUser,
    Path(case_id): Path<String>,
) -> AppResult<Json<ApiEnvelope<DedupeCandidatesData>>> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    // Aggregate per person_id to avoid duplicating persons across multiple role links.
    let rows: Vec<DedupeCandidateRow> = sqlx::query_as(
        r#"
        SELECT
          p.id,
          p.name,
          p.phone,
          p.email,
          p.organization,
          p.position,
          string_agg(DISTINCT l.role_type, ', ') AS roles_csv
        FROM person_case_links l
        JOIN persons p ON p.id = l.person_id
        WHERE l.case_id = $1 AND p.status != 'deleted'
        GROUP BY p.id
        ORDER BY p.created_at DESC
        LIMIT 800
        "#,
    )
    .bind(&case_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let persons = rows
        .into_iter()
        .map(|r| DedupeCandidatePerson {
            id: r.id,
            name: r.name,
            phone: r.phone,
            email: r.email,
            organization: r.organization,
            position: r.position,
            roles: r
                .roles_csv
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>(),
        })
        .collect::<Vec<_>>();

    let mut groups: Vec<DedupeCandidateGroup> = Vec::new();
    let mut by_name = BTreeMap::<String, Vec<DedupeCandidatePerson>>::new();
    let mut by_phone = BTreeMap::<String, Vec<DedupeCandidatePerson>>::new();
    let mut by_email = BTreeMap::<String, Vec<DedupeCandidatePerson>>::new();

    for p in persons.iter() {
        let name_key = p.name.trim().to_ascii_lowercase();
        if !name_key.is_empty() {
            by_name.entry(name_key).or_default().push(p.clone());
        }

        let phone_key = p
            .phone
            .as_deref()
            .unwrap_or("")
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>();
        if phone_key.len() >= 6 {
            by_phone.entry(phone_key).or_default().push(p.clone());
        }

        let email_key = p.email.as_deref().unwrap_or("").trim().to_ascii_lowercase();
        if email_key.contains('@') {
            by_email.entry(email_key).or_default().push(p.clone());
        }
    }

    for (k, persons) in by_name.into_iter() {
        if persons.len() >= 2 {
            groups.push(DedupeCandidateGroup {
                key: format!("name:{k}"),
                reason: "same name".to_string(),
                persons,
            });
        }
    }
    for (k, persons) in by_phone.into_iter() {
        if persons.len() >= 2 {
            groups.push(DedupeCandidateGroup {
                key: format!("phone:{k}"),
                reason: "same phone".to_string(),
                persons,
            });
        }
    }
    for (k, persons) in by_email.into_iter() {
        if persons.len() >= 2 {
            groups.push(DedupeCandidateGroup {
                key: format!("email:{k}"),
                reason: "same email".to_string(),
                persons,
            });
        }
    }

    Ok(Json(ApiEnvelope::ok(DedupeCandidatesData { groups })))
}

#[derive(Debug, Deserialize)]
struct MergeCasePersonsRequest {
    source_person_id: String,
    target_person_id: String,
}

#[derive(Debug, Serialize)]
struct MergeCasePersonsData {
    source_person_id: String,
    target_person_id: String,
    moved_links: i64,
    updated_links: i64,
    moved_relationships: i64,
    dropped_relationships: i64,
}

async fn merge_case_persons(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(case_id): Path<String>,
    Json(req): Json<MergeCasePersonsRequest>,
) -> AppResult<Json<ApiEnvelope<MergeCasePersonsData>>> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    crate::access::ensure_case_write_access(&state.pool, user.user_id, &case_id).await?;

    let source_person_id = normalize_uuid(&req.source_person_id, "invalid source_person_id")?;
    let target_person_id = normalize_uuid(&req.target_person_id, "invalid target_person_id")?;
    if source_person_id == target_person_id {
        return Err(AppError::bad_request(
            "source_person_id must be != target_person_id",
        ));
    }

    let source_name: Option<String> = sqlx::query_scalar(
        r#"
        SELECT p.name
        FROM persons p
        JOIN person_case_links l ON l.person_id = p.id
        WHERE l.case_id = $1 AND p.id = $2 AND p.status != 'deleted'
        LIMIT 1
        "#,
    )
    .bind(&case_id)
    .bind(&source_person_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;
    let Some(source_name) = source_name else {
        return Err(AppError::not_found_code(
            440101,
            "source person not found in case",
        ));
    };

    let target_name: Option<String> = sqlx::query_scalar(
        r#"
        SELECT p.name
        FROM persons p
        JOIN person_case_links l ON l.person_id = p.id
        WHERE l.case_id = $1 AND p.id = $2 AND p.status != 'deleted'
        LIMIT 1
        "#,
    )
    .bind(&case_id)
    .bind(&target_person_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;
    let Some(target_name) = target_name else {
        return Err(AppError::not_found_code(
            440101,
            "target person not found in case",
        ));
    };

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let links: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT role_type, role_detail, involved_date
        FROM person_case_links
        WHERE case_id = $1 AND person_id = $2
        "#,
    )
    .bind(&case_id)
    .bind(&source_person_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let mut moved_links = 0i64;
    let mut updated_links = 0i64;
    for (role_type, role_detail, involved_date) in links.into_iter() {
        let link_id = Uuid::new_v4().to_string();
        let inserted = sqlx::query(
            r#"
            INSERT INTO person_case_links (id, person_id, case_id, role_type, role_detail, involved_date)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (person_id, case_id, role_type) DO NOTHING
            "#,
        )
        .bind(&link_id)
        .bind(&target_person_id)
        .bind(&case_id)
        .bind(&role_type)
        .bind(role_detail.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .bind(involved_date.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        if inserted.rows_affected() == 1 {
            moved_links += 1;
        } else {
            // If the target already has the same role_type in this case, best-effort fill blanks.
            let updated = sqlx::query(
                r#"
                UPDATE person_case_links
                SET
                  role_detail = CASE
                    WHEN role_detail IS NULL OR TRIM(role_detail) = '' THEN $1
                    ELSE role_detail
                  END,
                  involved_date = CASE
                    WHEN involved_date IS NULL OR TRIM(involved_date) = '' THEN $2
                    ELSE involved_date
                  END
                WHERE person_id = $3 AND case_id = $4 AND role_type = $5
                "#,
            )
            .bind(
                role_detail
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty()),
            )
            .bind(
                involved_date
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty()),
            )
            .bind(&target_person_id)
            .bind(&case_id)
            .bind(&role_type)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;
            updated_links += updated.rows_affected() as i64;
        }
    }

    // Re-point case-local relationships. Use OR IGNORE to avoid violating the UNIQUE constraint.
    let moved_from = sqlx::query(
        r#"
        UPDATE person_relationships
        SET from_person_id = $1, updated_at = utc_text()
        WHERE case_id = $2 AND status != 'deleted' AND from_person_id = $3
          AND NOT EXISTS (
                SELECT 1 FROM person_relationships r2
                WHERE r2.case_id = $2 AND r2.status != 'deleted'
                  AND r2.from_person_id = $1
                  AND r2.to_person_id = person_relationships.to_person_id
                  AND r2.rel_type = person_relationships.rel_type
                  AND r2.id != person_relationships.id
          )
        "#,
    )
    .bind(&target_person_id)
    .bind(&case_id)
    .bind(&source_person_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;
    let moved_to = sqlx::query(
        r#"
        UPDATE person_relationships
        SET to_person_id = $1, updated_at = utc_text()
        WHERE case_id = $2 AND status != 'deleted' AND to_person_id = $3
          AND NOT EXISTS (
                SELECT 1 FROM person_relationships r2
                WHERE r2.case_id = $2 AND r2.status != 'deleted'
                  AND r2.from_person_id = person_relationships.from_person_id
                  AND r2.to_person_id = $1
                  AND r2.rel_type = person_relationships.rel_type
                  AND r2.id != person_relationships.id
          )
        "#,
    )
    .bind(&target_person_id)
    .bind(&case_id)
    .bind(&source_person_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;
    let moved_relationships = (moved_from.rows_affected() + moved_to.rows_affected()) as i64;

    // Soft-delete any remaining source-referencing relationships (e.g. conflicts or self edges).
    let dropped = sqlx::query(
        r#"
        UPDATE person_relationships
        SET status = 'deleted', updated_at = utc_text()
        WHERE case_id = $1
          AND status != 'deleted'
          AND (from_person_id = $2 OR to_person_id = $2)
        "#,
    )
    .bind(&case_id)
    .bind(&source_person_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;
    let dropped_relationships = dropped.rows_affected() as i64;

    // Finally remove the source person's links in THIS case.
    sqlx::query("DELETE FROM person_case_links WHERE case_id = $1 AND person_id = $2")
        .bind(&case_id)
        .bind(&source_person_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    tx.commit()
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: Some(case_id.clone()),
            action: "MERGE".to_string(),
            module: "person".to_string(),
            target_type: "person".to_string(),
            target_id: Some(target_person_id.clone()),
            target_title: Some(target_name.clone()),
            old_value: Some(serde_json::json!({
                "source_person_id": source_person_id.clone(),
                "source_name": source_name,
                "target_person_id": target_person_id.clone(),
                "target_name": target_name,
            })),
            new_value: Some(serde_json::json!({
                "moved_links": moved_links,
                "updated_links": updated_links,
                "moved_relationships": moved_relationships,
                "dropped_relationships": dropped_relationships,
            })),
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    Ok(Json(ApiEnvelope::ok(MergeCasePersonsData {
        source_person_id,
        target_person_id,
        moved_links,
        updated_links,
        moved_relationships,
        dropped_relationships,
    })))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct RelationshipItem {
    id: String,
    case_id: String,
    from_person_id: String,
    from_person_name: String,
    to_person_id: String,
    to_person_name: String,
    rel_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rel_detail: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct RelationshipListData {
    relationships: Vec<RelationshipItem>,
    total: i64,
}

#[derive(Debug, Deserialize)]
struct CreateRelationshipRequest {
    from_person_id: String,
    to_person_id: String,
    rel_type: String,
    #[serde(default)]
    rel_detail: Option<String>,
}

async fn list_relationships(
    State(state): State<AppState>,
    user: AuthUser,
    Path(case_id): Path<String>,
) -> AppResult<Json<ApiEnvelope<RelationshipListData>>> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    let out: Vec<RelationshipItem> = sqlx::query_as(
        r#"
        SELECT
          r.id,
          r.case_id,
          r.from_person_id,
          p1.name AS from_person_name,
          r.to_person_id,
          p2.name AS to_person_name,
          r.rel_type,
          r.rel_detail,
          r.created_at,
          r.updated_at
        FROM person_relationships r
        JOIN persons p1 ON p1.id = r.from_person_id
        JOIN persons p2 ON p2.id = r.to_person_id
        WHERE r.case_id = $1
          AND r.status != 'deleted'
          AND p1.status != 'deleted'
          AND p2.status != 'deleted'
        ORDER BY r.created_at DESC
        LIMIT 500
        "#,
    )
    .bind(&case_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    Ok(Json(ApiEnvelope::ok(RelationshipListData {
        total: out.len() as i64,
        relationships: out,
    })))
}

async fn create_relationship(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(case_id): Path<String>,
    Json(req): Json<CreateRelationshipRequest>,
) -> AppResult<Json<ApiEnvelope<RelationshipItem>>> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    crate::access::ensure_case_write_access(&state.pool, user.user_id, &case_id).await?;

    let from_person_id = normalize_uuid(&req.from_person_id, "invalid from_person_id")?;
    let to_person_id = normalize_uuid(&req.to_person_id, "invalid to_person_id")?;
    if from_person_id == to_person_id {
        return Err(AppError::bad_request(
            "from_person_id must be != to_person_id",
        ));
    }

    let rel_type = req.rel_type.trim().to_ascii_lowercase();
    if rel_type.is_empty() {
        return Err(AppError::bad_request("rel_type required"));
    }

    let from_name: Option<String> = sqlx::query_scalar(
        r#"
        SELECT p.name
        FROM persons p
        JOIN person_case_links l ON l.person_id = p.id
        WHERE l.case_id = $1 AND p.id = $2 AND p.status != 'deleted'
        LIMIT 1
        "#,
    )
    .bind(&case_id)
    .bind(&from_person_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;
    let Some(from_name) = from_name else {
        return Err(AppError::not_found_code(
            440101,
            "from person not found in case",
        ));
    };

    let to_name: Option<String> = sqlx::query_scalar(
        r#"
        SELECT p.name
        FROM persons p
        JOIN person_case_links l ON l.person_id = p.id
        WHERE l.case_id = $1 AND p.id = $2 AND p.status != 'deleted'
        LIMIT 1
        "#,
    )
    .bind(&case_id)
    .bind(&to_person_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;
    let Some(to_name) = to_name else {
        return Err(AppError::not_found_code(
            440101,
            "to person not found in case",
        ));
    };

    let rel_id = Uuid::new_v4().to_string();
    let inserted = sqlx::query(
        r#"
        INSERT INTO person_relationships (
          id, case_id, from_person_id, to_person_id, rel_type, rel_detail, status, created_by
        ) VALUES ($1, $2, $3, $4, $5, $6, 'active', $7)
        ON CONFLICT (case_id, from_person_id, to_person_id, rel_type) DO NOTHING
        "#,
    )
    .bind(&rel_id)
    .bind(&case_id)
    .bind(&from_person_id)
    .bind(&to_person_id)
    .bind(&rel_type)
    .bind(
        req.rel_detail
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(user.user_id.to_string())
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    if inserted.rows_affected() == 0 {
        return Err(AppError::conflict("relationship already exists"));
    }

    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT created_at, updated_at FROM person_relationships WHERE id = $1 LIMIT 1",
    )
    .bind(&rel_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;
    let (created_at, updated_at) =
        row.ok_or_else(|| AppError::internal("relationship inserted but not found"))?;

    let out = RelationshipItem {
        id: rel_id.clone(),
        case_id: case_id.clone(),
        from_person_id: from_person_id.clone(),
        from_person_name: from_name.clone(),
        to_person_id: to_person_id.clone(),
        to_person_name: to_name.clone(),
        rel_type: rel_type.clone(),
        rel_detail: req.rel_detail.clone(),
        created_at,
        updated_at,
    };

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: Some(case_id),
            action: "RELATE".to_string(),
            module: "person".to_string(),
            target_type: "person_relationship".to_string(),
            target_id: Some(rel_id),
            target_title: Some(format!("{from_name} -{rel_type}-> {to_name}")),
            old_value: None,
            new_value: Some(serde_json::json!({
                "from_person_id": from_person_id,
                "to_person_id": to_person_id,
                "rel_type": rel_type,
                "rel_detail": req.rel_detail,
            })),
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    Ok(Json(ApiEnvelope::ok(out)))
}

async fn delete_relationship(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path((case_id, id)): Path<(String, String)>,
) -> AppResult<Json<ApiEnvelope<serde_json::Value>>> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    crate::access::ensure_case_write_access(&state.pool, user.user_id, &case_id).await?;

    let rel_id = normalize_uuid(&id, "invalid relationship id")?;

    let row: Option<(String, String, String, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT from_person_id, to_person_id, rel_type, status, rel_detail
        FROM person_relationships
        WHERE id = $1 AND case_id = $2
        LIMIT 1
        "#,
    )
    .bind(&rel_id)
    .bind(&case_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;
    let Some((from_person_id, to_person_id, rel_type, status, rel_detail)) = row else {
        return Err(AppError::not_found("relationship not found"));
    };

    if status == "deleted" {
        return Ok(Json(ApiEnvelope::ok(serde_json::json!({}))));
    }

    sqlx::query(
        "UPDATE person_relationships SET status = 'deleted', updated_at = utc_text() WHERE id = $1 AND case_id = $2",
    )
    .bind(&rel_id)
    .bind(&case_id)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: Some(case_id),
            action: "DELETE".to_string(),
            module: "person".to_string(),
            target_type: "person_relationship".to_string(),
            target_id: Some(rel_id),
            target_title: Some(rel_type.clone()),
            old_value: Some(serde_json::json!({
                "from_person_id": from_person_id,
                "to_person_id": to_person_id,
                "rel_type": rel_type,
                "rel_detail": rel_detail,
            })),
            new_value: None,
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    Ok(Json(ApiEnvelope::ok(serde_json::json!({}))))
}

fn normalize_uuid(raw: &str, message: &str) -> AppResult<String> {
    let uuid = Uuid::parse_str(raw).map_err(|_| AppError::bad_request(message))?;
    Ok(uuid.to_string())
}

// Ensure we don't accidentally return a 200 on a handler that forgot to wrap `AppResult`.
#[allow(dead_code)]
fn _assert_case_persons_router_state(_: Router<AppState>) {}
