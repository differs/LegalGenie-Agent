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
use std::collections::HashMap;
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
    #[serde(default = "default_language_mode")]
    language_mode: String,
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

fn default_language_mode() -> String {
    "zh".to_string()
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
    Person,
}

impl SearchObjectType {
    fn as_str(self) -> &'static str {
        match self {
            SearchObjectType::Case => "case",
            SearchObjectType::Evidence => "evidence",
            SearchObjectType::Node => "node",
            SearchObjectType::Person => "person",
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
            "person" | "persons" => Ok(SearchObjectType::Person),
            _ => Err(AppError::bad_request(format!("invalid object_type: {s}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchLanguageMode {
    Bilingual,
    Source,
    Zh,
}

impl SearchLanguageMode {
    fn as_str(self) -> &'static str {
        match self {
            SearchLanguageMode::Bilingual => "bilingual",
            SearchLanguageMode::Source => "source",
            SearchLanguageMode::Zh => "zh",
        }
    }
}

impl std::str::FromStr for SearchLanguageMode {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "bilingual" => Ok(SearchLanguageMode::Bilingual),
            "source" => Ok(SearchLanguageMode::Source),
            "zh" => Ok(SearchLanguageMode::Zh),
            _ => Err(AppError::bad_request("invalid language_mode")),
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
    language_mode: Option<String>,
    file_id: Option<String>,
    file_name: Option<String>,
    chunk_id: Option<String>,
    chunk_index: Option<i64>,
    display_label: Option<String>,
    anchor_json: Option<serde_json::Value>,
    matched_language: Option<String>,
    snippet_source: Option<String>,
    snippet_translated: Option<String>,
    match_start_offset: Option<i64>,
    match_end_offset: Option<i64>,
    translation_incomplete: Option<bool>,
    source_fallback: Option<bool>,
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
    language_mode: SearchLanguageMode,
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

    let total: (i64,) = sqlx::query_as("SELECT COUNT(1) FROM search_histories WHERE user_id = $1")
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
            WHERE user_id = $1
            ORDER BY searched_at DESC
            LIMIT $2 OFFSET $3
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
    sqlx::query("DELETE FROM search_histories WHERE user_id = $1")
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
                    SearchObjectType::Person,
                ]
            } else {
                types
            }
        }
        None => vec![
            SearchObjectType::Case,
            SearchObjectType::Evidence,
            SearchObjectType::Node,
            SearchObjectType::Person,
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
    let language_mode = q.language_mode.parse::<SearchLanguageMode>()?;

    Ok(SearchParams {
        keyword_raw,
        fts_query,
        object_types,
        case_id,
        language_mode,
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
            &params.keyword_raw,
            &params.fts_query,
            params.language_mode,
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

    if params.object_types.contains(&SearchObjectType::Person) {
        let (mut r, t) = search_persons(
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
        JOIN cases c ON search_cases.rowid = c.id
        WHERE search_cases.searchable ILIKE $1
          AND c.status != 'deleted'
          AND c.id = $2
          AND (c.owner_id = $3 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $3
          ))
        "#
    } else {
        r#"
        SELECT COUNT(1)
        FROM search_cases
        JOIN cases c ON search_cases.rowid = c.id
        WHERE search_cases.searchable ILIKE $1
          AND c.status != 'deleted'
          AND (c.owner_id = $2 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $2
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
            c.name AS highlight,
            c.tags AS tags,
            c.created_at AS created_at,
            similarity(search_cases.searchable, replace($1, '%', '')) AS score
        FROM search_cases
        JOIN cases c ON search_cases.rowid = c.id
        WHERE search_cases.searchable ILIKE $1
          AND c.status != 'deleted'
          AND c.id = $2
          AND (c.owner_id = $3 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $3
          ))
        ORDER BY score DESC
        LIMIT $4
        "#
    } else {
        r#"
        SELECT
            c.id AS object_id,
            c.name AS title,
            c.description AS content,
            c.name AS highlight,
            c.tags AS tags,
            c.created_at AS created_at,
            similarity(search_cases.searchable, replace($1, '%', '')) AS score
        FROM search_cases
        JOIN cases c ON search_cases.rowid = c.id
        WHERE search_cases.searchable ILIKE $1
          AND c.status != 'deleted'
          AND (c.owner_id = $2 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $2
          ))
        ORDER BY score DESC
        LIMIT $3
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
                language_mode: None,
                file_id: None,
                file_name: None,
                chunk_id: None,
                chunk_index: None,
                display_label: None,
                anchor_json: None,
                matched_language: None,
                snippet_source: None,
                snippet_translated: None,
                match_start_offset: None,
                match_end_offset: None,
                translation_incomplete: None,
                source_fallback: None,
            },
        )
        .collect::<Vec<_>>();

    Ok((out, total.0))
}

async fn search_evidence(
    state: &AppState,
    user: &AuthUser,
    keyword_raw: &str,
    fts_query: &str,
    language_mode: SearchLanguageMode,
    case_id: Option<&str>,
    limit: i64,
) -> AppResult<(Vec<SearchResult>, i64)> {
    let branch_limit = limit.max(1).saturating_mul(2);
    let mut rows = Vec::new();
    let total = count_evidence_results(state, user, fts_query, case_id, language_mode).await?;

    rows.extend(list_title_evidence_matches(state, user, fts_query, case_id, branch_limit).await?);

    if matches!(
        language_mode,
        SearchLanguageMode::Source | SearchLanguageMode::Bilingual
    ) {
        rows.extend(
            list_evidence_chunk_matches(
                state,
                user,
                fts_query,
                case_id,
                branch_limit,
                "source_text",
                "source",
            )
            .await?,
        );
    }

    if matches!(
        language_mode,
        SearchLanguageMode::Zh | SearchLanguageMode::Bilingual
    ) {
        rows.extend(
            list_evidence_chunk_matches(
                state,
                user,
                fts_query,
                case_id,
                branch_limit,
                "translated_text",
                "translated",
            )
            .await?,
        );
    }

    if matches!(
        language_mode,
        SearchLanguageMode::Source | SearchLanguageMode::Zh | SearchLanguageMode::Bilingual
    ) {
        rows.extend(
            list_legacy_evidence_matches(state, user, fts_query, case_id, branch_limit).await?,
        );
    }

    let mut deduped = HashMap::new();
    for row in rows {
        let key = evidence_result_key(&row);
        match deduped.get(&key) {
            Some(existing) if should_keep_existing_evidence_row(existing, &row, language_mode) => {}
            _ => {
                deduped.insert(key, row);
            }
        }
    }

    let out = deduped
        .into_values()
        .map(|row| evidence_search_result_from_row(row, language_mode, keyword_raw))
        .collect::<AppResult<Vec<_>>>()?;

    Ok((out, total))
}

async fn count_evidence_results(
    state: &AppState,
    user: &AuthUser,
    fts_query: &str,
    case_id: Option<&str>,
    language_mode: SearchLanguageMode,
) -> AppResult<i64> {
    let user_id = user.user_id.to_string();
    let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new("SELECT COUNT(1) FROM (");
    let mut has_subquery = false;

    push_title_count_subquery(
        &mut builder,
        fts_query,
        case_id,
        &user_id,
        &mut has_subquery,
    );

    match language_mode {
        SearchLanguageMode::Source => push_chunk_count_subquery(
            &mut builder,
            &build_fts_column_query("source_text", fts_query),
            "source_text",
            case_id,
            &user_id,
            &mut has_subquery,
        ),
        SearchLanguageMode::Zh => push_chunk_count_subquery(
            &mut builder,
            &build_fts_column_query("translated_text", fts_query),
            "translated_text",
            case_id,
            &user_id,
            &mut has_subquery,
        ),
        SearchLanguageMode::Bilingual => push_chunk_count_subquery(
            &mut builder,
            &build_bilingual_fts_query(fts_query),
            "",
            case_id,
            &user_id,
            &mut has_subquery,
        ),
    }

    push_legacy_count_subquery(
        &mut builder,
        fts_query,
        case_id,
        &user_id,
        &mut has_subquery,
    );

    builder.push(") unique_results");

    let total: (i64,) = builder
        .build_query_as()
        .fetch_one(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error (evcount): {e}")))?;

    Ok(total.0)
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct EvidenceSearchRow {
    file_id: String,
    case_id: String,
    case_name: String,
    file_name: String,
    chunk_id: Option<String>,
    chunk_index: Option<i64>,
    display_label: Option<String>,
    anchor_json: Option<String>,
    snippet_source: Option<String>,
    snippet_translated: Option<String>,
    created_at: String,
    score: f64,
    translation_incomplete: i64,
    matched_language: String,
    source_fallback: i64,
}

async fn list_title_evidence_matches(
    state: &AppState,
    user: &AuthUser,
    fts_query: &str,
    case_id: Option<&str>,
    limit: i64,
) -> AppResult<Vec<EvidenceSearchRow>> {
    let scoped_query = build_fts_column_query("original_name", fts_query);
    let list_sql = if case_id.is_some() {
        r#"
        SELECT
            e.id AS file_id,
            e.case_id AS case_id,
            c.name AS case_name,
            e.original_name AS file_name,
            NULL AS chunk_id,
            NULL AS chunk_index,
            'file name' AS display_label,
            jsonb_build_object(
                'locator_type', 'file_name',
                'display_label', 'file name'
            )::text AS anchor_json,
            e.original_name AS snippet_source,
            NULL AS snippet_translated,
            e.created_at AS created_at,
            similarity(search_evidence.searchable, replace($1, '%', '')) ::float8 AS score,
            CASE
                WHEN e.translation_status != 'done'
                     OR (e.translated_chunk_count + e.failed_chunk_count) < e.chunk_count
                THEN 1
                ELSE 0
            END::bigint AS translation_incomplete,
            'source' AS matched_language,
            0::bigint AS source_fallback
        FROM search_evidence
        JOIN evidence_files e ON search_evidence.rowid = e.id
        JOIN cases c ON c.id = e.case_id
        WHERE search_evidence.original_name ILIKE $1
          AND e.status != 'deleted'
          AND c.status != 'deleted'
          AND e.case_id = $2
          AND (c.owner_id = $3 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $3
          ))
        ORDER BY score DESC
        LIMIT $4
        "#
    } else {
        r#"
        SELECT
            e.id AS file_id,
            e.case_id AS case_id,
            c.name AS case_name,
            e.original_name AS file_name,
            NULL AS chunk_id,
            NULL AS chunk_index,
            'file name' AS display_label,
            jsonb_build_object(
                'locator_type', 'file_name',
                'display_label', 'file name'
            )::text AS anchor_json,
            e.original_name AS snippet_source,
            NULL AS snippet_translated,
            e.created_at AS created_at,
            similarity(search_evidence.searchable, replace($1, '%', '')) ::float8 AS score,
            CASE
                WHEN e.translation_status != 'done'
                     OR (e.translated_chunk_count + e.failed_chunk_count) < e.chunk_count
                THEN 1
                ELSE 0
            END::bigint AS translation_incomplete,
            'source' AS matched_language,
            0::bigint AS source_fallback
        FROM search_evidence
        JOIN evidence_files e ON search_evidence.rowid = e.id
        JOIN cases c ON c.id = e.case_id
        WHERE search_evidence.original_name ILIKE $1
          AND e.status != 'deleted'
          AND c.status != 'deleted'
          AND (c.owner_id = $2 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $2
          ))
        ORDER BY score DESC
        LIMIT $3
        "#
    };

    let rows = if let Some(case_id) = case_id {
        sqlx::query_as::<_, EvidenceSearchRow>(list_sql)
            .bind(scoped_query)
            .bind(case_id)
            .bind(user.user_id.to_string())
            .bind(limit)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    } else {
        sqlx::query_as::<_, EvidenceSearchRow>(list_sql)
            .bind(scoped_query)
            .bind(user.user_id.to_string())
            .bind(limit)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    };

    Ok(rows)
}

async fn list_evidence_chunk_matches(
    state: &AppState,
    user: &AuthUser,
    fts_query: &str,
    case_id: Option<&str>,
    limit: i64,
    column_name: &str,
    matched_language: &str,
) -> AppResult<Vec<EvidenceSearchRow>> {
    let scoped_query = build_fts_column_query(column_name, fts_query);
    let chunk_col = if column_name.contains("translated") {
        "searchable_translated"
    } else if column_name.contains("source") {
        "searchable_source"
    } else {
        "searchable"
    };
    let list_sql = if case_id.is_some() {
        format!(
            r#"
        SELECT
            e.id AS file_id,
            e.case_id AS case_id,
            c.name AS case_name,
            e.original_name AS file_name,
            ch.id AS chunk_id,
            ch.chunk_index AS chunk_index,
            ch.display_label AS display_label,
            ch.anchor_json AS anchor_json,
            ch.source_text AS snippet_source,
            ch.translated_text AS snippet_translated,
            e.created_at AS created_at,
            similarity(search_evidence_chunks.searchable, replace($1, '%', '')) ::float8 AS score,
            CASE
                WHEN e.translation_status != 'done'
                     OR (e.translated_chunk_count + e.failed_chunk_count) < e.chunk_count
                THEN 1
                ELSE 0
            END::bigint AS translation_incomplete,
            $4 AS matched_language,
            0::bigint AS source_fallback
        FROM search_evidence_chunks
        JOIN evidence_file_chunks ch ON search_evidence_chunks.rowid = ch.id
        JOIN evidence_files e ON e.id = ch.evidence_id
        JOIN cases c ON c.id = e.case_id
        WHERE search_evidence_chunks.{chunk_col} ILIKE $1
          AND e.status != 'deleted'
          AND c.status != 'deleted'
          AND e.case_id = $2
          AND (c.owner_id = $3 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $3
          ))
        ORDER BY score DESC
        LIMIT $5
        "#,
            chunk_col = chunk_col
        )
    } else {
        format!(
            r#"
        SELECT
            e.id AS file_id,
            e.case_id AS case_id,
            c.name AS case_name,
            e.original_name AS file_name,
            ch.id AS chunk_id,
            ch.chunk_index AS chunk_index,
            ch.display_label AS display_label,
            ch.anchor_json AS anchor_json,
            ch.source_text AS snippet_source,
            ch.translated_text AS snippet_translated,
            e.created_at AS created_at,
            similarity(search_evidence_chunks.searchable, replace($1, '%', '')) ::float8 AS score,
            CASE
                WHEN e.translation_status != 'done'
                     OR (e.translated_chunk_count + e.failed_chunk_count) < e.chunk_count
                THEN 1
                ELSE 0
            END::bigint AS translation_incomplete,
            $3 AS matched_language,
            0::bigint AS source_fallback
        FROM search_evidence_chunks
        JOIN evidence_file_chunks ch ON search_evidence_chunks.rowid = ch.id
        JOIN evidence_files e ON e.id = ch.evidence_id
        JOIN cases c ON c.id = e.case_id
        WHERE search_evidence_chunks.{chunk_col} ILIKE $1
          AND e.status != 'deleted'
          AND c.status != 'deleted'
          AND (c.owner_id = $2 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $2
          ))
        ORDER BY score DESC
        LIMIT $4
        "#,
            chunk_col = chunk_col
        )
    };

    let rows = if let Some(case_id) = case_id {
        sqlx::query_as::<_, EvidenceSearchRow>(&list_sql)
            .bind(scoped_query)
            .bind(case_id)
            .bind(user.user_id.to_string())
            .bind(matched_language)
            .bind(limit)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| {
                AppError::internal(format!("db error (chunk-list): {e}\nSQL: {list_sql}"))
            })?
    } else {
        sqlx::query_as::<_, EvidenceSearchRow>(&list_sql)
            .bind(scoped_query)
            .bind(user.user_id.to_string())
            .bind(matched_language)
            .bind(limit)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| {
                AppError::internal(format!("db error (chunk-list): {e}\nSQL: {list_sql}"))
            })?
    };

    Ok(rows)
}

