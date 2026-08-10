use crate::api::ApiEnvelope;
use crate::context::RequestMeta;
use crate::errors::{AppError, AppResult};
use crate::oplog::{spawn_operation_log, OperationLogNew};
use crate::routes::auth::AuthUser;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/:id",
            axum::routing::get(get_person)
                .put(update_person)
                .delete(delete_person),
        )
        .route("/:id/cases", post(link_case))
}

#[derive(Debug, Serialize)]
struct PersonCaseLink {
    case_id: String,
    role_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    role_detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    involved_date: Option<String>,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct PersonDetail {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
    cases: Vec<PersonCaseLink>,
}

#[derive(Debug, sqlx::FromRow)]
struct PersonRow {
    id: String,
    name: String,
    gender: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    organization: Option<String>,
    position: Option<String>,
    notes: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
}

async fn get_person(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<PersonDetail>>> {
    let person_id = normalize_uuid(&id, "invalid person id")?;
    let person = fetch_person_with_access(&state, &user, &person_id).await?;

    let links: Vec<(String, String, Option<String>, Option<String>, String)> = sqlx::query_as(
        r#"
        SELECT l.case_id, l.role_type, l.role_detail, l.involved_date, l.created_at
        FROM person_case_links l
        JOIN cases c ON c.id = l.case_id
        WHERE l.person_id = $1
          AND c.status != 'deleted'
          AND (c.owner_id = $2 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $2
          ))
        ORDER BY l.created_at DESC
        "#,
    )
    .bind(&person_id)
    .bind(user.user_id.to_string())
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let cases = links
        .into_iter()
        .map(
            |(case_id, role_type, role_detail, involved_date, created_at)| PersonCaseLink {
                case_id,
                role_type,
                role_detail,
                involved_date,
                created_at,
            },
        )
        .collect::<Vec<_>>();

    Ok(Json(ApiEnvelope::ok(PersonDetail {
        id: person.id,
        name: person.name,
        gender: person.gender,
        phone: person.phone,
        email: person.email,
        organization: person.organization,
        position: person.position,
        notes: person.notes,
        status: person.status,
        created_at: person.created_at,
        updated_at: person.updated_at,
        cases,
    })))
}

#[derive(Debug, Deserialize)]
struct UpdatePersonRequest {
    #[serde(default)]
    name: Option<String>,
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
    notes: Option<String>,
}

async fn update_person(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(id): Path<String>,
    Json(req): Json<UpdatePersonRequest>,
) -> AppResult<Json<ApiEnvelope<PersonDetail>>> {
    let person_id = normalize_uuid(&id, "invalid person id")?;
    let existing = fetch_person_with_access(&state, &user, &person_id).await?;
    let case_id = primary_writable_case_for_person(&state, &user, &person_id).await?;
    if case_id.is_none() {
        return Err(AppError::forbidden("read-only case access"));
    }

    let old_value = serde_json::json!({
        "id": existing.id.clone(),
        "name": existing.name.clone(),
        "gender": existing.gender.clone(),
        "phone": existing.phone.clone(),
        "email": existing.email.clone(),
        "organization": existing.organization.clone(),
        "position": existing.position.clone(),
        "notes": existing.notes.clone(),
        "status": existing.status.clone(),
    });

    let name = req
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&existing.name)
        .to_string();

    let gender = req
        .gender
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let gender = gender.map(|s| s.to_ascii_lowercase()).or(existing.gender);

    let phone = req
        .phone
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let phone = phone.map(|s| s.to_string()).or(existing.phone);

    let email = req
        .email
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let email = email.map(|s| s.to_string()).or(existing.email);

    let organization = req
        .organization
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let organization = organization
        .map(|s| s.to_string())
        .or(existing.organization);

    let position = req
        .position
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let position = position.map(|s| s.to_string()).or(existing.position);

    let notes = req
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let notes = notes.map(|s| s.to_string()).or(existing.notes);

