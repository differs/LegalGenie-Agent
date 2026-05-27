use crate::config::TranslationConfig;
use crate::parser;
use crate::state::AppState;
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration as StdDuration;

const RETRY_POLL_INTERVAL: StdDuration = StdDuration::from_secs(30);
const FILE_ERROR_MAX_CHARS: usize = 2_000;

#[derive(Debug, Clone)]
pub struct TranslationRequest {
    pub evidence_id: String,
    pub chunk_id: String,
    pub chunk_index: i64,
    pub source_text: String,
    pub target_language: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationResult {
    pub translated_text: String,
    pub source_language: Option<String>,
}

#[async_trait]
pub trait TranslationProvider: Send + Sync {
    fn provider_name(&self) -> &str;

    fn model_name(&self) -> Option<&str>;

    fn is_enabled(&self) -> bool {
        true
    }

    async fn translate(&self, request: TranslationRequest) -> anyhow::Result<TranslationResult>;
}

pub fn provider_from_config(
    config: &TranslationConfig,
) -> anyhow::Result<Arc<dyn TranslationProvider>> {
    if config.provider.trim().eq_ignore_ascii_case("disabled") {
        return Ok(Arc::new(DisabledTranslationProvider));
    }

    Ok(Arc::new(OpenAiCompatibleTranslationProvider::new(config)?))
}

pub async fn start_file_translation(state: AppState, file_id: String) -> anyhow::Result<()> {
    tokio::spawn(async move {
        if let Err(error) = process_file_translation(state.clone(), file_id.clone()).await {
            tracing::error!(file_id = %file_id, error = %error, "translation worker failed");
        }
    });
    Ok(())
}

pub fn start_retry_poller_once(state: AppState) {
    static STARTED: OnceLock<()> = OnceLock::new();

    if STARTED.set(()).is_err() {
        return;
    }

    tokio::spawn(async move {
        loop {
            if let Err(error) = run_retry_cycle_once(state.clone()).await {
                tracing::error!(error = %error, "translation retry poller cycle failed");
            }
            tokio::time::sleep(RETRY_POLL_INTERVAL).await;
        }
    });
}

pub async fn run_retry_cycle_once(state: AppState) -> anyhow::Result<()> {
    let stale_file_ids = find_stale_processing_file_ids(&state.pool).await?;
    if !stale_file_ids.is_empty() {
        reset_stale_processing_chunks(&state.pool, &stale_file_ids).await?;
    }

    let due_file_ids = find_due_file_ids(&state.pool).await?;
    for file_id in stale_file_ids.into_iter().chain(due_file_ids.into_iter()) {
        start_file_translation(state.clone(), file_id).await?;
    }

    Ok(())
}

async fn process_file_translation(state: AppState, file_id: String) -> anyhow::Result<()> {
    let session = ensure_file_translation_session(&state, &file_id).await?;

    if !state.translation_provider.is_enabled() {
        mark_file_translation_disabled(&state, &session).await?;
        return Ok(());
    }

    initialize_file_translation(&state, &session).await?;

    loop {
        reset_stale_processing_chunks_for_session(&state.pool, &session).await?;

        let mut join_set = tokio::task::JoinSet::new();
        for _ in 0..usize::from(state.translation.max_concurrency.max(1)) {
            let Some(chunk) = claim_next_due_chunk(&state.pool, &session).await? else {
                break;
            };
            let state_clone = state.clone();
            let session_clone = session.clone();
            join_set.spawn(async move {
                translate_claimed_chunk(state_clone, chunk, &session_clone).await
            });
        }

        if join_set.is_empty() {
            refresh_file_translation_aggregate_for_session(&state.pool, &session).await?;
            break;
        }

        while let Some(result) = join_set.join_next().await {
            result.context("join translation chunk task")??;
        }

        refresh_file_translation_aggregate_for_session(&state.pool, &session).await?;
    }

    Ok(())
}

async fn initialize_file_translation(
    state: &AppState,
    session: &TranslationSession,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE evidence_files
        SET
            translation_status = CASE
                WHEN chunk_count > 0 THEN 'processing'
                ELSE 'done'
            END,
            translation_error = NULL,
            target_language = ?1
        WHERE id = ?2 AND status != 'deleted' AND parse_status = 'done' AND parsed_at = ?3
        "#,
    )
    .bind(&state.translation.target_language)
    .bind(&session.file_id)
    .bind(&session.parsed_at)
    .execute(&state.pool)
    .await
    .context("initialize file translation")?;

