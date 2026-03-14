use crate::access::ensure_case_access;
use crate::api::ApiEnvelope;
use crate::errors::{AppError, AppResult};
use crate::routes::auth::AuthUser;
use crate::state::AppState;
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(global_search))
        .route("/cases", get(search_cases_only))
        .route("/evidence", get(search_evidence_only))
        .route("/nodes", get(search_nodes_only))
        .route("/suggestions", get(suggestions))
        .route("/history", get(history).delete(clear_history))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    keyword: String,
    #[serde(default)]
    object_types: Option<String>,
    #[serde(default)]
    case_id: Option<String>,
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
struct SuggestionsQuery {
    keyword: String,
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_page_size")]
    page_size: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchObjectType {
    Case,
    Evidence,
    Node,
}

impl SearchObjectType {
    fn as_str(self) -> &'static str {
        match self {
            SearchObjectType::Case => "case",
            SearchObjectType::Evidence => "evidence",
            SearchObjectType::Node => "node",
        }
    }
}

impl std::str::FromStr for SearchObjectType {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_ascii_lowercase();
        match s.as_str() {
            "case" | "cases" => Ok(SearchObjectType::Case),
            "evidence" | "file" | "files" => Ok(SearchObjectType::Evidence),
            "node" | "nodes" | "timeline" => Ok(SearchObjectType::Node),
            _ => Err(AppError::bad_request(format!("invalid object_type: {s}"))),
        }
    }
}

#[derive(Debug, Serialize)]
struct SearchResult {
    object_type: String,
    object_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    case_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    case_name: Option<String>,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    highlight: Option<String>,
    tags: Vec<String>,
    created_at: String,
    score: f64,
}