    sqlx::query(
        r#"
        UPDATE persons
        SET name = $1,
            gender = $2,
            phone = $3,
            email = $4,
            organization = $5,
            position = $6,
            notes = $7,
            updated_at = utc_text()
        WHERE id = $8 AND status != 'deleted'
        "#,
    )
    .bind(&name)
    .bind(&gender)
    .bind(&phone)
    .bind(&email)
    .bind(&organization)
    .bind(&position)
    .bind(&notes)
    .bind(&person_id)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let updated = fetch_person_with_access(&state, &user, &person_id).await?;

    let new_value = serde_json::json!({
        "id": updated.id.clone(),
        "name": updated.name.clone(),
        "gender": updated.gender.clone(),
        "phone": updated.phone.clone(),
        "email": updated.email.clone(),
        "organization": updated.organization.clone(),
        "position": updated.position.clone(),
        "notes": updated.notes.clone(),
        "status": updated.status.clone(),
    });

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id,
            action: "UPDATE".to_string(),
            module: "person".to_string(),
            target_type: "person".to_string(),
            target_id: Some(person_id),
            target_title: Some(updated.name.clone()),
            old_value: Some(old_value),
            new_value: Some(new_value),
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    let links = fetch_person_links(&state, &user, &updated.id).await?;

    Ok(Json(ApiEnvelope::ok(PersonDetail {
        id: updated.id,
        name: updated.name,
        gender: updated.gender,
        phone: updated.phone,
        email: updated.email,
        organization: updated.organization,
        position: updated.position,
        notes: updated.notes,
        status: updated.status,
        created_at: updated.created_at,
        updated_at: updated.updated_at,
        cases: links,
    })))
}