    if session.lock.provider != state.translation_provider.provider_name()
        || session.lock.model != effective_model_name(state)
    {
        anyhow::bail!("translation provider/model drift detected after initialization");
    }

    Ok(())
}

async fn mark_file_translation_disabled(
    state: &AppState,
    session: &TranslationSession,
) -> anyhow::Result<()> {
    let error = "translation provider disabled";
    let provider = state.translation_provider.provider_name().to_string();
    let model = effective_model_name(state);
    let mut tx = state
        .pool
        .begin()
        .await
        .context("begin disabled translation tx")?;

    sqlx::query(
        r#"
        UPDATE evidence_file_chunks
        SET
            translation_status = CASE
                WHEN translation_status = 'done' THEN 'done'
                ELSE 'failed'
            END,
            target_language = ?1,
            translation_error = CASE
                WHEN translation_status = 'done' THEN translation_error
                ELSE ?2
            END,
            next_retry_at = NULL,
            updated_at = CURRENT_TIMESTAMP
        WHERE evidence_id = ?3
          AND EXISTS (
              SELECT 1
              FROM evidence_files
              WHERE id = ?3 AND parsed_at = ?4 AND status != 'deleted'
          )
        "#,
    )
    .bind(&state.translation.target_language)
    .bind(error)
    .bind(&session.file_id)
    .bind(&session.parsed_at)
    .execute(&mut *tx)
    .await
    .context("mark disabled translation chunks failed")?;

    sqlx::query(
        r#"
        UPDATE evidence_files
        SET
            translation_status = 'failed',
            translation_error = ?1,
            target_language = ?2,
            translation_provider = ?3,
            translation_model = ?4,
            translated_chunk_count = (
                SELECT COUNT(1)
                FROM evidence_file_chunks
                WHERE evidence_id = ?5 AND translation_status = 'done'
            ),
            failed_chunk_count = (
                SELECT COUNT(1)
                FROM evidence_file_chunks
                WHERE evidence_id = ?5 AND translation_status = 'failed'
            )
        WHERE id = ?5 AND parsed_at = ?6
        "#,
    )
    .bind(error)
    .bind(&state.translation.target_language)
    .bind(provider)
    .bind(model)
    .bind(&session.file_id)
    .bind(&session.parsed_at)
    .execute(&mut *tx)
    .await
    .context("mark disabled translation file failed")?;

    tx.commit()
        .await
        .context("commit disabled translation tx")?;
    Ok(())
}

#[derive(Debug, Clone, FromRow)]
struct TranslationChunkRow {
    id: String,
    evidence_id: String,
    chunk_index: i64,
    source_text: String,
    source_text_hash: String,
    retry_count: i64,
    max_retries: i64,
}