#[derive(Debug, Serialize)]
struct SearchResponseData {
    results: Vec<SearchResult>,
    total: i64,
    page: i64,
    page_size: i64,
    suggestions: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SuggestionsData {
    suggestions: Vec<String>,
}

#[derive(Debug, Serialize)]
struct HistoryItem {
    id: String,
    keyword: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    object_types: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    case_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result_count: Option<i64>,
    searched_at: String,
}

#[derive(Debug, Serialize)]
struct HistoryData {
    items: Vec<HistoryItem>,
    total: i64,
    page: i64,
    page_size: i64,
}

#[derive(Debug)]
struct SearchParams {
    keyword_raw: String,
    fts_query: String,
    object_types: Vec<SearchObjectType>,
    case_id: Option<String>,
    page: i64,
    page_size: i64,
}

async fn global_search(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<SearchQuery>,
) -> AppResult<Json<ApiEnvelope<SearchResponseData>>> {
    let params = parse_search_params(&state, &user, q).await?;
    let data = perform_search(&state, &user, params, true).await?;
    Ok(Json(ApiEnvelope::ok(data)))
}

async fn search_cases_only(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<SearchQuery>,
) -> AppResult<Json<ApiEnvelope<SearchResponseData>>> {
    let mut params = parse_search_params(&state, &user, q).await?;
    params.object_types = vec![SearchObjectType::Case];
    let data = perform_search(&state, &user, params, true).await?;
    Ok(Json(ApiEnvelope::ok(data)))
}

async fn search_evidence_only(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<SearchQuery>,
) -> AppResult<Json<ApiEnvelope<SearchResponseData>>> {
    let mut params = parse_search_params(&state, &user, q).await?;
    params.object_types = vec![SearchObjectType::Evidence];
    let data = perform_search(&state, &user, params, true).await?;
    Ok(Json(ApiEnvelope::ok(data)))
}

async fn search_nodes_only(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<SearchQuery>,
) -> AppResult<Json<ApiEnvelope<SearchResponseData>>> {
    let mut params = parse_search_params(&state, &user, q).await?;
    params.object_types = vec![SearchObjectType::Node];
    let data = perform_search(&state, &user, params, true).await?;
    Ok(Json(ApiEnvelope::ok(data)))
}

async fn suggestions(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<SuggestionsQuery>,
) -> AppResult<Json<ApiEnvelope<SuggestionsData>>> {
    let kw = q.keyword.trim();
    if kw.len() < 2 {
        return Ok(Json(ApiEnvelope::ok(SuggestionsData {
            suggestions: Vec::new(),
        })));
    }

    let suggestions = get_suggestions(&state, user.user_id, kw).await?;
    Ok(Json(ApiEnvelope::ok(SuggestionsData { suggestions })))
}

async fn history(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<HistoryQuery>,
) -> AppResult<Json<ApiEnvelope<HistoryData>>> {
    let page = q.page.max(1);
    let page_size = q.page_size.clamp(1, 200);
    let offset = (page - 1) * page_size;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(1) FROM search_histories WHERE user_id = ?1")
        .bind(user.user_id.to_string())
        .fetch_one(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let rows: Vec<(
        String,
        String,
        Option<String>,
        Option<String>,
        Option<i64>,
        String,
    )> = sqlx::query_as(
        r#"
            SELECT id, keyword, object_types, case_id, result_count, searched_at
            FROM search_histories
            WHERE user_id = ?1
            ORDER BY searched_at DESC
            LIMIT ?2 OFFSET ?3
            "#,
    )
    .bind(user.user_id.to_string())
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let items = rows
        .into_iter()
        .map(
            |(id, keyword, object_types, case_id, result_count, searched_at)| HistoryItem {
                id,
                keyword,
                object_types,
                case_id,
                result_count,
                searched_at,
            },
        )
        .collect::<Vec<_>>();

    Ok(Json(ApiEnvelope::ok(HistoryData {
        items,
        total: total.0,
        page,
        page_size,
    })))
}

async fn clear_history(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Json<ApiEnvelope<serde_json::Value>>> {
    sqlx::query("DELETE FROM search_histories WHERE user_id = ?1")
        .bind(user.user_id.to_string())
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    Ok(Json(ApiEnvelope::ok(serde_json::json!({}))))
}

async fn parse_search_params(
    state: &AppState,
    user: &AuthUser,
    q: SearchQuery,
) -> AppResult<SearchParams> {
    let keyword_raw = q.keyword.trim().to_string();
    if keyword_raw.is_empty() {
        return Err(AppError::bad_request("keyword required"));
    }

    let object_types = match q.object_types {
        Some(raw) => {
            let mut types = Vec::new();
            for part in raw.split(',') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                types.push(part.parse::<SearchObjectType>()?);
            }
            if types.is_empty() {
                vec![
                    SearchObjectType::Case,
                    SearchObjectType::Evidence,
                    SearchObjectType::Node,
                ]
            } else {
                types
            }
        }
        None => vec![
            SearchObjectType::Case,
            SearchObjectType::Evidence,
            SearchObjectType::Node,
        ],
    };

    let case_id = match q.case_id {
        Some(raw) => {
            let case_id = normalize_uuid(&raw, "invalid case_id")?;
            ensure_case_access(&state.pool, user.user_id, &case_id).await?;
            Some(case_id)
        }
        None => None,
    };

    let page = q.page.max(1);
    let page_size = q.page_size.clamp(1, 200);
    let fts_query = build_fts_query(&keyword_raw)?;

    Ok(SearchParams {
        keyword_raw,
        fts_query,
        object_types,
        case_id,
        page,
        page_size,
    })
}

async fn perform_search(
    state: &AppState,
    user: &AuthUser,
    params: SearchParams,
    record_history: bool,
) -> AppResult<SearchResponseData> {
    let limit_per_type = params.page * params.page_size;
    let offset = (params.page - 1) * params.page_size;

    let mut results = Vec::new();
    let mut total = 0_i64;

    if params.object_types.contains(&SearchObjectType::Case) {
        let (mut r, t) = search_cases(
            state,
            user,
            &params.fts_query,
            params.case_id.as_deref(),
            limit_per_type,
        )
        .await?;
        results.append(&mut r);
        total += t;
    }

    if params.object_types.contains(&SearchObjectType::Evidence) {
        let (mut r, t) = search_evidence(
            state,
            user,
            &params.fts_query,
            params.case_id.as_deref(),
            limit_per_type,
        )
        .await?;
        results.append(&mut r);
        total += t;
    }

    if params.object_types.contains(&SearchObjectType::Node) {
        let (mut r, t) = search_nodes(
            state,
            user,
            &params.fts_query,
            params.case_id.as_deref(),
            limit_per_type,
        )
        .await?;
        results.append(&mut r);
        total += t;
    }

    // Order by score desc, then created_at desc as tie-breaker.
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.created_at.cmp(&a.created_at))
    });

    let paged = results
        .into_iter()
        .skip(offset as usize)
        .take(params.page_size as usize)
        .collect::<Vec<_>>();

    if record_history {
        record_search_history(
            state,
            user,
            &params.keyword_raw,
            &params.object_types,
            params.case_id.as_deref(),
            total,
        )
        .await?;
        bump_hot_search(state, user.user_id, &params.keyword_raw).await?;
    }

    let suggestions = if params.keyword_raw.len() >= 2 {
        get_suggestions(state, user.user_id, &params.keyword_raw).await?
    } else {
        Vec::new()
    };

    Ok(SearchResponseData {
        results: paged,
        total,
        page: params.page,
        page_size: params.page_size,
        suggestions,
    })
}

