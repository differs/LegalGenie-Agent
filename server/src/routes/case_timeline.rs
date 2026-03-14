use crate::access::ensure_case_access;
use crate::api::ApiEnvelope;
use crate::errors::{AppError, AppResult};
use crate::routes::auth::AuthUser;
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::QueryBuilder;
use std::collections::HashMap;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/:case_id/timeline/nodes",
        get(list_nodes).post(create_node),
    )
}

#[derive(Debug, Deserialize)]
struct NodeListQuery {
    #[serde(default)]
    start_date: Option<String>,
    #[serde(default)]
    end_date: Option<String>,
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_page_size")]
    page_size: i64,
}

fn default_page() -> i64 {
    1
}

fn default_page_size() -> i64 {
    50
}

#[derive(Debug, Deserialize)]
struct CreateNodeRequest {
    title: String,
    #[serde(default)]
    description: Option<String>,
    event_time: String, // YYYY-MM-DD
    #[serde(default)]
    tags: Option<Vec<String>>,
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

#[derive(Debug, Serialize)]
struct NodeListData {
    nodes: Vec<TimelineNode>,
    total: i64,
    page: i64,
    page_size: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct NodeRow {
    id: String,
    case_id: String,
    title: String,
    description: Option<String>,
    event_time: String,
    sort_order: i64,
    tags: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct LinkRow {
    id: String,
    node_id: String,
    evidence_id: String,
    evidence_name: String,
    anchor_type: String,
    anchor_data: Option<String>,
}

async fn list_nodes(
    State(state): State<AppState>,
    user: AuthUser,
    Path(case_id): Path<String>,
    Query(q): Query<NodeListQuery>,
) -> AppResult<Json<ApiEnvelope<NodeListData>>> {
    let case_id = normalize_case_id(&case_id)?;
    ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    let (start, end) = parse_date_range(q.start_date.as_deref(), q.end_date.as_deref())?;
    let page = q.page.max(1);
    let page_size = q.page_size.clamp(1, 500);
    let offset = (page - 1) * page_size;

    let total: (i64,) = {
        let mut qb = QueryBuilder::new("SELECT COUNT(1) FROM event_nodes WHERE case_id = ");
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

        qb.build_query_as::<(i64,)>()
            .fetch_one(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    };

    let nodes: Vec<NodeRow> = {
        let mut qb = QueryBuilder::new(
            "SELECT id, case_id, title, description, event_time, sort_order, tags, created_at, updated_at FROM event_nodes WHERE case_id = ",
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
        qb.push_bind(page_size);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        qb.build_query_as::<NodeRow>()
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    };

    let node_ids = nodes.iter().map(|n| n.id.clone()).collect::<Vec<_>>();
    let links = fetch_links(&state, &node_ids).await?;
    let mut links_by_node: HashMap<String, Vec<EvidenceLink>> = HashMap::new();
    for l in links {
        links_by_node
            .entry(l.node_id)
            .or_default()
            .push(EvidenceLink {
                id: l.id,
                evidence_id: l.evidence_id,
                evidence_name: l.evidence_name,
                anchor_type: l.anchor_type,
                anchor_data: l
                    .anchor_data
                    .as_deref()
                    .and_then(|raw| serde_json::from_str(raw).ok()),
            });
    }

    let out = nodes
        .into_iter()
        .map(|n| TimelineNode {
            id: n.id.clone(),
            case_id: n.case_id,
            title: n.title,
            description: n.description,
            event_time: n.event_time,
            sort_order: n.sort_order,
            tags: n
                .tags
                .as_deref()
                .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
                .unwrap_or_default(),
            evidence_links: links_by_node.remove(&n.id).unwrap_or_default(),
            created_at: n.created_at,
            updated_at: n.updated_at,
        })
        .collect::<Vec<_>>();

    Ok(Json(ApiEnvelope::ok(NodeListData {
        nodes: out,
        total: total.0,
        page,
        page_size,
    })))
}

async fn create_node(
    State(state): State<AppState>,
    user: AuthUser,
    Path(case_id): Path<String>,
    Json(req): Json<CreateNodeRequest>,
) -> AppResult<Json<ApiEnvelope<TimelineNode>>> {
    let case_id = normalize_case_id(&case_id)?;
    ensure_case_access(&state.pool, user.user_id, &case_id).await?;

    let title = req.title.trim();
    if title.is_empty() {
        return Err(AppError::bad_request("title required"));
    }

    let event_date = parse_date(&req.event_time)?;

    let next_sort: (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM event_nodes WHERE case_id = ?1 AND event_time = ?2 AND status != 'deleted'",
    )
    .bind(&case_id)
    .bind(event_date.to_string())
    .fetch_one(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let node_id = Uuid::new_v4();
    let tags_json = req.tags.unwrap_or_default();
    let tags_json = if tags_json.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(&tags_json)
                .map_err(|e| AppError::bad_request(format!("invalid tags: {e}")))?,
        )
    };

    sqlx::query(
        r#"
        INSERT INTO event_nodes (
            id, case_id, title, description, event_time, sort_order, tags, status, created_by
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', ?8)
        "#,
    )
    .bind(node_id.to_string())
    .bind(&case_id)
    .bind(title)
    .bind(req.description.clone())
    .bind(event_date.to_string())
    .bind(next_sort.0)
    .bind(tags_json.clone())
    .bind(user.user_id.to_string())
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let row: Option<NodeRow> = sqlx::query_as(
        "SELECT id, case_id, title, description, event_time, sort_order, tags, created_at, updated_at FROM event_nodes WHERE id = ?1 LIMIT 1",
    )
    .bind(node_id.to_string())
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some(row) = row else {
        return Err(AppError::internal("node inserted but not found"));
    };

    Ok(Json(ApiEnvelope::ok(TimelineNode {
        id: row.id,
        case_id: row.case_id,
        title: row.title,
        description: row.description,
        event_time: row.event_time,
        sort_order: row.sort_order,
        tags: row
            .tags
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
            .unwrap_or_default(),
        evidence_links: Vec::new(),
        created_at: row.created_at,
        updated_at: row.updated_at,
    })))
}

async fn fetch_links(state: &AppState, node_ids: &[String]) -> AppResult<Vec<LinkRow>> {
    if node_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut qb = QueryBuilder::new(
        r#"
        SELECT
            l.id,
            l.node_id,
            l.evidence_id,
            e.original_name AS evidence_name,
            l.anchor_type,
            l.anchor_data
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

    let query = qb.build_query_as::<LinkRow>();
    query
        .fetch_all(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))
}

fn normalize_case_id(raw: &str) -> AppResult<String> {
    let uuid = Uuid::parse_str(raw).map_err(|_| AppError::bad_request("invalid case_id"))?;
    Ok(uuid.to_string())
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

// Ensure we don't accidentally return a 200 on a handler that forgot to wrap `AppResult`.
#[allow(dead_code)]
fn _assert_case_timeline_router_state(_: Router<AppState>) {}