async fn list_legacy_evidence_matches(
    state: &AppState,
    user: &AuthUser,
    fts_query: &str,
    case_id: Option<&str>,
    limit: i64,
) -> AppResult<Vec<EvidenceSearchRow>> {
    let scoped_query = build_fts_column_query("parsed_text", fts_query);
    let list_sql = if case_id.is_some() {
        r#"
        SELECT
            e.id AS file_id,
            e.case_id AS case_id,
            c.name AS case_name,
            e.original_name AS file_name,
            NULL AS chunk_id,
            NULL AS chunk_index,
            COALESCE(NULLIF(trim(e.original_name), ''), 'full document') AS display_label,
            jsonb_build_object(
                'legacy_fallback', 1,
                'display_label', COALESCE(NULLIF(trim(e.original_name), ''), 'full document')
            )::text AS anchor_json,
            e.parsed_text AS snippet_source,
            NULL AS snippet_translated,
            e.created_at AS created_at,
            similarity(search_evidence.searchable, replace($1, '%', '')) ::float8 AS score,
            1::bigint AS translation_incomplete,
            'source' AS matched_language,
            1::bigint AS source_fallback
        FROM search_evidence
        JOIN evidence_files e ON search_evidence.rowid = e.id
        JOIN cases c ON c.id = e.case_id
        WHERE search_evidence.parsed_text ILIKE $1
          AND e.status != 'deleted'
          AND c.status != 'deleted'
          AND e.case_id = $2
          AND NOT EXISTS (
                SELECT 1 FROM evidence_file_chunks ch
                WHERE ch.evidence_id = e.id
          )
          AND (c.owner_id = $3 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $3
          ))
        ORDER BY score DESC
        LIMIT $4
        "#
    } else {
        r#"
        SELECT
            e.id AS file_id,
            e.case_id AS case_id,
            c.name AS case_name,
            e.original_name AS file_name,
            NULL AS chunk_id,
            NULL AS chunk_index,
            COALESCE(NULLIF(trim(e.original_name), ''), 'full document') AS display_label,
            jsonb_build_object(
                'legacy_fallback', 1,
                'display_label', COALESCE(NULLIF(trim(e.original_name), ''), 'full document')
            )::text AS anchor_json,
            e.parsed_text AS snippet_source,
            NULL AS snippet_translated,
            e.created_at AS created_at,
            similarity(search_evidence.searchable, replace($1, '%', '')) ::float8 AS score,
            1::bigint AS translation_incomplete,
            'source' AS matched_language,
            1::bigint AS source_fallback
        FROM search_evidence
        JOIN evidence_files e ON search_evidence.rowid = e.id
        JOIN cases c ON c.id = e.case_id
        WHERE search_evidence.parsed_text ILIKE $1
          AND e.status != 'deleted'
          AND c.status != 'deleted'
          AND NOT EXISTS (
                SELECT 1 FROM evidence_file_chunks ch
                WHERE ch.evidence_id = e.id
          )
          AND (c.owner_id = $2 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $2
          ))
        ORDER BY score DESC
        LIMIT $3
        "#
    };

    let rows = if let Some(case_id) = case_id {
        sqlx::query_as::<_, EvidenceSearchRow>(list_sql)
            .bind(scoped_query)
            .bind(case_id)
            .bind(user.user_id.to_string())
            .bind(limit)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    } else {
        sqlx::query_as::<_, EvidenceSearchRow>(list_sql)
            .bind(scoped_query)
            .bind(user.user_id.to_string())
            .bind(limit)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
    };

    Ok(rows)
}