async fn search_cases(
    state: &AppState,
    user: &AuthUser,
    fts_query: &str,
    case_id: Option<&str>,
    limit: i64,
) -> AppResult<(Vec<SearchResult>, i64)> {
    let count_sql = if case_id.is_some() {
        r#"
        SELECT COUNT(1)
        FROM search_cases
        JOIN cases c ON search_cases.rowid = c.rowid
        WHERE search_cases MATCH ?1
          AND c.status != 'deleted'
          AND c.id = ?2
          AND (c.owner_id = ?3 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = ?3
          ))
        "#
    } else {
        r#"
        SELECT COUNT(1)
        FROM search_cases
        JOIN cases c ON search_cases.rowid = c.rowid
        WHERE search_cases MATCH ?1
          AND c.status != 'deleted'
          AND (c.owner_id = ?2 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = ?2
          ))
        "#
    };

    let total: (i64,) = if let Some(case_id) = case_id {
        sqlx::query_as(count_sql)
            .bind(fts_query)
            .bind(case_id)
            .bind(user.user_id.to_string())
            .fetch_one(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    } else {
        sqlx::query_as(count_sql)
            .bind(fts_query)
            .bind(user.user_id.to_string())
            .fetch_one(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    };

    let list_sql = if case_id.is_some() {
        r#"
        SELECT
            c.id AS object_id,
            c.name AS title,
            c.description AS content,
            snippet(search_cases, 0, '<em>', '</em>', '...', 10) AS highlight,
            c.tags AS tags,
            c.created_at AS created_at,
            (-bm25(search_cases)) AS score
        FROM search_cases
        JOIN cases c ON search_cases.rowid = c.rowid
        WHERE search_cases MATCH ?1
          AND c.status != 'deleted'
          AND c.id = ?2
          AND (c.owner_id = ?3 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = ?3
          ))
        ORDER BY score DESC
        LIMIT ?4
        "#
    } else {
        r#"
        SELECT
            c.id AS object_id,
            c.name AS title,
            c.description AS content,
            snippet(search_cases, 0, '<em>', '</em>', '...', 10) AS highlight,
            c.tags AS tags,
            c.created_at AS created_at,
            (-bm25(search_cases)) AS score
        FROM search_cases
        JOIN cases c ON search_cases.rowid = c.rowid
        WHERE search_cases MATCH ?1
          AND c.status != 'deleted'
          AND (c.owner_id = ?2 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = ?2
          ))
        ORDER BY score DESC
        LIMIT ?3
        "#
    };

    let rows: Vec<(
        String,
        String,
        Option<String>,
        String,
        Option<String>,
        String,
        f64,
    )> = if let Some(case_id) = case_id {
        sqlx::query_as(list_sql)
            .bind(fts_query)
            .bind(case_id)
            .bind(user.user_id.to_string())
            .bind(limit)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    } else {
        sqlx::query_as(list_sql)
            .bind(fts_query)
            .bind(user.user_id.to_string())
            .bind(limit)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    };

    let out = rows
        .into_iter()
        .map(
            |(object_id, title, content, highlight, tags_raw, created_at, score)| SearchResult {
                object_type: SearchObjectType::Case.as_str().to_string(),
                object_id,
                case_id: None,
                case_name: None,
                title,
                content,
                highlight: Some(highlight),
                tags: tags_raw
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
                    .unwrap_or_default(),
                created_at,
                score,
            },
        )
        .collect::<Vec<_>>();

    Ok((out, total.0))
}

