//! Agent runtime HTTP endpoints (P2).
//!
//! - sessions: create / list / detail (messages + tool calls)
//! - messages: post user input -> rule engine -> tool pipeline
//! - approvals: pending list + confirm / reject (server-enforced)
use crate::agent::engine;
use crate::agent::tools::{find_tool, ToolContext};
use crate::api::ApiEnvelope;
use crate::context::RequestMeta;
use crate::errors::{AppError, AppResult};
use crate::routes::auth::AuthUser;
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sessions", post(create_session).get(list_sessions))
        .route("/sessions/:id", get(get_session).delete(delete_session))
        .route(
            "/sessions/:id/messages",
            post(post_message).get(list_messages),
        )
        .route("/approvals/pending", get(list_pending_approvals))
        .route("/approvals/:id/confirm", post(confirm_approval))
        .route("/approvals/:id/reject", post(reject_approval))
        .route("/audit/:case_id", get(case_audit_trail))
}

/// Audit replay (P4): full tool-call trail for a case — who requested, who
/// decided, when, and the result. Read-only and access-scoped.
async fn case_audit_trail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(case_id): Path<String>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let case_id = normalize_uuid(&case_id, "invalid case_id")?;
    crate::access::ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    let rows: Vec<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT
            tc.id, tc.tool_name, tc.input, tc.output, tc.error,
            tc.status, tc.danger_level,
            u1.username AS requested_by,
            u2.username AS decided_by
        FROM agent_tool_calls tc
        JOIN agent_sessions s ON s.id = tc.session_id
        LEFT JOIN users u1 ON u1.id = tc.requested_by
        LEFT JOIN users u2 ON u2.id = tc.decided_by
        WHERE s.case_id = $1
        ORDER BY tc.created_at ASC
        "#,
    )
    .bind(&case_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let trail: Vec<Value> = rows
        .into_iter()
        .map(
            |(id, tool, input, output, error, status, danger, requested_by, decided_by)| {
                json!({
                    "id": id,
                    "tool": tool,
                    "input": serde_json::from_str::<Value>(&input).unwrap_or(Value::Null),
                    "output": output.map(|o| serde_json::from_str::<Value>(&o).unwrap_or(Value::Null)),
                    "error": error,
                    "status": status,
                    "danger_level": danger,
                    "requested_by": requested_by,
                    "decided_by": decided_by,
                })
            },
        )
        .collect();

    Ok(Json(ApiEnvelope::ok(
        json!({ "case_id": case_id, "tool_calls": trail }),
    )))
}

// ---------- Sessions ----------

#[derive(Debug, Deserialize)]
struct CreateSessionRequest {
    case_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Serialize)]
struct SessionSummary {
    id: String,
    case_id: Option<String>,
    title: String,
    status: String,
    created_at: String,
    updated_at: String,
}

async fn create_session(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<CreateSessionRequest>,
) -> AppResult<Json<ApiEnvelope<SessionSummary>>> {
    let case_id = match req.case_id {
        Some(raw) => {
            let case_id = normalize_uuid(&raw, "invalid case_id")?;
            crate::access::ensure_case_access(&state.pool, user.user_id, &case_id).await?;
            Some(case_id)
        }
        None => None,
    };

    let id = Uuid::new_v4().to_string();
    let title = req.title.unwrap_or_else(|| "New session".to_string());
    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, case_id, user_id, title)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(&id)
    .bind(&case_id)
    .bind(user.user_id.to_string())
    .bind(&title)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let summary = fetch_session_summary(&state, &user, &id).await?;
    Ok(Json(ApiEnvelope::ok(summary)))
}