fn build_fts_column_query(_column_name: &str, fts_query: &str) -> String {
    // Mirror tables index all columns into a unified `searchable` text column,
    // so column-scoped FTS queries reduce to the plain ILIKE pattern.
    fts_query.to_string()
}

fn build_bilingual_fts_query(fts_query: &str) -> String {
    fts_query.to_string()
}

fn evidence_result_key(row: &EvidenceSearchRow) -> String {
    match &row.chunk_id {
        Some(chunk_id) => chunk_id.clone(),
        None => format!("file:{}", row.file_id),
    }
}

fn should_keep_existing_evidence_row(
    existing: &EvidenceSearchRow,
    candidate: &EvidenceSearchRow,
    language_mode: SearchLanguageMode,
) -> bool {
    let existing_priority = evidence_row_priority(existing, language_mode);
    let candidate_priority = evidence_row_priority(candidate, language_mode);

    if existing_priority > candidate_priority {
        return true;
    }
    if existing_priority < candidate_priority {
        return false;
    }

    if existing.score > candidate.score {
        return true;
    }
    if existing.score < candidate.score {
        return false;
    }
    existing.matched_language == "translated" && candidate.matched_language != "translated"
}

fn evidence_row_priority(row: &EvidenceSearchRow, language_mode: SearchLanguageMode) -> i32 {
    if row.chunk_id.is_some() {
        return match language_mode {
            SearchLanguageMode::Source => {
                if row.matched_language == "source" {
                    4
                } else {
                    3
                }
            }
            SearchLanguageMode::Zh | SearchLanguageMode::Bilingual => {
                if row.matched_language == "translated" {
                    4
                } else {
                    3
                }
            }
        };
    }

    if row.source_fallback != 0 {
        2
    } else {
        1
    }
}

