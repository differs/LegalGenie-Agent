use crate::access::ensure_case_access;
use crate::api::ApiEnvelope;
use crate::errors::{AppError, AppResult};
use crate::routes::auth::AuthUser;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    routing::{delete, post, put},
    Json, Router,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/nodes/:id", put(update_node).delete(delete_node))
        .route("/nodes/:id/move", post(move_node))
        .route("/nodes/:id/evidence", post(link_evidence))
        .route("/nodes/:id/evidence/:link_id", delete(unlink_evidence))
}

#[derive(Debug, Deserialize)]
struct UpdateNodeRequest {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    event_time: Option<String>, // YYYY-MM-DD
    #[serde(default)]
    tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct MoveNodeRequest {
    new_time: String, // YYYY-MM-DD
    #[serde(default)]
    new_sort_order: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct LinkEvidenceRequest {
    evidence_id: String,
    anchor_type: String,
    #[serde(default)]
    anchor_data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NodeRow {
    id: String,
    case_id: String,
    title: String,
    description: Option<String>,
    event_time: String,
    sort_order: i64,
    tags: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct EvidenceLink {
    id: String,
    evidence_id: String,
    evidence_name: String,
    anchor_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    anchor_data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct TimelineNode {
    id: String,
    case_id: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    event_time: String,
    sort_order: i64,
    tags: Vec<String>,
    evidence_links: Vec<EvidenceLink>,
    created_at: String,
    updated_at: String,
}

async fn update_node(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<UpdateNodeRequest>,
) -> AppResult<Json<ApiEnvelope<TimelineNode>>> {
    let node_id = normalize_uuid(&id, "invalid node id")?;
    let existing = fetch_node(&state, &user, &node_id).await?;
    let existing_case_id = existing.case_id.clone();
    let existing_event_time = existing.event_time.clone();
    let existing_sort_order = existing.sort_order;
    let existing_tags = existing.tags.clone();
    let existing_description = existing.description.clone();

    let new_title = req
        .title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&existing.title)
        .to_string();

    let new_description = req.description.or(existing_description);

    let new_event_time = match req.event_time {
        Some(raw) => {
            let d = parse_date(&raw)?;
            d.to_string()
        }
        None => existing_event_time.clone(),
    };

    let new_tags_json = match req.tags {
        Some(tags) => {
            if tags.is_empty() {
                None
            } else {
                Some(
                    serde_json::to_string(&tags)
                        .map_err(|e| AppError::bad_request(format!("invalid tags: {e}")))?,
                )
            }
        }
        None => existing_tags,
    };

    // If the event_time changes, append to the end of that date's ordering.
    let mut sort_order = existing_sort_order;
    if new_event_time != existing_event_time {
        let next_sort: (i64,) = sqlx::query_as(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM event_nodes WHERE case_id = ?1 AND event_time = ?2 AND status != 'deleted'",
        )
        .bind(&existing_case_id)
        .bind(&new_event_time)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;
        sort_order = next_sort.0;
    }

    sqlx::query(
        r#"
        UPDATE event_nodes
        SET title = ?1, description = ?2, event_time = ?3, sort_order = ?4, tags = ?5, updated_at = CURRENT_TIMESTAMP
        WHERE id = ?6 AND status != 'deleted'
        "#,
    )
    .bind(&new_title)
    .bind(&new_description)
    .bind(&new_event_time)
    .bind(sort_order)
    .bind(&new_tags_json)
    .bind(&node_id)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let node = fetch_node_with_links(&state, &user, &node_id).await?;
    Ok(Json(ApiEnvelope::ok(node)))
}

async fn move_node(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<MoveNodeRequest>,
) -> AppResult<Json<ApiEnvelope<TimelineNode>>> {
    let node_id = normalize_uuid(&id, "invalid node id")?;
    let existing = fetch_node(&state, &user, &node_id).await?;

    let new_time = parse_date(&req.new_time)?.to_string();
    let new_sort_order = match req.new_sort_order {
        Some(v) => v,
        None => {
            let next_sort: (i64,) = sqlx::query_as(
                "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM event_nodes WHERE case_id = ?1 AND event_time = ?2 AND status != 'deleted'",
            )
            .bind(&existing.case_id)
            .bind(&new_time)
            .fetch_one(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;
            next_sort.0
        }
    };

    sqlx::query(
        "UPDATE event_nodes SET event_time = ?1, sort_order = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?3 AND status != 'deleted'",
    )
    .bind(&new_time)
    .bind(new_sort_order)
    .bind(&node_id)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let node = fetch_node_with_links(&state, &user, &node_id).await?;
    Ok(Json(ApiEnvelope::ok(node)))
}

async fn delete_node(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiEnvelope<serde_json::Value>>> {
    let node_id = normalize_uuid(&id, "invalid node id")?;
    let existing = fetch_node(&state, &user, &node_id).await?;

    sqlx::query(
        "UPDATE event_nodes SET status = 'deleted', updated_at = CURRENT_TIMESTAMP WHERE id = ?1 AND status != 'deleted'",
    )
    .bind(&existing.id)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    Ok(Json(ApiEnvelope::ok(serde_json::json!({}))))
}

async fn link_evidence(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<LinkEvidenceRequest>,
) -> AppResult<Json<ApiEnvelope<EvidenceLink>>> {
    let node_id = normalize_uuid(&id, "invalid node id")?;
    let node = fetch_node(&state, &user, &node_id).await?;

    let evidence_id = normalize_uuid(&req.evidence_id, "invalid evidence id")?;

    let evidence: Option<(String, String)> = sqlx::query_as(
        "SELECT case_id, original_name FROM evidence_files WHERE id = ?1 AND status != 'deleted' LIMIT 1",
    )
    .bind(&evidence_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some((e_case_id, evidence_name)) = evidence else {
        return Err(AppError::not_found("evidence not found"));
    };

    if e_case_id != node.case_id {
        return Err(AppError::bad_request("evidence must belong to same case"));
    }

    let anchor_type = req.anchor_type.trim().to_string();
    let anchor_data_str = req
        .anchor_data
        .as_ref()
        .map(|v| serde_json::to_string(v))
        .transpose()
        .map_err(|e| AppError::bad_request(format!("invalid anchor_data: {e}")))?;

    let link_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO node_evidence_links (id, node_id, evidence_id, anchor_type, anchor_data)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind(link_id.to_string())
    .bind(&node_id)
    .bind(&evidence_id)
    .bind(&anchor_type)
    .bind(anchor_data_str.clone())
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    Ok(Json(ApiEnvelope::ok(EvidenceLink {
        id: link_id.to_string(),
        evidence_id,
        evidence_name,
        anchor_type,
        anchor_data: anchor_data_str
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok()),
    })))
}

async fn unlink_evidence(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, link_id)): Path<(String, String)>,
) -> AppResult<Json<ApiEnvelope<serde_json::Value>>> {
    let node_id = normalize_uuid(&id, "invalid node id")?;
    let link_id = normalize_uuid(&link_id, "invalid link id")?;
    let _node = fetch_node(&state, &user, &node_id).await?;

    let deleted = sqlx::query("DELETE FROM node_evidence_links WHERE id = ?1 AND node_id = ?2")
        .bind(&link_id)
        .bind(&node_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    if deleted.rows_affected() == 0 {
        return Err(AppError::not_found("link not found"));
    }

    Ok(Json(ApiEnvelope::ok(serde_json::json!({}))))
}

async fn fetch_node(state: &AppState, user: &AuthUser, node_id: &str) -> AppResult<NodeRow> {
    let row: Option<NodeRow> = sqlx::query_as(
        r#"
        SELECT id, case_id, title, description, event_time, sort_order, tags, status, created_at, updated_at
        FROM event_nodes
        WHERE id = ?1 AND status != 'deleted'
        LIMIT 1
        "#,
    )
    .bind(node_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some(row) = row else {
        return Err(AppError::not_found("node not found"));
    };
    ensure_case_access(&state.pool, user.user_id, &row.case_id).await?;
    Ok(row)
}

async fn fetch_node_with_links(
    state: &AppState,
    user: &AuthUser,
    node_id: &str,
) -> AppResult<TimelineNode> {
    let node = fetch_node(state, user, node_id).await?;

    let link_rows: Vec<(String, String, String, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT
            l.id,
            l.evidence_id,
            e.original_name AS evidence_name,
            l.anchor_type,
            l.anchor_data
        FROM node_evidence_links l
        JOIN evidence_files e ON e.id = l.evidence_id
        WHERE l.node_id = ?1 AND e.status != 'deleted'
        ORDER BY l.created_at ASC
        "#,
    )
    .bind(&node.id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let evidence_links = link_rows
        .into_iter()
        .map(
            |(id, evidence_id, evidence_name, anchor_type, anchor_data)| EvidenceLink {
                id,
                evidence_id,
                evidence_name,
                anchor_type,
                anchor_data: anchor_data
                    .as_deref()
                    .and_then(|raw| serde_json::from_str(raw).ok()),
            },
        )
        .collect::<Vec<_>>();

    Ok(TimelineNode {
        id: node.id,
        case_id: node.case_id,
        title: node.title,
        description: node.description,
        event_time: node.event_time,
        sort_order: node.sort_order,
        tags: node
            .tags
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
            .unwrap_or_default(),
        evidence_links,
        created_at: node.created_at,
        updated_at: node.updated_at,
    })
}

fn normalize_uuid(raw: &str, message: &str) -> AppResult<String> {
    let uuid = Uuid::parse_str(raw).map_err(|_| AppError::bad_request(message))?;
    Ok(uuid.to_string())
}

fn parse_date(raw: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map_err(|_| AppError::bad_request("invalid date (expected YYYY-MM-DD)"))
}

// Ensure we don't accidentally return a 200 on a handler that forgot to wrap `AppResult`.
#[allow(dead_code)]
fn _assert_timeline_router_state(_: Router<AppState>) {}