async fn list_sessions(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Json<ApiEnvelope<Vec<SessionSummary>>>> {
    let rows: Vec<(String, Option<String>, String, String, String, String)> = sqlx::query_as(
        r#"
        SELECT id, case_id, title, status, created_at, updated_at
        FROM agent_sessions
        WHERE user_id = $1 AND status = 'active'
        ORDER BY updated_at DESC
        LIMIT 50
        "#,
    )
    .bind(user.user_id.to_string())
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let sessions = rows
        .into_iter()
        .map(
            |(id, case_id, title, status, created_at, updated_at)| SessionSummary {
                id,
                case_id,
                title,
                status,
                created_at,
                updated_at,
            },
        )
        .collect();
    Ok(Json(ApiEnvelope::ok(sessions)))
}

async fn fetch_session_summary(
    state: &AppState,
    user: &AuthUser,
    session_id: &str,
) -> AppResult<SessionSummary> {
    let row: Option<(String, Option<String>, String, String, String, String)> = sqlx::query_as(
        r#"
        SELECT id, case_id, title, status, created_at, updated_at
        FROM agent_sessions
        WHERE id = $1 AND user_id = $2 AND status = 'active'
        "#,
    )
    .bind(session_id)
    .bind(user.user_id.to_string())
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some((id, case_id, title, status, created_at, updated_at)) = row else {
        return Err(AppError::not_found_code(480101, "session not found"));
    };
    Ok(SessionSummary {
        id,
        case_id,
        title,
        status,
        created_at,
        updated_at,
    })
}

async fn get_session(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let session_id = normalize_uuid(&id, "invalid session id")?;
    let summary = fetch_session_summary(&state, &user, &session_id).await?;

    let messages: Vec<Value> = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, role, content, created_at FROM agent_messages WHERE session_id = $1 ORDER BY created_at ASC",
    )
    .bind(&session_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?
    .into_iter()
    .map(|(id, role, content, created_at)| {
        json!({ "id": id, "role": role, "content": content, "created_at": created_at })
    })
    .collect();

    let tool_calls: Vec<Value> = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            Option<String>,
            String,
            bool,
            String,
            Option<String>,
        ),
    >(
        r#"
        SELECT id, tool_name, input, output, status, requires_approval, danger_level, error
        FROM agent_tool_calls
        WHERE session_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(&session_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?
    .into_iter()
    .map(
        |(id, tool, input, output, status, approval, danger, error)| {
            json!({
                "id": id, "tool": tool,
                "input": serde_json::from_str::<Value>(&input).unwrap_or(Value::Null),
                "output": output.and_then(|o| serde_json::from_str::<Value>(&o).ok()),
                "status": status, "requires_approval": approval,
                "danger_level": danger, "error": error,
            })
        },
    )
    .collect();

    Ok(Json(ApiEnvelope::ok(json!({
        "session": summary,
        "messages": messages,
        "tool_calls": tool_calls,
    }))))
}

async fn delete_session(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let session_id = normalize_uuid(&id, "invalid session id")?;
    let result = sqlx::query(
        "UPDATE agent_sessions SET status = 'archived', updated_at = utc_text() WHERE id = $1 AND user_id = $2",
    )
    .bind(&session_id)
    .bind(user.user_id.to_string())
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found_code(480101, "session not found"));
    }
    Ok(Json(ApiEnvelope::ok(json!({}))))
}

// ---------- Messages ----------

#[derive(Debug, Deserialize)]
struct PostMessageRequest {
    content: String,
}

#[derive(Debug, Serialize)]
struct PostMessageResponse {
    session: SessionSummary,
    user_message_id: String,
    assistant_text: String,
    tool_calls: Vec<Value>,
    pending_approvals: Vec<Value>,
}