fn push_chunk_count_subquery(
    builder: &mut sqlx::QueryBuilder<'_, sqlx::Postgres>,
    scoped_query: &str,
    column_name: &str,
    case_id: Option<&str>,
    user_id: &str,
    has_subquery: &mut bool,
) {
    if *has_subquery {
        builder.push(" UNION ");
    }
    *has_subquery = true;

    let chunk_col = if column_name.contains("translated") {
        "searchable_translated"
    } else if column_name.contains("source") {
        "searchable_source"
    } else {
        "searchable"
    };
    builder.push(&format!("SELECT format('chunk:%s', ch.id) AS result_key FROM search_evidence_chunks JOIN evidence_file_chunks ch ON search_evidence_chunks.rowid = ch.id JOIN evidence_files e ON e.id = ch.evidence_id JOIN cases c ON c.id = e.case_id WHERE search_evidence_chunks.{chunk_col} ILIKE "));
    builder.push_bind(scoped_query.to_string());
    builder.push(" AND e.status != 'deleted' AND c.status != 'deleted'");

    if let Some(case_id) = case_id {
        builder.push(" AND e.case_id = ");
        builder.push_bind(case_id.to_string());
        builder.push(" AND (c.owner_id = ");
        builder.push_bind(user_id.to_string());
        builder.push(
            " OR EXISTS (SELECT 1 FROM case_members m WHERE m.case_id = c.id AND m.user_id = ",
        );
        builder.push_bind(user_id.to_string());
        builder.push("))");
    } else {
        builder.push(" AND (c.owner_id = ");
        builder.push_bind(user_id.to_string());
        builder.push(
            " OR EXISTS (SELECT 1 FROM case_members m WHERE m.case_id = c.id AND m.user_id = ",
        );
        builder.push_bind(user_id.to_string());
        builder.push("))");
    }
}