async fn search_evidence(
    state: &AppState,
    user: &AuthUser,
    fts_query: &str,
    case_id: Option<&str>,
    limit: i64,
) -> AppResult<(Vec<SearchResult>, i64)> {
    let count_sql = if case_id.is_some() {
        r#"
        SELECT COUNT(1)
        FROM search_evidence
        JOIN evidence_files e ON search_evidence.rowid = e.rowid
        JOIN cases c ON c.id = e.case_id
        WHERE search_evidence MATCH ?1
          AND e.status != 'deleted'
          AND c.status != 'deleted'
          AND e.case_id = ?2
          AND (c.owner_id = ?3 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = ?3
          ))
        "#
    } else {
        r#"
        SELECT COUNT(1)
        FROM search_evidence
        JOIN evidence_files e ON search_evidence.rowid = e.rowid
        JOIN cases c ON c.id = e.case_id
        WHERE search_evidence MATCH ?1
          AND e.status != 'deleted'
          AND c.status != 'deleted'
          AND (c.owner_id = ?2 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = ?2
          ))
        "#
    };

    let total: (i64,) = if let Some(case_id) = case_id {
        sqlx::query_as(count_sql)
            .bind(fts_query)
            .bind(case_id)
            .bind(user.user_id.to_string())
            .fetch_one(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    } else {
        sqlx::query_as(count_sql)
            .bind(fts_query)
            .bind(user.user_id.to_string())
            .fetch_one(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    };

    let list_sql = if case_id.is_some() {
        r#"
        SELECT
            e.id AS object_id,
            e.case_id AS case_id,
            c.name AS case_name,
            e.original_name AS title,
            substr(e.parsed_text, 1, 200) AS content,
            snippet(search_evidence, 0, '<em>', '</em>', '...', 10) AS highlight,
            e.created_at AS created_at,
            (-bm25(search_evidence)) AS score
        FROM search_evidence
        JOIN evidence_files e ON search_evidence.rowid = e.rowid
        JOIN cases c ON c.id = e.case_id
        WHERE search_evidence MATCH ?1
          AND e.status != 'deleted'
          AND c.status != 'deleted'
          AND e.case_id = ?2
          AND (c.owner_id = ?3 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = ?3
          ))
        ORDER BY score DESC
        LIMIT ?4
        "#
    } else {
        r#"
        SELECT
            e.id AS object_id,
            e.case_id AS case_id,
            c.name AS case_name,
            e.original_name AS title,
            substr(e.parsed_text, 1, 200) AS content,
            snippet(search_evidence, 0, '<em>', '</em>', '...', 10) AS highlight,
            e.created_at AS created_at,
            (-bm25(search_evidence)) AS score
        FROM search_evidence
        JOIN evidence_files e ON search_evidence.rowid = e.rowid
        JOIN cases c ON c.id = e.case_id
        WHERE search_evidence MATCH ?1
          AND e.status != 'deleted'
          AND c.status != 'deleted'
          AND (c.owner_id = ?2 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = ?2
          ))
        ORDER BY score DESC
        LIMIT ?3
        "#
    };

    let rows: Vec<(
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        String,
        f64,
    )> = if let Some(case_id) = case_id {
        sqlx::query_as(list_sql)
            .bind(fts_query)
            .bind(case_id)
            .bind(user.user_id.to_string())
            .bind(limit)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    } else {
        sqlx::query_as(list_sql)
            .bind(fts_query)
            .bind(user.user_id.to_string())
            .bind(limit)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    };

    let out = rows
        .into_iter()
        .map(
            |(object_id, case_id, case_name, title, content, highlight, created_at, score)| {
                SearchResult {
                    object_type: SearchObjectType::Evidence.as_str().to_string(),
                    object_id,
                    case_id: Some(case_id),
                    case_name: Some(case_name),
                    title,
                    content,
                    highlight: Some(highlight),
                    tags: Vec::new(),
                    created_at,
                    score,
                }
            },
        )
        .collect::<Vec<_>>();

    Ok((out, total.0))
}