#[derive(Debug, Clone, FromRow)]
struct FileTranslationStateRow {
    parsed_at: Option<String>,
    translation_provider: Option<String>,
    translation_model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileTranslationLock {
    provider: String,
    model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TranslationSession {
    file_id: String,
    parsed_at: String,
    lock: FileTranslationLock,
}

#[derive(Debug, Clone)]
struct ClaimedTranslationChunk {
    row: TranslationChunkRow,
    attempt_token: String,
}

async fn claim_next_due_chunk(
    pool: &SqlitePool,
    session: &TranslationSession,
) -> anyhow::Result<Option<ClaimedTranslationChunk>> {
    loop {
        let row: Option<TranslationChunkRow> = sqlx::query_as(
            r#"
            SELECT
                c.id,
                c.evidence_id,
                c.chunk_index,
                c.source_text,
                c.source_text_hash,
                c.retry_count,
                c.max_retries
            FROM evidence_file_chunks c
            INNER JOIN evidence_files f ON f.id = c.evidence_id
            WHERE c.evidence_id = ?1
              AND f.parsed_at = ?2
              AND f.status != 'deleted'
              AND f.parse_status = 'done'
              AND (
                    (
                        c.translation_status = 'pending'
                        AND (
                            c.retry_count = 0
                            OR (
                                c.retry_count <= c.max_retries
                                AND c.next_retry_at IS NOT NULL
                                AND c.next_retry_at <= CURRENT_TIMESTAMP
                            )
                        )
                    )
                 OR (
                    c.translation_status = 'failed'
                    AND c.retry_count <= c.max_retries
                    AND c.next_retry_at IS NOT NULL
                    AND c.next_retry_at <= CURRENT_TIMESTAMP
                 )
              )
            ORDER BY c.chunk_index ASC
            LIMIT 1
            "#,
        )
        .bind(&session.file_id)
        .bind(&session.parsed_at)
        .fetch_optional(pool)
        .await
        .context("select due translation chunk")?;

        let Some(row) = row else {
            return Ok(None);
        };

        let attempt_token = current_timestamp_precise();
        let claimed = sqlx::query(
            r#"
            UPDATE evidence_file_chunks
            SET
                translation_status = 'processing',
                last_attempt_at = ?2,
                translation_error = NULL,
                next_retry_at = NULL,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
              AND (
                    (
                        translation_status = 'pending'
                        AND (
                            retry_count = 0
                            OR (
                                retry_count <= max_retries
                                AND next_retry_at IS NOT NULL
                                AND next_retry_at <= CURRENT_TIMESTAMP
                            )
                        )
                    )
                 OR (
                    translation_status = 'failed'
                    AND retry_count <= max_retries
                    AND next_retry_at IS NOT NULL
                    AND next_retry_at <= CURRENT_TIMESTAMP
                 )
              )
              AND EXISTS (
                    SELECT 1
                    FROM evidence_files
                    WHERE id = evidence_file_chunks.evidence_id
                      AND parsed_at = ?3
                      AND status != 'deleted'
                      AND parse_status = 'done'
              )
            "#,
        )
        .bind(&row.id)
        .bind(&attempt_token)
        .bind(&session.parsed_at)
        .execute(pool)
        .await
        .context("claim translation chunk")?;

        if claimed.rows_affected() > 0 {
            return Ok(Some(ClaimedTranslationChunk { row, attempt_token }));
        }
    }
}

async fn translate_claimed_chunk(
    state: AppState,
    chunk: ClaimedTranslationChunk,
    session: &TranslationSession,
) -> anyhow::Result<()> {
    let current_hash = parser::source_text_hash(&chunk.row.source_text);
    if chunk.row.source_text_hash != current_hash {
        let message = "source text hash mismatch";
        mark_chunk_failed(&state.pool, &chunk, session, message).await?;
        return Ok(());
    }

    tracing::info!(
        file_id = %chunk.row.evidence_id,
        chunk_id = %chunk.row.id,
        chunk_index = chunk.row.chunk_index,
        provider = %state.translation_provider.provider_name(),
        model = ?effective_model_name(&state),
        retry_count = chunk.row.retry_count,
        "translating evidence chunk"
    );

    let request = TranslationRequest {
        evidence_id: chunk.row.evidence_id.clone(),
        chunk_id: chunk.row.id.clone(),
        chunk_index: chunk.row.chunk_index,
        source_text: chunk.row.source_text.clone(),
        target_language: state.translation.target_language.clone(),
    };

    match state.translation_provider.translate(request).await {
        Ok(result) => {
            mark_chunk_done(
                &state.pool,
                &chunk,
                session,
                &result,
                &state.translation.target_language,
            )
            .await?;
            Ok(())
        }
        Err(error) => {
            tracing::warn!(
                file_id = %chunk.row.evidence_id,
                chunk_id = %chunk.row.id,
                chunk_index = chunk.row.chunk_index,
                provider = %state.translation_provider.provider_name(),
                model = ?effective_model_name(&state),
                retry_count = chunk.row.retry_count,
                error = %error,
                "translation provider returned error"
            );
            mark_chunk_failed(&state.pool, &chunk, session, &error.to_string()).await?;
            Ok(())
        }
    }
}

async fn mark_chunk_done(
    pool: &SqlitePool,
    chunk: &ClaimedTranslationChunk,
    session: &TranslationSession,
    result: &TranslationResult,
    target_language: &str,
) -> anyhow::Result<()> {
    let source_language = result
        .source_language
        .clone()
        .or_else(|| infer_source_language(&chunk.row.source_text));

    let updated = sqlx::query(
        r#"
        UPDATE evidence_file_chunks
        SET
            translated_text = ?1,
            source_language = ?2,
            target_language = ?3,
            translation_status = 'done',
            translation_error = NULL,
            translated_at = CURRENT_TIMESTAMP,
            next_retry_at = NULL,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ?4
          AND evidence_id = ?5
          AND translation_status = 'processing'
          AND last_attempt_at = ?6
          AND EXISTS (
              SELECT 1
              FROM evidence_files
              WHERE id = ?5
                AND parsed_at = ?7
                AND status != 'deleted'
                AND parse_status = 'done'
          )
        "#,
    )
    .bind(&result.translated_text)
    .bind(source_language.as_deref())
    .bind(target_language)
    .bind(&chunk.row.id)
    .bind(&chunk.row.evidence_id)
    .bind(&chunk.attempt_token)
    .bind(&session.parsed_at)
    .execute(pool)
    .await
    .context("mark translation chunk done")?;

    if updated.rows_affected() == 0 {
        tracing::debug!(
            chunk_id = %chunk.row.id,
            evidence_id = %chunk.row.evidence_id,
            "ignoring stale translation success result"
        );
    }

    Ok(())
}

async fn mark_chunk_failed(
    pool: &SqlitePool,
    chunk: &ClaimedTranslationChunk,
    session: &TranslationSession,
    error: &str,
) -> anyhow::Result<()> {
    let next_retry_count = chunk.row.retry_count + 1;
    let next_retry_at = if next_retry_count > chunk.row.max_retries {
        None
    } else {
        Some(next_retry_at(next_retry_count))
    };

    let updated = sqlx::query(
        r#"
        UPDATE evidence_file_chunks
        SET
            translation_status = 'failed',
            retry_count = ?1,
            next_retry_at = ?2,
            translation_error = ?3,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ?4
          AND evidence_id = ?5
          AND translation_status = 'processing'
          AND last_attempt_at = ?6
          AND EXISTS (
              SELECT 1
              FROM evidence_files
              WHERE id = ?5
                AND parsed_at = ?7
                AND status != 'deleted'
                AND parse_status = 'done'
          )
        "#,
    )
    .bind(next_retry_count)
    .bind(next_retry_at)
    .bind(truncate_error(error))
    .bind(&chunk.row.id)
    .bind(&chunk.row.evidence_id)
    .bind(&chunk.attempt_token)
    .bind(&session.parsed_at)
    .execute(pool)
    .await
    .context("mark translation chunk failed")?;

    if updated.rows_affected() == 0 {
        tracing::debug!(
            chunk_id = %chunk.row.id,
            evidence_id = %chunk.row.evidence_id,
            "ignoring stale translation failure result"
        );
    }

    Ok(())
}

async fn refresh_file_translation_aggregate_for_session(
    pool: &SqlitePool,
    session: &TranslationSession,
) -> anyhow::Result<()> {
    #[derive(Debug, FromRow)]
    struct AggregateRow {
        total_count: i64,
        pending_count: i64,
        processing_count: i64,
        done_count: i64,
        failed_count: i64,
        latest_error: Option<String>,
    }

    let aggregate: AggregateRow = sqlx::query_as(
        r#"
        SELECT
            COUNT(1) AS total_count,
            COALESCE(SUM(CASE WHEN translation_status = 'pending' THEN 1 ELSE 0 END), 0) AS pending_count,
            COALESCE(SUM(CASE WHEN translation_status = 'processing' THEN 1 ELSE 0 END), 0) AS processing_count,
            COALESCE(SUM(CASE WHEN translation_status = 'done' THEN 1 ELSE 0 END), 0) AS done_count,
            COALESCE(SUM(CASE WHEN translation_status = 'failed' THEN 1 ELSE 0 END), 0) AS failed_count,
            (
                SELECT translation_error
                FROM evidence_file_chunks
                WHERE evidence_id = ?1
                  AND translation_status = 'failed'
                  AND translation_error IS NOT NULL
                ORDER BY updated_at DESC, chunk_index DESC
                LIMIT 1
            ) AS latest_error
        FROM evidence_file_chunks
        WHERE evidence_id = ?1
        "#,
    )
    .bind(&session.file_id)
    .fetch_one(pool)
    .await
    .context("fetch file translation aggregate")?;

    let source_language = aggregate_dominant_source_language(pool, &session.file_id).await?;

    let status = if aggregate.total_count == 0 || aggregate.done_count == aggregate.total_count {
        "done"
    } else if aggregate.pending_count > 0 || aggregate.processing_count > 0 {
        "processing"
    } else if aggregate.failed_count > 0 && aggregate.done_count > 0 {
        "partial"
    } else if aggregate.failed_count > 0 {
        "failed"
    } else {
        "processing"
    };

    let translation_error = match status {
        "done" => None,
        "partial" | "failed" => aggregate.latest_error.as_deref().map(truncate_error),
        _ => None,
    };

    sqlx::query(
        r#"
        UPDATE evidence_files
        SET
            translation_status = ?1,
            translation_error = ?2,
            source_language = ?3,
            translated_chunk_count = ?4,
            failed_chunk_count = ?5
        WHERE id = ?6 AND parsed_at = ?7
        "#,
    )
    .bind(status)
    .bind(translation_error)
    .bind(source_language)
    .bind(aggregate.done_count)
    .bind(aggregate.failed_count)
    .bind(&session.file_id)
    .bind(&session.parsed_at)
    .execute(pool)
    .await
    .context("update file translation aggregate")?;

    Ok(())
}

async fn find_stale_processing_file_ids(pool: &SqlitePool) -> anyhow::Result<Vec<String>> {
    #[derive(Debug, FromRow)]
    struct FileIdRow {
        evidence_id: String,
    }

    let rows: Vec<FileIdRow> = sqlx::query_as(
        r#"
        SELECT DISTINCT c.evidence_id
        FROM evidence_file_chunks c
        INNER JOIN evidence_files f ON f.id = c.evidence_id
        WHERE f.status != 'deleted'
          AND f.parse_status = 'done'
          AND c.translation_status = 'processing'
          AND c.last_attempt_at IS NOT NULL
          AND datetime(c.last_attempt_at) <= datetime(CURRENT_TIMESTAMP, '-10 minutes')
        "#,
    )
    .fetch_all(pool)
    .await
    .context("find stale processing chunks")?;

    Ok(rows.into_iter().map(|row| row.evidence_id).collect())
}

async fn reset_stale_processing_chunks(
    pool: &SqlitePool,
    file_ids: &[String],
) -> anyhow::Result<()> {
    for file_id in file_ids {
        sqlx::query(
            r#"
            UPDATE evidence_file_chunks
            SET
                retry_count = retry_count + 1,
                translation_status = CASE
                    WHEN retry_count + 1 > max_retries THEN 'failed'
                    ELSE 'pending'
                END,
                next_retry_at = CASE
                    WHEN retry_count + 1 > max_retries THEN NULL
                    WHEN retry_count + 1 = 1 THEN datetime(CURRENT_TIMESTAMP, '+1 minute')
                    WHEN retry_count + 1 = 2 THEN datetime(CURRENT_TIMESTAMP, '+5 minutes')
                    ELSE datetime(CURRENT_TIMESTAMP, '+30 minutes')
                END,
                translation_error = COALESCE(translation_error, 'translation processing timed out; retrying'),
                updated_at = CURRENT_TIMESTAMP
            WHERE evidence_id = ?1
              AND translation_status = 'processing'
              AND last_attempt_at IS NOT NULL
              AND datetime(last_attempt_at) <= datetime(CURRENT_TIMESTAMP, '-10 minutes')
            "#,
        )
        .bind(file_id)
        .execute(pool)
        .await
        .with_context(|| format!("reset stale processing chunks for {file_id}"))?;
    }

    Ok(())
}

async fn reset_stale_processing_chunks_for_session(
    pool: &SqlitePool,
    session: &TranslationSession,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE evidence_file_chunks
        SET
            retry_count = retry_count + 1,
            translation_status = CASE
                WHEN retry_count + 1 > max_retries THEN 'failed'
                ELSE 'pending'
            END,
            next_retry_at = CASE
                WHEN retry_count + 1 > max_retries THEN NULL
                WHEN retry_count + 1 = 1 THEN datetime(CURRENT_TIMESTAMP, '+1 minute')
                WHEN retry_count + 1 = 2 THEN datetime(CURRENT_TIMESTAMP, '+5 minutes')
                ELSE datetime(CURRENT_TIMESTAMP, '+30 minutes')
            END,
            translation_error = COALESCE(translation_error, 'translation processing timed out; retrying'),
            updated_at = CURRENT_TIMESTAMP
        WHERE evidence_id = ?1
          AND translation_status = 'processing'
          AND last_attempt_at IS NOT NULL
          AND datetime(last_attempt_at) <= datetime(CURRENT_TIMESTAMP, '-10 minutes')
          AND EXISTS (
              SELECT 1
              FROM evidence_files
              WHERE id = ?1
                AND parsed_at = ?2
                AND status != 'deleted'
                AND parse_status = 'done'
          )
        "#,
    )
    .bind(&session.file_id)
    .bind(&session.parsed_at)
    .execute(pool)
    .await
    .with_context(|| format!("reset stale processing chunks for {}", session.file_id))?;

    Ok(())
}

async fn find_due_file_ids(pool: &SqlitePool) -> anyhow::Result<Vec<String>> {
    #[derive(Debug, FromRow)]
    struct FileIdRow {
        evidence_id: String,
    }

    let rows: Vec<FileIdRow> = sqlx::query_as(
        r#"
        SELECT DISTINCT c.evidence_id
        FROM evidence_file_chunks c
        INNER JOIN evidence_files f ON f.id = c.evidence_id
        WHERE f.status != 'deleted'
          AND f.parse_status = 'done'
          AND (
                (
                    c.translation_status = 'pending'
                    AND (
                        c.retry_count = 0
                        OR (
                            c.retry_count <= c.max_retries
                            AND c.next_retry_at IS NOT NULL
                            AND c.next_retry_at <= CURRENT_TIMESTAMP
                        )
                    )
                )
             OR (
                c.translation_status = 'failed'
                AND c.retry_count <= c.max_retries
                AND c.next_retry_at IS NOT NULL
                AND c.next_retry_at <= CURRENT_TIMESTAMP
             )
          )
        ORDER BY c.evidence_id ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .context("find due translation files")?;

    Ok(rows.into_iter().map(|row| row.evidence_id).collect())
}

fn next_retry_at(retry_count: i64) -> String {
    let minutes = match retry_count {
        1 => 1,
        2 => 5,
        _ => 30,
    };
    (Utc::now() + Duration::minutes(minutes))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

fn current_timestamp_precise() -> String {
    Utc::now().format("%Y-%m-%d %H:%M:%S%.6f").to_string()
}

fn truncate_error(message: &str) -> String {
    message.chars().take(FILE_ERROR_MAX_CHARS).collect()
}

fn effective_model_name(state: &AppState) -> Option<String> {
    state
        .translation_provider
        .model_name()
        .map(ToOwned::to_owned)
        .or_else(|| state.translation.model.clone())
}

async fn ensure_file_translation_session(
    state: &AppState,
    file_id: &str,
) -> anyhow::Result<TranslationSession> {
    let current_provider = state
        .translation_provider
        .provider_name()
        .trim()
        .to_string();
    let current_model = effective_model_name(state);
    loop {
        let existing: FileTranslationStateRow = sqlx::query_as(
            r#"
            SELECT
                parsed_at,
                translation_provider,
                translation_model
            FROM evidence_files
            WHERE id = ?1 AND status != 'deleted' AND parse_status = 'done'
            LIMIT 1
            "#,
        )
        .bind(file_id)
        .fetch_one(&state.pool)
        .await
        .context("fetch existing file translation state")?;

        let Some(parsed_at) = existing.parsed_at.clone() else {
            anyhow::bail!("translation session missing parsed_at");
        };

        if existing.translation_provider.is_none() && existing.translation_model.is_none() {
            let updated = sqlx::query(
                r#"
                UPDATE evidence_files
                SET
                    translation_provider = ?1,
                    translation_model = ?2
                WHERE id = ?3
                  AND status != 'deleted'
                  AND parse_status = 'done'
                  AND parsed_at = ?4
                  AND translation_provider IS NULL
                  AND translation_model IS NULL
                "#,
            )
            .bind(&current_provider)
            .bind(current_model.as_deref())
            .bind(file_id)
            .bind(&parsed_at)
            .execute(&state.pool)
            .await
            .context("persist file translation lock")?;

            if updated.rows_affected() > 0 {
                return Ok(TranslationSession {
                    file_id: file_id.to_string(),
                    parsed_at,
                    lock: FileTranslationLock {
                        provider: current_provider.clone(),
                        model: current_model.clone(),
                    },
                });
            }

            continue;
        }

        match existing.translation_provider {
            Some(provider)
                if provider != current_provider || existing.translation_model != current_model =>
            {
                let message = format!(
                    "translation provider/model mismatch: locked provider={}, locked model={:?}, current provider={}, current model={:?}",
                    provider,
                    existing.translation_model,
                    current_provider,
                    current_model
                );
                stop_file_translation_for_mismatch(state, file_id, &parsed_at, &message).await?;
                anyhow::bail!(message);
            }
            Some(provider) => {
                return Ok(TranslationSession {
                    file_id: file_id.to_string(),
                    parsed_at,
                    lock: FileTranslationLock {
                        provider,
                        model: existing.translation_model,
                    },
                });
            }
            None => continue,
        }
    }
}

async fn stop_file_translation_for_mismatch(
    state: &AppState,
    file_id: &str,
    parsed_at: &str,
    message: &str,
) -> anyhow::Result<()> {
    let mut tx = state
        .pool
        .begin()
        .await
        .context("begin translation mismatch tx")?;

    sqlx::query(
        r#"
        UPDATE evidence_file_chunks
        SET
            translation_status = CASE
                WHEN translation_status = 'done' THEN 'done'
                ELSE 'failed'
            END,
            next_retry_at = NULL,
            translation_error = CASE
                WHEN translation_status = 'done' THEN translation_error
                ELSE ?1
            END,
            updated_at = CURRENT_TIMESTAMP
        WHERE evidence_id = ?2
          AND EXISTS (
              SELECT 1
              FROM evidence_files
              WHERE id = ?2
                AND parsed_at = ?3
                AND status != 'deleted'
                AND parse_status = 'done'
          )
        "#,
    )
    .bind(truncate_error(message))
    .bind(file_id)
    .bind(parsed_at)
    .execute(&mut *tx)
    .await
    .context("mark chunks failed for translation mismatch")?;

    tx.commit()
        .await
        .context("commit translation mismatch tx")?;

    refresh_file_translation_aggregate_for_session(
        &state.pool,
        &TranslationSession {
            file_id: file_id.to_string(),
            parsed_at: parsed_at.to_string(),
            lock: FileTranslationLock {
                provider: String::new(),
                model: None,
            },
        },
    )
    .await?;
    sqlx::query(
        "UPDATE evidence_files SET translation_error = ?1 WHERE id = ?2 AND parsed_at = ?3",
    )
    .bind(truncate_error(message))
    .bind(file_id)
    .bind(parsed_at)
    .execute(&state.pool)
    .await
    .context("store translation mismatch error")?;

    Ok(())
}

async fn aggregate_dominant_source_language(
    pool: &SqlitePool,
    file_id: &str,
) -> anyhow::Result<Option<String>> {
    #[derive(Debug, FromRow)]
    struct ChunkLanguageRow {
        source_language: Option<String>,
        source_text: String,
    }

    let rows: Vec<ChunkLanguageRow> = sqlx::query_as(
        r#"
        SELECT source_language, source_text
        FROM evidence_file_chunks
        WHERE evidence_id = ?1
        ORDER BY chunk_index ASC
        "#,
    )
    .bind(file_id)
    .fetch_all(pool)
    .await
    .context("fetch chunk languages for aggregate")?;

    let mut scores: HashMap<String, usize> = HashMap::new();
    for row in rows {
        let language = row
            .source_language
            .or_else(|| infer_source_language(&row.source_text));
        if let Some(language) = language {
            *scores.entry(language).or_default() += 1;
        }
    }

    Ok(scores
        .into_iter()
        .max_by(|(lang_a, count_a), (lang_b, count_b)| {
            count_a.cmp(count_b).then_with(|| lang_b.cmp(lang_a))
        })
        .map(|(language, _)| language))
}

fn infer_source_language(text: &str) -> Option<String> {
    let mut cjk_count = 0usize;
    let mut ascii_alpha_count = 0usize;

    for ch in text.chars() {
        if is_cjk(ch) {
            cjk_count += 1;
        } else if ch.is_ascii_alphabetic() {
            ascii_alpha_count += 1;
        }
    }

    if cjk_count > 0 {
        Some("zh-CN".to_string())
    } else if ascii_alpha_count > 0 {
        Some("en".to_string())
    } else {
        None
    }
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x2CEB0..=0x2EBEF
    )
}

struct DisabledTranslationProvider;

#[async_trait]
impl TranslationProvider for DisabledTranslationProvider {
    fn provider_name(&self) -> &str {
        "disabled"
    }

    fn model_name(&self) -> Option<&str> {
        None
    }

    fn is_enabled(&self) -> bool {
        false
    }

    async fn translate(&self, _request: TranslationRequest) -> anyhow::Result<TranslationResult> {
        Err(anyhow!("translation provider disabled"))
    }
}

struct OpenAiCompatibleTranslationProvider {
    provider_name: String,
    model_name: String,
    endpoint: String,
    api_key: Option<String>,
    client: Client,
}

impl OpenAiCompatibleTranslationProvider {
    fn new(config: &TranslationConfig) -> anyhow::Result<Self> {
        let base_url = config
            .base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow!("TRANSLATION_BASE_URL is required when translation is enabled")
            })?;
        let model_name = config
            .model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("TRANSLATION_MODEL is required when translation is enabled"))?;

