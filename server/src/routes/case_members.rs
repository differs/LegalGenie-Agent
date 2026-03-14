use crate::access::{ensure_case_access, ensure_case_owner};
use crate::api::ApiEnvelope;
use crate::errors::{AppError, AppResult};
use crate::routes::auth::AuthUser;
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    routing::{delete, get},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/:case_id/members", get(list_members).post(add_member))
        .route("/:case_id/members/:user_id", delete(remove_member))
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
struct AddMemberRequest {
    user_id: String,
    #[serde(default)]
    role_in_case: Option<String>, // member/viewer
}

#[derive(Debug, Serialize)]
struct CaseMember {
    user_id: String,
    username: String,
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    real_name: Option<String>,
    role_in_case: String,
    joined_at: String,
    joined_by: String,
}

#[derive(Debug, Serialize)]
struct CaseMemberListData {
    members: Vec<CaseMember>,
    total: i64,
    page: i64,
    page_size: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct MemberRow {
    user_id: String,
    username: String,
    email: String,
    real_name: Option<String>,
    role_in_case: String,
    joined_at: String,
    joined_by: String,
}

async fn list_members(
    State(state): State<AppState>,
    user: AuthUser,
    Path(case_id): Path<String>,
    Query(p): Query<Pagination>,
) -> AppResult<Json<ApiEnvelope<CaseMemberListData>>> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    let page = p.page.max(1);
    let page_size = p.page_size.clamp(1, 200);
    let offset = (page - 1) * page_size;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(1) FROM case_members WHERE case_id = ?1")
        .bind(&case_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let rows: Vec<MemberRow> = sqlx::query_as(
        r#"
        SELECT
            m.user_id,
            u.username,
            u.email,
            u.real_name,
            m.role_in_case,
            m.joined_at,
            m.joined_by
        FROM case_members m
        JOIN users u ON u.id = m.user_id
        WHERE m.case_id = ?1
        ORDER BY
            CASE m.role_in_case
                WHEN 'owner' THEN 0
                WHEN 'member' THEN 1
                ELSE 2
            END,
            m.joined_at ASC
        LIMIT ?2 OFFSET ?3
        "#,
    )
    .bind(&case_id)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let members = rows
        .into_iter()
        .map(|r| CaseMember {
            user_id: r.user_id,
            username: r.username,
            email: r.email,
            real_name: r.real_name,
            role_in_case: r.role_in_case,
            joined_at: r.joined_at,
            joined_by: r.joined_by,
        })
        .collect::<Vec<_>>();

    Ok(Json(ApiEnvelope::ok(CaseMemberListData {
        members,
        total: total.0,
        page,
        page_size,
    })))
}

async fn add_member(
    State(state): State<AppState>,
    user: AuthUser,
    Path(case_id): Path<String>,
    Json(req): Json<AddMemberRequest>,
) -> AppResult<Json<ApiEnvelope<CaseMember>>> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    ensure_case_owner(&state.pool, user.user_id, &case_id).await?;

    let member_user_id = normalize_uuid(&req.user_id, "invalid user_id")?;

    // Ensure user exists and is active.
    let user_row: Option<(String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT username, email, real_name, status FROM users WHERE id = ?1 LIMIT 1",
    )
    .bind(&member_user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some((username, email, real_name, status)) = user_row else {
        return Err(AppError::not_found("user not found"));
    };
    if status != "active" {
        return Err(AppError::bad_request("user inactive"));
    }

    // The owner is always a member; do not allow changing their role via this endpoint.
    if member_user_id == user.user_id.to_string() {
        sqlx::query(
            "INSERT OR IGNORE INTO case_members (id, case_id, user_id, role_in_case, joined_by) VALUES (?1, ?2, ?3, 'owner', ?3)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&case_id)
        .bind(&member_user_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        let row: Option<(String, String, String)> = sqlx::query_as(
            "SELECT role_in_case, joined_at, joined_by FROM case_members WHERE case_id = ?1 AND user_id = ?2 LIMIT 1",
        )
        .bind(&case_id)
        .bind(&member_user_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        let Some((role_in_case, joined_at, joined_by)) = row else {
            return Err(AppError::internal("owner membership not found"));
        };

        return Ok(Json(ApiEnvelope::ok(CaseMember {
            user_id: member_user_id,
            username,
            email,
            real_name,
            role_in_case,
            joined_at,
            joined_by,
        })));
    }

    let role = req
        .role_in_case
        .unwrap_or_else(|| "member".to_string())
        .trim()
        .to_ascii_lowercase();

    match role.as_str() {
        "member" | "viewer" => {}
        "owner" => return Err(AppError::bad_request("role_in_case cannot be owner")),
        _ => {
            return Err(AppError::bad_request(
                "invalid role_in_case (member/viewer)",
            ))
        }
    }

    let new_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO case_members (id, case_id, user_id, role_in_case, joined_by)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(case_id, user_id)
        DO UPDATE SET
          role_in_case = excluded.role_in_case,
          joined_by = excluded.joined_by
        "#,
    )
    .bind(new_id.to_string())
    .bind(&case_id)
    .bind(&member_user_id)
    .bind(&role)
    .bind(user.user_id.to_string())
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT joined_at, joined_by FROM case_members WHERE case_id = ?1 AND user_id = ?2 LIMIT 1",
    )
    .bind(&case_id)
    .bind(&member_user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some((joined_at, joined_by)) = row else {
        return Err(AppError::internal("member upserted but not found"));
    };

    Ok(Json(ApiEnvelope::ok(CaseMember {
        user_id: member_user_id,
        username,
        email,
        real_name,
        role_in_case: role,
        joined_at,
        joined_by,
    })))
}

async fn remove_member(
    State(state): State<AppState>,
    user: AuthUser,
    Path((case_id, user_id)): Path<(String, String)>,
) -> AppResult<Json<ApiEnvelope<serde_json::Value>>> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    ensure_case_owner(&state.pool, user.user_id, &case_id).await?;

    let member_user_id = normalize_uuid(&user_id, "invalid user_id")?;
    if member_user_id == user.user_id.to_string() {
        return Err(AppError::bad_request("cannot remove case owner"));
    }

    let deleted = sqlx::query("DELETE FROM case_members WHERE case_id = ?1 AND user_id = ?2")
        .bind(&case_id)
        .bind(&member_user_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    if deleted.rows_affected() == 0 {
        return Err(AppError::not_found("member not found"));
    }

    Ok(Json(ApiEnvelope::ok(serde_json::json!({}))))
}

fn normalize_uuid(raw: &str, message: &str) -> AppResult<String> {
    let uuid = Uuid::parse_str(raw).map_err(|_| AppError::bad_request(message))?;
    Ok(uuid.to_string())
}

// Ensure we don't accidentally return a 200 on a handler that forgot to wrap `AppResult`.
#[allow(dead_code)]
fn _assert_case_members_router_state(_: Router<AppState>) {}