fn push_title_count_subquery(
    builder: &mut sqlx::QueryBuilder<'_, sqlx::Postgres>,
    fts_query: &str,
    case_id: Option<&str>,
    user_id: &str,
    has_subquery: &mut bool,
) {
    if *has_subquery {
        builder.push(" UNION ");
    }
    *has_subquery = true;

    builder.push("SELECT format('file:%s', e.id) AS result_key FROM search_evidence JOIN evidence_files e ON search_evidence.rowid = e.id JOIN cases c ON c.id = e.case_id WHERE search_evidence.original_name ILIKE ");
    builder.push_bind(fts_query.to_string());
    builder.push(" AND e.status != 'deleted' AND c.status != 'deleted'");

    if let Some(case_id) = case_id {
        builder.push(" AND e.case_id = ");
        builder.push_bind(case_id.to_string());
        builder.push(" AND (c.owner_id = ");
        builder.push_bind(user_id.to_string());
        builder.push(
            " OR EXISTS (SELECT 1 FROM case_members m WHERE m.case_id = c.id AND m.user_id = ",
        );
        builder.push_bind(user_id.to_string());
        builder.push("))");
    } else {
        builder.push(" AND (c.owner_id = ");
        builder.push_bind(user_id.to_string());
        builder.push(
            " OR EXISTS (SELECT 1 FROM case_members m WHERE m.case_id = c.id AND m.user_id = ",
        );
        builder.push_bind(user_id.to_string());
        builder.push("))");
    }
}