        Ok(Self {
            provider_name: config.provider.trim().to_string(),
            model_name: model_name.to_string(),
            endpoint: format!("{}/chat/completions", base_url.trim_end_matches('/')),
            api_key: config.api_key.clone(),
            client: Client::new(),
        })
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: [ChatMessage<'a>; 2],
    temperature: u8,
}

#[derive(Debug, Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatCompletionChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    message: ChatCompletionMessage,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionMessage {
    content: serde_json::Value,
}

#[async_trait]
impl TranslationProvider for OpenAiCompatibleTranslationProvider {
    fn provider_name(&self) -> &str {
        &self.provider_name
    }

    fn model_name(&self) -> Option<&str> {
        Some(&self.model_name)
    }

    async fn translate(&self, request: TranslationRequest) -> anyhow::Result<TranslationResult> {
        let system_prompt = format!(
            "你是法律证据翻译与规范化助手。请把用户提供的证据文本忠实翻译为{target_language}。不得删减、补充、改写、总结或解释，不得省略格式和编号，只返回译文正文。",
            target_language = request.target_language
        );
        let body = ChatCompletionRequest {
            model: &self.model_name,
            messages: [
                ChatMessage {
                    role: "system",
                    content: &system_prompt,
                },
                ChatMessage {
                    role: "user",
                    content: &request.source_text,
                },
            ],
            temperature: 0,
        };

        let mut request_builder = self.client.post(&self.endpoint).json(&body);
        if let Some(api_key) = &self.api_key {
            request_builder = request_builder.bearer_auth(api_key);
        }

        let response = request_builder
            .send()
            .await
            .context("call translation provider")?
            .error_for_status()
            .context("translation provider returned non-success status")?;

        let payload: ChatCompletionResponse = response
            .json()
            .await
            .context("decode translation provider response")?;

        let translated_text = payload
            .choices
            .into_iter()
            .next()
            .and_then(|choice| extract_message_text(choice.message.content))
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
            .ok_or_else(|| anyhow!("translation provider returned empty content"))?;

        Ok(TranslationResult {
            translated_text,
            source_language: None,
        })
    }
}

fn extract_message_text(content: serde_json::Value) -> Option<String> {
    match content {
        serde_json::Value::String(text) => Some(text),
        serde_json::Value::Array(items) => {
            let text = items
                .into_iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
                .join("");
            if text.trim().is_empty() {
                None
            } else {
                Some(text)
            }
        }
        _ => None,
    }
}