async fn search_nodes(
    state: &AppState,
    user: &AuthUser,
    fts_query: &str,
    case_id: Option<&str>,
    limit: i64,
) -> AppResult<(Vec<SearchResult>, i64)> {
    let count_sql = if case_id.is_some() {
        r#"
        SELECT COUNT(1)
        FROM search_nodes
        JOIN event_nodes n ON search_nodes.rowid = n.rowid
        JOIN cases c ON c.id = n.case_id
        WHERE search_nodes MATCH ?1
          AND n.status != 'deleted'
          AND c.status != 'deleted'
          AND n.case_id = ?2
          AND (c.owner_id = ?3 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = ?3
          ))
        "#
    } else {
        r#"
        SELECT COUNT(1)
        FROM search_nodes
        JOIN event_nodes n ON search_nodes.rowid = n.rowid
        JOIN cases c ON c.id = n.case_id
        WHERE search_nodes MATCH ?1
          AND n.status != 'deleted'
          AND c.status != 'deleted'
          AND (c.owner_id = ?2 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = ?2
          ))
        "#
    };

    let total: (i64,) = if let Some(case_id) = case_id {
        sqlx::query_as(count_sql)
            .bind(fts_query)
            .bind(case_id)
            .bind(user.user_id.to_string())
            .fetch_one(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    } else {
        sqlx::query_as(count_sql)
            .bind(fts_query)
            .bind(user.user_id.to_string())
            .fetch_one(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    };

    let list_sql = if case_id.is_some() {
        r#"
        SELECT
            n.id AS object_id,
            n.case_id AS case_id,
            c.name AS case_name,
            n.title AS title,
            n.description AS content,
            snippet(search_nodes, 0, '<em>', '</em>', '...', 10) AS highlight,
            n.tags AS tags,
            n.created_at AS created_at,
            (-bm25(search_nodes)) AS score
        FROM search_nodes
        JOIN event_nodes n ON search_nodes.rowid = n.rowid
        JOIN cases c ON c.id = n.case_id
        WHERE search_nodes MATCH ?1
          AND n.status != 'deleted'
          AND c.status != 'deleted'
          AND n.case_id = ?2
          AND (c.owner_id = ?3 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = ?3
          ))
        ORDER BY score DESC
        LIMIT ?4
        "#
    } else {
        r#"
        SELECT
            n.id AS object_id,
            n.case_id AS case_id,
            c.name AS case_name,
            n.title AS title,
            n.description AS content,
            snippet(search_nodes, 0, '<em>', '</em>', '...', 10) AS highlight,
            n.tags AS tags,
            n.created_at AS created_at,
            (-bm25(search_nodes)) AS score
        FROM search_nodes
        JOIN event_nodes n ON search_nodes.rowid = n.rowid
        JOIN cases c ON c.id = n.case_id
        WHERE search_nodes MATCH ?1
          AND n.status != 'deleted'
          AND c.status != 'deleted'
          AND (c.owner_id = ?2 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = ?2
          ))
        ORDER BY score DESC
        LIMIT ?3
        "#
    };

    let rows: Vec<(
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        Option<String>,
        String,
        f64,
    )> = if let Some(case_id) = case_id {
        sqlx::query_as(list_sql)
            .bind(fts_query)
            .bind(case_id)
            .bind(user.user_id.to_string())
            .bind(limit)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    } else {
        sqlx::query_as(list_sql)
            .bind(fts_query)
            .bind(user.user_id.to_string())
            .bind(limit)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    };

    let out = rows
        .into_iter()
        .map(
            |(
                object_id,
                case_id,
                case_name,
                title,
                content,
                highlight,
                tags_raw,
                created_at,
                score,
            )| SearchResult {
                object_type: SearchObjectType::Node.as_str().to_string(),
                object_id,
                case_id: Some(case_id),
                case_name: Some(case_name),
                title,
                content,
                highlight: Some(highlight),
                tags: tags_raw
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
                    .unwrap_or_default(),
                created_at,
                score,
            },
        )
        .collect::<Vec<_>>();

    Ok((out, total.0))
}