async fn post_message(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Path(id): Path<String>,
    Json(req): Json<PostMessageRequest>,
) -> AppResult<Json<ApiEnvelope<PostMessageResponse>>> {
    let session_id = normalize_uuid(&id, "invalid session id")?;
    let summary = fetch_session_summary(&state, &user, &session_id).await?;
    let content = req.content.trim().to_string();
    if content.is_empty() {
        return Err(AppError::bad_request("content required"));
    }

    let case_id = summary.case_id.clone();
    let run = engine::process_user_message(
        &state,
        &user.user_id.to_string(),
        &user.username,
        case_id.as_deref(),
        &session_id,
        &content,
    )
    .await
    .map_err(|e| AppError::internal(format!("agent error: {e}")))?;

    sqlx::query("UPDATE agent_sessions SET updated_at = utc_text() WHERE id = $1")
        .bind(&session_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let _ = meta;
    Ok(Json(ApiEnvelope::ok(PostMessageResponse {
        session: summary,
        user_message_id: Uuid::new_v4().to_string(),
        assistant_text: run.assistant_text,
        tool_calls: run.tool_calls,
        pending_approvals: run.pending_approvals,
    })))
}

async fn list_messages(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let session_id = normalize_uuid(&id, "invalid session id")?;
    fetch_session_summary(&state, &user, &session_id).await?;
    let messages: Vec<Value> = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, role, content, created_at FROM agent_messages WHERE session_id = $1 ORDER BY created_at ASC",
    )
    .bind(&session_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?
    .into_iter()
    .map(|(id, role, content, created_at)| {
        json!({ "id": id, "role": role, "content": content, "created_at": created_at })
    })
    .collect();
    Ok(Json(ApiEnvelope::ok(json!({ "messages": messages }))))
}

// ---------- Approvals ----------

#[derive(Debug, Deserialize)]
struct PendingApprovalsQuery {
    case_id: Option<String>,
}

async fn list_pending_approvals(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<PendingApprovalsQuery>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let case_id = match q.case_id {
        Some(raw) => Some(normalize_uuid(&raw, "invalid case_id")?),
        None => None,
    };

    let mut sql = String::from(
        r#"
        SELECT
            tc.id, tc.tool_name, tc.input, tc.danger_level,
            s.id AS session_id, s.case_id
        FROM agent_tool_calls tc
        JOIN agent_sessions s ON s.id = tc.session_id
        WHERE tc.status = 'pending' AND tc.requires_approval = TRUE
          AND s.status = 'active'
        "#,
    );
    let mut binds: Vec<Option<String>> = Vec::new();
    if let Some(cid) = &case_id {
        sql.push_str(" AND s.case_id = ");
        sql.push_str("$");
        sql.push_str(&(binds.len() + 1).to_string());
        binds.push(Some(cid.clone()));
    }

    let mut query =
        sqlx::query_as::<_, (String, String, String, String, String, Option<String>)>(&sql);
    for b in binds {
        query = query.bind(b);
    }

    let rows: Vec<(String, String, String, String, String, Option<String>)> = query
        .bind(user.user_id.to_string()) // placeholder for owner/member filter not used; scope below
        .fetch_all(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let mut approvals = Vec::new();
    for (id, tool, input, danger, session_id, row_case_id) in rows {
        let Some(cid) = row_case_id else {
            continue;
        };
        // Server-side scoping: only approvals for cases the user can access.
        if crate::access::ensure_case_access(&state.pool, user.user_id, &cid)
            .await
            .is_err()
        {
            continue;
        }
        approvals.push(json!({
            "id": id, "tool": tool,
            "input": serde_json::from_str::<Value>(&input).unwrap_or(Value::Null),
            "danger_level": danger,
            "session_id": session_id,
            "case_id": cid,
        }));
    }
    Ok(Json(ApiEnvelope::ok(json!({ "approvals": approvals }))))
}

async fn confirm_approval(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    execute_approval(&state, &user, &id, true).await
}

async fn reject_approval(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    execute_approval(&state, &user, &id, false).await
}

async fn execute_approval(
    state: &AppState,
    user: &AuthUser,
    approval_id: &str,
    confirm: bool,
) -> AppResult<Json<ApiEnvelope<Value>>> {
    let approval_id = normalize_uuid(approval_id, "invalid approval id")?;

    let row: Option<(String, String, String, String, Option<String>, String)> = sqlx::query_as(
        r#"
        SELECT tc.tool_name, tc.input, s.case_id, s.id, tc.output, tc.status
        FROM agent_tool_calls tc
        JOIN agent_sessions s ON s.id = tc.session_id
        WHERE tc.id = $1 AND tc.requires_approval = TRUE AND tc.requested_by = $2
        "#,
    )
    .bind(&approval_id)
    .bind(user.user_id.to_string())
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some((tool_name, input_raw, case_id, session_id, _output, status)) = row else {
        return Err(AppError::not_found_code(480102, "approval not found"));
    };
    if status != "pending" {
        return Err(AppError::conflict("approval already decided"));
    }
    let input: Value =
        serde_json::from_str(&input_raw).map_err(|e| AppError::bad_request(format!("{e}")))?;

    if !confirm {
        sqlx::query(
            "UPDATE agent_tool_calls SET status = 'rejected', decided_by = $1, decided_at = utc_text() WHERE id = $2",
        )
        .bind(user.user_id.to_string())
        .bind(&approval_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;
        engine::save_assistant(state, &session_id, "已取消该动作，未做任何写入。")
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;
        return Ok(Json(ApiEnvelope::ok(json!({
            "status": "rejected", "id": approval_id,
        }))));
    }

    let Some(tool) = find_tool(&tool_name) else {
        return Err(AppError::bad_request(format!("unknown tool: {tool_name}")));
    };
    let user_id = user.user_id.to_string();
    let ctx = ToolContext {
        state,
        user_id: &user_id,
        username: &user.username,
        case_id: Some(case_id.as_str()),
    };

    sqlx::query(
        "UPDATE agent_tool_calls SET status = 'approved', decided_by = $1, decided_at = utc_text() WHERE id = $2",
    )
    .bind(user.user_id.to_string())
    .bind(&approval_id)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    match tool.execute(&ctx, input.clone()).await {
        Ok(output) => {
            sqlx::query(
                "UPDATE agent_tool_calls SET status = 'executed', output = $1 WHERE id = $2",
            )
            .bind(output.to_string())
            .bind(&approval_id)
            .execute(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

            let summary = engine::summarize_tool_output(&tool_name, &output);
            engine::save_assistant(state, &session_id, &summary)
                .await
                .map_err(|e| AppError::internal(format!("db error: {e}")))?;

            Ok(Json(ApiEnvelope::ok(json!({
                "status": "executed", "id": approval_id,
                "output": output, "summary": summary,
            }))))
        }
        Err(err) => {
            let redacted = crate::config::redact_sensitive(&err.to_string());
            sqlx::query("UPDATE agent_tool_calls SET status = 'failed', error = $1 WHERE id = $2")
                .bind(redacted)
                .bind(&approval_id)
                .execute(&state.pool)
                .await
                .map_err(|e| AppError::internal(format!("db error: {e}")))?;
            engine::save_assistant(state, &session_id, &format!("工具执行失败：{err}"))
                .await
                .map_err(|e| AppError::internal(format!("db error: {e}")))?;
            Err(AppError::internal(format!("tool execution failed: {err}")))
        }
    }
}

fn normalize_uuid(raw: &str, what: &str) -> AppResult<String> {
    Uuid::parse_str(raw.trim())
        .map(|u| u.to_string())
        .map_err(|_| AppError::bad_request(format!("invalid {what}")))
}