async fn delete_person(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<serde_json::Value>>> {
    let person_id = normalize_uuid(&id, "invalid person id")?;
    let existing = fetch_person_with_access(&state, &user, &person_id).await?;
    let case_id = primary_writable_case_for_person(&state, &user, &person_id).await?;
    if case_id.is_none() {
        return Err(AppError::forbidden("read-only case access"));
    }

    let old_value = serde_json::json!({
        "id": existing.id.clone(),
        "name": existing.name.clone(),
        "gender": existing.gender.clone(),
        "phone": existing.phone.clone(),
        "email": existing.email.clone(),
        "organization": existing.organization.clone(),
        "position": existing.position.clone(),
        "notes": existing.notes.clone(),
        "status": existing.status.clone(),
    });

    sqlx::query("UPDATE persons SET status = 'deleted', updated_at = utc_text() WHERE id = $1 AND status != 'deleted'")
        .bind(&person_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id,
            action: "DELETE".to_string(),
            module: "person".to_string(),
            target_type: "person".to_string(),
            target_id: Some(person_id),
            target_title: Some(existing.name),
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

#[derive(Debug, Deserialize)]
struct LinkCaseRequest {
    case_id: String,
    role_type: String,
    #[serde(default)]
    role_detail: Option<String>,
    #[serde(default)]
    involved_date: Option<String>, // YYYY-MM-DD
}

async fn link_case(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(id): Path<String>,
    Json(req): Json<LinkCaseRequest>,
) -> AppResult<Json<ApiEnvelope<serde_json::Value>>> {
    let person_id = normalize_uuid(&id, "invalid person id")?;
    // Ensure caller can see the person.
    let person = fetch_person_with_access(&state, &user, &person_id).await?;
    if primary_writable_case_for_person(&state, &user, &person_id)
        .await?
        .is_none()
    {
        return Err(AppError::forbidden("read-only case access"));
    }

    let case_id = normalize_uuid(&req.case_id, "invalid case_id")?;
    crate::access::ensure_case_write_access(&state.pool, user.user_id, &case_id).await?;

    let role_type = req.role_type.trim().to_ascii_lowercase();
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

    let link_id = Uuid::new_v4().to_string();
    let inserted = sqlx::query(
        r#"
        INSERT INTO person_case_links (id, person_id, case_id, role_type, role_detail, involved_date)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (person_id, case_id, role_type) DO NOTHING
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

    if inserted.rows_affected() == 0 {
        return Err(AppError::conflict("link already exists"));
    }

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: Some(case_id.clone()),
            action: "LINK".to_string(),
            module: "person".to_string(),
            target_type: "person_case_link".to_string(),
            target_id: Some(link_id.clone()),
            target_title: Some(person.name.clone()),
            old_value: None,
            new_value: Some(serde_json::json!({
                "id": link_id,
                "person_id": person_id,
                "case_id": case_id,
                "role_type": role_type,
                "role_detail": req.role_detail,
                "involved_date": req.involved_date,
            })),
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    Ok(Json(ApiEnvelope::ok(serde_json::json!({}))))
}

async fn fetch_person_with_access(
    state: &AppState,
    user: &AuthUser,
    person_id: &str,
) -> AppResult<PersonRow> {
    let row: Option<PersonRow> = sqlx::query_as(
        r#"
        SELECT
          p.id,
          p.name,
          p.gender,
          p.phone,
          p.email,
          p.organization,
          p.position,
          p.notes,
          p.status,
          p.created_at,
          p.updated_at
        FROM persons p
        WHERE p.id = $1
          AND p.status != 'deleted'
          AND EXISTS (
                SELECT 1
                FROM person_case_links l
                JOIN cases c ON c.id = l.case_id
                WHERE l.person_id = p.id
                  AND c.status != 'deleted'
                  AND (c.owner_id = $2 OR EXISTS (
                        SELECT 1 FROM case_members m
                        WHERE m.case_id = c.id AND m.user_id = $2
                  ))
          )
        LIMIT 1
        "#,
    )
    .bind(person_id)
    .bind(user.user_id.to_string())
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    row.ok_or_else(|| AppError::not_found_code(440101, "person not found"))
}

async fn primary_writable_case_for_person(
    state: &AppState,
    user: &AuthUser,
    person_id: &str,
) -> AppResult<Option<String>> {
    let uid = user.user_id.to_string();
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT l.case_id
        FROM person_case_links l
        JOIN cases c ON c.id = l.case_id
        LEFT JOIN case_members m ON m.case_id = c.id AND m.user_id = $2
        WHERE l.person_id = $1
          AND c.status != 'deleted'
          AND (
            c.owner_id = $2
            OR (m.role_in_case IN ('owner', 'member'))
          )
        ORDER BY l.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(person_id)
    .bind(uid)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    Ok(row.map(|(cid,)| cid))
}

async fn fetch_person_links(
    state: &AppState,
    user: &AuthUser,
    person_id: &str,
) -> AppResult<Vec<PersonCaseLink>> {
    let links: Vec<(String, String, Option<String>, Option<String>, String)> = sqlx::query_as(
        r#"
        SELECT l.case_id, l.role_type, l.role_detail, l.involved_date, l.created_at
        FROM person_case_links l
        JOIN cases c ON c.id = l.case_id
        WHERE l.person_id = $1
          AND c.status != 'deleted'
          AND (c.owner_id = $2 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $2
          ))
        ORDER BY l.created_at DESC
        "#,
    )
    .bind(person_id)
    .bind(user.user_id.to_string())
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    Ok(links
        .into_iter()
        .map(
            |(case_id, role_type, role_detail, involved_date, created_at)| PersonCaseLink {
                case_id,
                role_type,
                role_detail,
                involved_date,
                created_at,
            },
        )
        .collect::<Vec<_>>())
}

fn normalize_uuid(raw: &str, message: &str) -> AppResult<String> {
    let uuid = Uuid::parse_str(raw).map_err(|_| AppError::bad_request(message))?;
    Ok(uuid.to_string())
}

// Ensure we don't accidentally return a 200 on a handler that forgot to wrap `AppResult`.
#[allow(dead_code)]
fn _assert_persons_router_state(_: Router<AppState>) {}