fn build_fts_query(keyword: &str) -> AppResult<String> {
    // Conservative query building: split on whitespace and prefix-match each term.
    // Double quotes are stripped to avoid invalid query syntax.
    let parts = keyword
        .split_whitespace()
        .map(|p| p.replace('"', ""))
        .filter(|p| !p.is_empty())
        .take(10)
        .collect::<Vec<_>>();

    if parts.is_empty() {
        return Err(AppError::bad_request("keyword required"));
    }

    let mut out = String::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push('"');
        out.push_str(p);
        out.push_str("\"*");
    }
    Ok(out)
}

async fn record_search_history(
    state: &AppState,
    user: &AuthUser,
    keyword: &str,
    object_types: &[SearchObjectType],
    case_id: Option<&str>,
    result_count: i64,
) -> AppResult<()> {
    let object_types = if object_types.is_empty() {
        None
    } else {
        Some(
            object_types
                .iter()
                .map(|t| t.as_str())
                .collect::<Vec<_>>()
                .join(","),
        )
    };

    sqlx::query(
        "INSERT INTO search_histories (id, user_id, keyword, object_types, case_id, result_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(user.user_id.to_string())
    .bind(keyword)
    .bind(object_types)
    .bind(case_id)
    .bind(result_count)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    Ok(())
}

async fn bump_hot_search(state: &AppState, user_id: Uuid, keyword: &str) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO user_hot_searches (user_id, keyword, search_count, last_searched, updated_at)
        VALUES (?1, ?2, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT(user_id, keyword)
        DO UPDATE SET
          search_count = search_count + 1,
          last_searched = CURRENT_TIMESTAMP,
          updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(user_id.to_string())
    .bind(keyword)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;
    Ok(())
}

async fn get_suggestions(state: &AppState, user_id: Uuid, prefix: &str) -> AppResult<Vec<String>> {
    let like = format!("{}%", prefix);
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT keyword FROM user_hot_searches WHERE user_id = ?1 AND keyword LIKE ?2 ORDER BY search_count DESC LIMIT 10",
    )
    .bind(user_id.to_string())
    .bind(like)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    Ok(rows)
}

fn normalize_uuid(raw: &str, message: &str) -> AppResult<String> {
    let uuid = Uuid::parse_str(raw).map_err(|_| AppError::bad_request(message))?;
    Ok(uuid.to_string())
}

// Ensure we don't accidentally return a 200 on a handler that forgot to wrap `AppResult`.
#[allow(dead_code)]
fn _assert_search_router_state(_: Router<AppState>) {}