fn push_legacy_count_subquery(
    builder: &mut sqlx::QueryBuilder<'_, sqlx::Postgres>,
    fts_query: &str,
    case_id: Option<&str>,
    user_id: &str,
    has_subquery: &mut bool,
) {
    if *has_subquery {
        builder.push(" UNION ");
    }
    *has_subquery = true;

    builder.push("SELECT format('file:%s', e.id) AS result_key FROM search_evidence JOIN evidence_files e ON search_evidence.rowid = e.id JOIN cases c ON c.id = e.case_id WHERE search_evidence.parsed_text ILIKE ");
    builder.push_bind(fts_query.to_string());
    builder.push(" AND e.status != 'deleted' AND c.status != 'deleted' AND NOT EXISTS (SELECT 1 FROM evidence_file_chunks ch WHERE ch.evidence_id = e.id)");

    if let Some(case_id) = case_id {
        builder.push(" AND e.case_id = ");
        builder.push_bind(case_id.to_string());
        builder.push(" AND (c.owner_id = ");
        builder.push_bind(user_id.to_string());
        builder.push(
            " OR EXISTS (SELECT 1 FROM case_members m WHERE m.case_id = c.id AND m.user_id = ",
        );
        builder.push_bind(user_id.to_string());
        builder.push("))");
    } else {
        builder.push(" AND (c.owner_id = ");
        builder.push_bind(user_id.to_string());
        builder.push(
            " OR EXISTS (SELECT 1 FROM case_members m WHERE m.case_id = c.id AND m.user_id = ",
        );
        builder.push_bind(user_id.to_string());
        builder.push("))");
    }
}

fn evidence_search_result_from_row(
    row: EvidenceSearchRow,
    language_mode: SearchLanguageMode,
    keyword_raw: &str,
) -> AppResult<SearchResult> {
    let matched_text = match row.matched_language.as_str() {
        "translated" => row.snippet_translated.as_deref(),
        _ => row.snippet_source.as_deref(),
    };
    let keyword_clean = keyword_raw.trim_matches('%').trim().to_string();
    let offsets = matched_text.and_then(|text| {
        let kw = keyword_clean.as_str();
        if kw.is_empty() {
            None
        } else {
            find_text_span(text, kw)
        }
    });
    let highlight = matched_text.map(|text| highlight_match(text, Some(keyword_raw)));
    let content = matched_text.map(str::to_string);

    Ok(SearchResult {
        object_type: SearchObjectType::Evidence.as_str().to_string(),
        object_id: row.file_id.clone(),
        case_id: Some(row.case_id),
        case_name: Some(row.case_name),
        title: row.file_name.clone(),
        content,
        highlight,
        tags: Vec::new(),
        created_at: row.created_at,
        score: row.score,
        language_mode: Some(language_mode.as_str().to_string()),
        file_id: Some(row.file_id),
        file_name: Some(row.file_name),
        chunk_id: row.chunk_id,
        chunk_index: row.chunk_index,
        display_label: Some(
            row.display_label
                .unwrap_or_else(|| "full document".to_string()),
        ),
        anchor_json: Some(parse_anchor_json(row.anchor_json)?),
        matched_language: Some(row.matched_language),
        snippet_source: row.snippet_source,
        snippet_translated: row.snippet_translated,
        match_start_offset: offsets.map(|(start, _)| start),
        match_end_offset: offsets.map(|(_, end)| end),
        translation_incomplete: Some(row.translation_incomplete != 0),
        source_fallback: Some(row.source_fallback != 0),
    })
}

fn parse_anchor_json(raw: Option<String>) -> AppResult<serde_json::Value> {
    match raw {
        Some(value) => serde_json::from_str(&value)
            .map_err(|e| AppError::internal(format!("invalid search anchor_json: {e}"))),
        None => Ok(serde_json::Value::Null),
    }
}

