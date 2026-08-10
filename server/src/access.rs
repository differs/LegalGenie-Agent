use crate::errors::{AppError, AppResult};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn ensure_case_access(pool: &PgPool, user_id: Uuid, case_id: &str) -> AppResult<()> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT owner_id, status FROM cases WHERE id = $1 LIMIT 1")
            .bind(case_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some((owner_id, status)) = row else {
        return Err(AppError::not_found_code(410101, "case not found"));
    };

    if status == "deleted" {
        return Err(AppError::not_found_code(410101, "case not found"));
    }

    if owner_id == user_id.to_string() {
        return Ok(());
    }

    let member: Option<(i64,)> = sqlx::query_as(
        "SELECT 1::bigint FROM case_members WHERE case_id = $1 AND user_id = $2 LIMIT 1",
    )
    .bind(case_id)
    .bind(user_id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    if member.is_some() {
        Ok(())
    } else {
        Err(AppError::forbidden_code(410102, "no case access"))
    }
}

pub async fn ensure_case_write_access(
    pool: &PgPool,
    user_id: Uuid,
    case_id: &str,
) -> AppResult<()> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT owner_id, status FROM cases WHERE id = $1 LIMIT 1")
            .bind(case_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some((owner_id, status)) = row else {
        return Err(AppError::not_found_code(410101, "case not found"));
    };

    if status == "deleted" {
        return Err(AppError::not_found_code(410101, "case not found"));
    }

    if owner_id == user_id.to_string() {
        return Ok(());
    }

    let member_role: Option<(String,)> = sqlx::query_as(
        "SELECT role_in_case FROM case_members WHERE case_id = $1 AND user_id = $2 LIMIT 1",
    )
    .bind(case_id)
    .bind(user_id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    match member_role.as_ref().map(|(r,)| r.as_str()) {
        Some("owner") | Some("member") => Ok(()),
        Some("viewer") => Err(AppError::forbidden_code(410102, "read-only case access")),
        Some(_) => Err(AppError::forbidden_code(410102, "no case write access")),
        None => Err(AppError::forbidden_code(410102, "no case access")),
    }
}

pub async fn ensure_case_owner(pool: &PgPool, user_id: Uuid, case_id: &str) -> AppResult<()> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT owner_id, status FROM cases WHERE id = $1 LIMIT 1")
            .bind(case_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some((owner_id, status)) = row else {
        return Err(AppError::not_found_code(410101, "case not found"));
    };

    if status == "deleted" {
        return Err(AppError::not_found_code(410101, "case not found"));
    }

    if owner_id == user_id.to_string() {
        Ok(())
    } else {
        Err(AppError::forbidden_code(410102, "only owner allowed"))
    }
}