fn find_text_span(text: &str, needle: &str) -> Option<(i64, i64)> {
    if needle.is_empty() {
        return None;
    }

    if let Some(byte_start) = text.find(needle) {
        let start = text[..byte_start].chars().count() as i64;
        let end = start + needle.chars().count() as i64;
        return Some((start, end));
    }

    if !needle.is_ascii() || !text.is_ascii() {
        return None;
    }

    let lowered_text = text.to_ascii_lowercase();
    let lowered_needle = needle.to_ascii_lowercase();
    lowered_text.find(&lowered_needle).map(|byte_start| {
        let start = text[..byte_start].chars().count() as i64;
        let end = start + needle.chars().count() as i64;
        (start, end)
    })
}

fn highlight_match(text: &str, keyword: Option<&str>) -> String {
    // pg_trgm has no offsets(); wrap the raw keyword (case-insensitively)
    // wherever it occurs in the source text.
    let Some(kw) = keyword else {
        return text.to_string();
    };
    if kw.is_empty() {
        return text.to_string();
    }
    let needle = kw.trim_matches('%');
    if needle.is_empty() {
        return text.to_string();
    }
    let lowered = text.to_lowercase();
    let lowered_needle = needle.to_lowercase();
    let Some(byte_start) = lowered.find(&lowered_needle) else {
        return text.to_string();
    };
    let byte_end = byte_start + needle.len();
    let mut out = String::with_capacity(text.len() + 9);
    out.push_str(&text[..byte_start]);
    out.push_str("<em>");
    out.push_str(&text[byte_start..byte_end]);
    out.push_str("</em>");
    out.push_str(&text[byte_end..]);
    out
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
        JOIN event_nodes n ON search_nodes.rowid = n.id
        JOIN cases c ON c.id = n.case_id
        WHERE search_nodes.searchable ILIKE $1
          AND n.status != 'deleted'
          AND c.status != 'deleted'
          AND n.case_id = $2
          AND (c.owner_id = $3 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $3
          ))
        "#
    } else {
        r#"
        SELECT COUNT(1)
        FROM search_nodes
        JOIN event_nodes n ON search_nodes.rowid = n.id
        JOIN cases c ON c.id = n.case_id
        WHERE search_nodes.searchable ILIKE $1
          AND n.status != 'deleted'
          AND c.status != 'deleted'
          AND (c.owner_id = $2 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $2
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
            n.title AS highlight,
            n.tags AS tags,
            n.created_at AS created_at,
            similarity(search_nodes.searchable, replace($1, '%', '')) ::float8 AS score
        FROM search_nodes
        JOIN event_nodes n ON search_nodes.rowid = n.id
        JOIN cases c ON c.id = n.case_id
        WHERE search_nodes.searchable ILIKE $1
          AND n.status != 'deleted'
          AND c.status != 'deleted'
          AND n.case_id = $2
          AND (c.owner_id = $3 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $3
          ))
        ORDER BY score DESC
        LIMIT $4
        "#
    } else {
        r#"
        SELECT
            n.id AS object_id,
            n.case_id AS case_id,
            c.name AS case_name,
            n.title AS title,
            n.description AS content,
            n.title AS highlight,
            n.tags AS tags,
            n.created_at AS created_at,
            similarity(search_nodes.searchable, replace($1, '%', '')) ::float8 AS score
        FROM search_nodes
        JOIN event_nodes n ON search_nodes.rowid = n.id
        JOIN cases c ON c.id = n.case_id
        WHERE search_nodes.searchable ILIKE $1
          AND n.status != 'deleted'
          AND c.status != 'deleted'
          AND (c.owner_id = $2 OR EXISTS (
                SELECT 1 FROM case_members m
                WHERE m.case_id = c.id AND m.user_id = $2
          ))
        ORDER BY score DESC
        LIMIT $3
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
                language_mode: None,
                file_id: None,
                file_name: None,
                chunk_id: None,
                chunk_index: None,
                display_label: None,
                anchor_json: None,
                matched_language: None,
                snippet_source: None,
                snippet_translated: None,
                match_start_offset: None,
                match_end_offset: None,
                translation_incomplete: None,
                source_fallback: None,
            },
        )
        .collect::<Vec<_>>();

    Ok((out, total.0))
}

async fn search_persons(
    state: &AppState,
    user: &AuthUser,
    fts_query: &str,
    case_id: Option<&str>,
    limit: i64,
) -> AppResult<(Vec<SearchResult>, i64)> {
    let count_sql = if case_id.is_some() {
        r#"
        SELECT COUNT(1)
        FROM search_persons
        JOIN persons p ON search_persons.rowid = p.id
        WHERE search_persons.searchable ILIKE $1
          AND p.status != 'deleted'
          AND EXISTS (
                SELECT 1
                FROM person_case_links l
                JOIN cases c ON c.id = l.case_id
                WHERE l.person_id = p.id
                  AND c.status != 'deleted'
                  AND l.case_id = $2
                  AND (c.owner_id = $3 OR EXISTS (
                        SELECT 1 FROM case_members m
                        WHERE m.case_id = c.id AND m.user_id = $3
                  ))
          )
        "#
    } else {
        r#"
        SELECT COUNT(1)
        FROM search_persons
        JOIN persons p ON search_persons.rowid = p.id
        WHERE search_persons.searchable ILIKE $1
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
            p.id AS object_id,
            $2 AS case_id,
            (SELECT name FROM cases WHERE id = $2) AS case_name,
            p.name AS title,
            NULLIF(trim(
                COALESCE(p.organization, '') || ' ' ||
                COALESCE(p.position, '') || ' ' ||
                COALESCE(p.phone, '') || ' ' ||
                COALESCE(p.email, '')
            ), '') AS content,
            p.name AS highlight,
            p.created_at AS created_at,
            similarity(search_persons.searchable, replace($1, '%', '')) ::float8 AS score
        FROM search_persons
        JOIN persons p ON search_persons.rowid = p.id
        WHERE search_persons.searchable ILIKE $1
          AND p.status != 'deleted'
          AND EXISTS (
                SELECT 1
                FROM person_case_links l
                JOIN cases c ON c.id = l.case_id
                WHERE l.person_id = p.id
                  AND c.status != 'deleted'
                  AND l.case_id = $2
                  AND (c.owner_id = $3 OR EXISTS (
                        SELECT 1 FROM case_members m
                        WHERE m.case_id = c.id AND m.user_id = $3
                  ))
          )
        ORDER BY score DESC
        LIMIT $4
        "#
    } else {
        r#"
        SELECT
            p.id AS object_id,
            (
                SELECT l.case_id
                FROM person_case_links l
                JOIN cases c2 ON c2.id = l.case_id
                WHERE l.person_id = p.id
                  AND c2.status != 'deleted'
                  AND (c2.owner_id = $2 OR EXISTS (
                        SELECT 1 FROM case_members m
                        WHERE m.case_id = c2.id AND m.user_id = $2
                  ))
                ORDER BY l.created_at DESC
                LIMIT 1
            ) AS case_id,
            (
                SELECT c2.name
                FROM person_case_links l
                JOIN cases c2 ON c2.id = l.case_id
                WHERE l.person_id = p.id
                  AND c2.status != 'deleted'
                  AND (c2.owner_id = $2 OR EXISTS (
                        SELECT 1 FROM case_members m
                        WHERE m.case_id = c2.id AND m.user_id = $2
                  ))
                ORDER BY l.created_at DESC
                LIMIT 1
            ) AS case_name,
            p.name AS title,
            NULLIF(trim(
                COALESCE(p.organization, '') || ' ' ||
                COALESCE(p.position, '') || ' ' ||
                COALESCE(p.phone, '') || ' ' ||
                COALESCE(p.email, '')
            ), '') AS content,
            p.name AS highlight,
            p.created_at AS created_at,
            similarity(search_persons.searchable, replace($1, '%', '')) ::float8 AS score
        FROM search_persons
        JOIN persons p ON search_persons.rowid = p.id
        WHERE search_persons.searchable ILIKE $1
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
        ORDER BY score DESC
        LIMIT $3
        "#
    };

    let rows: Vec<(
        String,
        Option<String>,
        Option<String>,
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
                    object_type: SearchObjectType::Person.as_str().to_string(),
                    object_id,
                    case_id,
                    case_name,
                    title,
                    content,
                    highlight: Some(highlight),
                    tags: Vec::new(),
                    created_at,
                    score,
                    language_mode: None,
                    file_id: None,
                    file_name: None,
                    chunk_id: None,
                    chunk_index: None,
                    display_label: None,
                    anchor_json: None,
                    matched_language: None,
                    snippet_source: None,
                    snippet_translated: None,
                    match_start_offset: None,
                    match_end_offset: None,
                    translation_incomplete: None,
                    source_fallback: None,
                }
            },
        )
        .collect::<Vec<_>>();

    Ok((out, total.0))
}

fn build_fts_query(keyword: &str) -> AppResult<String> {
    // pg_trgm ILIKE pattern: keep the raw keyword (quotes stripped), the
    // database matches case-insensitively with trigram indexing.
    let kw = keyword.replace('"', "").trim().to_string();
    if kw.is_empty() {
        return Err(AppError::bad_request("keyword required"));
    }
    Ok(format!("%{kw}%"))
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
        "INSERT INTO search_histories (id, user_id, keyword, object_types, case_id, result_count) VALUES ($1, $2, $3, $4, $5, $6)",
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
        VALUES ($1, $2, 1, utc_text(), utc_text())
        ON CONFLICT(user_id, keyword)
        DO UPDATE SET
          search_count = user_hot_searches.search_count + 1,
          last_searched = utc_text(),
          updated_at = utc_text()
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
        "SELECT keyword FROM user_hot_searches WHERE user_id = $1 AND keyword LIKE $2 ORDER BY search_count DESC LIMIT 10",
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
