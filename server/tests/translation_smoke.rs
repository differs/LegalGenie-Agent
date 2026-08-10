mod test_support;

use anyhow::anyhow;
use axum::http::{Method, StatusCode};
use legalminds_server::translation::{
    self, TranslationProvider, TranslationRequest, TranslationResult,
};
use legalminds_server::{router, AppConfig, AppEnv, AppState, CorsOrigins, TranslationConfig};
use serde_json::Value;
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use test_support::{
    create_case, register_user, request_json, request_multipart_text, upload_text_file,
    wait_for_file_parse_done,
};
use tokio::sync::Notify;

#[derive(Clone, Default)]
struct FakeTranslationProvider {
    provider_name: String,
    model_name: Option<String>,
    responses: Arc<Mutex<HashMap<String, VecDeque<FakeProviderResponse>>>>,
    call_counts: Arc<Mutex<HashMap<String, usize>>>,
}

#[derive(Clone)]
enum FakeProviderResponse {
    Success {
        translated_text: String,
        source_language: Option<String>,
    },
    Failure {
        message: String,
    },
    Wait {
        notify: Arc<Notify>,
        next: Box<FakeProviderResponse>,
    },
}

impl FakeTranslationProvider {
    fn new(provider_name: impl Into<String>, model_name: Option<&str>) -> Self {
        Self {
            provider_name: provider_name.into(),
            model_name: model_name.map(str::to_string),
            responses: Arc::new(Mutex::new(HashMap::new())),
            call_counts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn push_responses(
        &self,
        source_text: &str,
        responses: impl IntoIterator<Item = FakeProviderResponse>,
    ) {
        self.responses
            .lock()
            .expect("lock fake responses")
            .insert(source_text.to_string(), responses.into_iter().collect());
    }

    fn call_count(&self, source_text: &str) -> usize {
        self.call_counts
            .lock()
            .expect("lock fake counts")
            .get(source_text)
            .copied()
            .unwrap_or(0)
    }
}

#[async_trait::async_trait]
impl TranslationProvider for FakeTranslationProvider {
    fn provider_name(&self) -> &str {
        &self.provider_name
    }

    fn model_name(&self) -> Option<&str> {
        self.model_name.as_deref()
    }

    async fn translate(&self, request: TranslationRequest) -> anyhow::Result<TranslationResult> {
        let _ = (
            request.evidence_id.as_str(),
            request.chunk_id.as_str(),
            request.chunk_index,
            request.target_language.as_str(),
        );

        {
            let mut counts = self.call_counts.lock().expect("lock fake counts");
            let entry = counts.entry(request.source_text.clone()).or_insert(0);
            *entry += 1;
        }

        let response = self
            .responses
            .lock()
            .expect("lock fake responses")
            .get_mut(&request.source_text)
            .and_then(|items| items.pop_front());

        match response {
            Some(FakeProviderResponse::Success {
                translated_text,
                source_language,
            }) => Ok(TranslationResult {
                translated_text,
                source_language,
            }),
            Some(FakeProviderResponse::Failure { message }) => Err(anyhow!(message)),
            Some(FakeProviderResponse::Wait { notify, next }) => {
                notify.notified().await;
                match *next {
                    FakeProviderResponse::Success {
                        translated_text,
                        source_language,
                    } => Ok(TranslationResult {
                        translated_text,
                        source_language,
                    }),
                    FakeProviderResponse::Failure { message } => Err(anyhow!(message)),
                    FakeProviderResponse::Wait { .. } => {
                        Err(anyhow!("nested wait fake response is unsupported"))
                    }
                }
            }
            None => Ok(TranslationResult {
                translated_text: format!("ZH::{}", request.source_text),
                source_language: Some("en".to_string()),
            }),
        }
    }
}

#[tokio::test]
async fn translated_chunk_schema_is_available() {
    let (_app, _state, _tmp, pool, _provider) = build_test_app_with_fake_translation().await;

    let file_columns = sqlx::query("PRAGMA table_info(evidence_files)")
        .fetch_all(&pool)
        .await
        .expect("query evidence_files schema")
        .into_iter()
        .map(|r| r.get::<String, _>("name"))
        .collect::<Vec<_>>();

    for expected in [
        "translation_status",
        "translation_error",
        "source_language",
        "target_language",
        "chunk_count",
        "translated_chunk_count",
        "failed_chunk_count",
        "translation_model",
        "translation_provider",
    ] {
        assert!(
            file_columns.iter().any(|c| c == expected),
            "missing evidence_files column: {expected}"
        );
    }

    let chunk_columns = sqlx::query("PRAGMA table_info(evidence_file_chunks)")
        .fetch_all(&pool)
        .await
        .expect("query evidence_file_chunks schema")
        .into_iter()
        .map(|r| r.get::<String, _>("name"))
        .collect::<Vec<_>>();

    for expected in [
        "id",
        "evidence_id",
        "case_id",
        "chunk_index",
        "page_number",
        "segment_number",
        "chunk_kind",
        "display_label",
        "source_text",
        "char_count",
        "token_estimate",
        "anchor_json",
        "source_text_hash",
        "source_language",
        "target_language",
        "translated_text",
        "translation_status",
        "retry_count",
        "max_retries",
        "last_attempt_at",
        "next_retry_at",
        "translation_error",
        "translated_at",
        "created_at",
        "updated_at",
    ] {
        assert!(
            chunk_columns.iter().any(|c| c == expected),
            "missing evidence_file_chunks column: {expected}"
        );
    }
}

#[tokio::test]
async fn parsing_auto_starts_translation() {
    let (app, _state, _tmp, pool, _provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) =
        register_user(&app, "auto-translate", "auto-translate@example.com").await;
    let case_id = create_case(&app, &token, "Auto Translation", "auto translation").await;
    let evidence_id = upload_text_file(&app, &token, &case_id, "note.txt", "Alpha\n\nBeta\n").await;

    let detail = wait_for_file_parse_done(&app, &token, &evidence_id).await;
    assert_eq!(detail.0, StatusCode::OK);

    wait_for_file_translation_status(&pool, &evidence_id, &["done"]).await;

    let file_row = sqlx::query(
        r#"
        SELECT
            parse_status,
            translation_status,
            translation_error,
            source_language,
            translation_model,
            translation_provider,
            translated_chunk_count,
            failed_chunk_count,
            chunk_count
        FROM evidence_files
        WHERE id = ?1
        LIMIT 1
        "#,
    )
    .bind(&evidence_id)
    .fetch_one(&pool)
    .await
    .expect("fetch evidence file");

    assert_eq!(file_row.get::<String, _>("parse_status"), "done");
    assert_eq!(file_row.get::<String, _>("translation_status"), "done");
    assert_eq!(file_row.get::<Option<String>, _>("translation_error"), None);
    assert_eq!(
        file_row.get::<Option<String>, _>("source_language"),
        Some("en".to_string())
    );
    assert_eq!(
        file_row.get::<Option<String>, _>("translation_model"),
        Some("fake-legal-v1".to_string())
    );
    assert_eq!(
        file_row.get::<Option<String>, _>("translation_provider"),
        Some("fake".to_string())
    );

    let translated_chunk_count = file_row.get::<i64, _>("translated_chunk_count");
    let failed_chunk_count = file_row.get::<i64, _>("failed_chunk_count");
    let chunk_count = file_row.get::<i64, _>("chunk_count");

    assert!(translated_chunk_count >= 2);
    assert_eq!(failed_chunk_count, 0);
    assert_eq!(translated_chunk_count, chunk_count);

    let chunk_rows = sqlx::query(
        r#"
        SELECT
            chunk_index,
            source_text,
            translated_text,
            source_language,
            target_language,
            translation_status,
            retry_count
        FROM evidence_file_chunks
        WHERE evidence_id = ?1
        ORDER BY chunk_index ASC
        "#,
    )
    .bind(&evidence_id)
    .fetch_all(&pool)
    .await
    .expect("fetch translated chunks");

    assert_eq!(chunk_rows.len() as i64, chunk_count);
    for row in chunk_rows {
        let source_text = row.get::<String, _>("source_text");
        assert_eq!(row.get::<String, _>("translation_status"), "done");
        assert_eq!(row.get::<i64, _>("retry_count"), 0);
        assert_eq!(
            row.get::<Option<String>, _>("translated_text"),
            Some(format!("ZH::{source_text}"))
        );
        assert_eq!(
            row.get::<Option<String>, _>("source_language"),
            Some("en".to_string())
        );
        assert_eq!(
            row.get::<Option<String>, _>("target_language"),
            Some("zh-CN".to_string())
        );
    }
}

#[tokio::test]
async fn missing_provider_source_language_falls_back_to_heuristic() {
    let (app, _state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-source-lang-fallback",
        "translation-source-lang-fallback@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Source Lang Fallback", "heuristic fallback").await;

    provider.push_responses(
        "Alpha",
        [FakeProviderResponse::Success {
            translated_text: "ZH::Alpha".to_string(),
            source_language: None,
        }],
    );

    let evidence_id = upload_text_file(&app, &token, &case_id, "lang.txt", "Alpha\n").await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    wait_for_file_translation_status(&pool, &evidence_id, &["done"]).await;

    let chunk_row = sqlx::query(
        r#"
        SELECT source_language
        FROM evidence_file_chunks
        WHERE evidence_id = ?1
        LIMIT 1
        "#,
    )
    .bind(&evidence_id)
    .fetch_one(&pool)
    .await
    .expect("fetch chunk source language");
    assert_eq!(
        chunk_row.get::<Option<String>, _>("source_language"),
        Some("en".to_string())
    );

    let file_row = sqlx::query(
        r#"
        SELECT source_language
        FROM evidence_files
        WHERE id = ?1
        LIMIT 1
        "#,
    )
    .bind(&evidence_id)
    .fetch_one(&pool)
    .await
    .expect("fetch file source language");
    assert_eq!(
        file_row.get::<Option<String>, _>("source_language"),
        Some("en".to_string())
    );
}

#[tokio::test]
async fn parsing_persists_chunk_source_fields() {
    let (app, _state, _tmp, pool, _provider) = build_test_app_with_disabled_translation().await;
    let (_user_id, token) = register_user(&app, "chunk-persist", "chunk-persist@example.com").await;
    let case_id = create_case(&app, &token, "Chunk Persist", "task 2 regression").await;
    let evidence_id = upload_text_file(&app, &token, &case_id, "note.txt", "Alpha\n\nBeta\n").await;

    let detail = wait_for_file_parse_done(&app, &token, &evidence_id).await;
    assert_eq!(detail.0, StatusCode::OK);

    let file_row = sqlx::query(
        r#"
        SELECT
            parse_status,
            chunk_count
        FROM evidence_files
        WHERE id = ?1
        LIMIT 1
        "#,
    )
    .bind(&evidence_id)
    .fetch_one(&pool)
    .await
    .expect("fetch file row");

    assert_eq!(file_row.get::<String, _>("parse_status"), "done");

    let chunk_rows = sqlx::query(
        r#"
        SELECT
            chunk_index,
            segment_number,
            display_label,
            source_text,
            source_text_hash,
            anchor_json
        FROM evidence_file_chunks
        WHERE evidence_id = ?1
        ORDER BY chunk_index ASC
        "#,
    )
    .bind(&evidence_id)
    .fetch_all(&pool)
    .await
    .expect("fetch chunks");

    assert_eq!(chunk_rows.len(), 2);
    assert_eq!(
        file_row.get::<i64, _>("chunk_count"),
        chunk_rows.len() as i64
    );

    for (expected_index, row) in chunk_rows.iter().enumerate() {
        let source_text = row.get::<String, _>("source_text");
        assert_eq!(row.get::<i64, _>("chunk_index"), expected_index as i64);
        assert!(row.get::<i64, _>("segment_number") > 0);
        assert_eq!(
            row.get::<String, _>("source_text_hash"),
            local_source_text_hash(&source_text)
        );

        let display_label = row.get::<String, _>("display_label");
        assert!(!display_label.trim().is_empty());

        let anchor_json = row.get::<String, _>("anchor_json");
        let anchor: Value = serde_json::from_str(&anchor_json).expect("valid anchor json");
        assert_eq!(
            anchor["display_label"].as_str().unwrap_or(""),
            display_label.as_str()
        );
        assert_eq!(
            anchor["segment_number"].as_i64().unwrap_or_default(),
            row.get::<i64, _>("segment_number")
        );
    }
}

#[tokio::test]
async fn rapid_reparse_changes_parsed_at_generation_token() {
    let (app, _state, _tmp, pool, _provider) = build_test_app_with_disabled_translation().await;
    let (_user_id, token) =
        register_user(&app, "parsed-at-token", "parsed-at-token@example.com").await;
    let case_id = create_case(&app, &token, "ParsedAt Token", "rapid reparse").await;
    let evidence_id =
        upload_text_file(&app, &token, &case_id, "parsed-at.txt", "Alpha\n\nBeta\n").await;

    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    let first_parsed_at = fetch_file_parsed_at(&pool, &evidence_id).await;

    let reparse_one = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/files/{evidence_id}/parse"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(reparse_one.0, StatusCode::OK);
    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    let second_parsed_at = fetch_file_parsed_at(&pool, &evidence_id).await;

    let reparse_two = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/files/{evidence_id}/parse"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(reparse_two.0, StatusCode::OK);
    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    let third_parsed_at = fetch_file_parsed_at(&pool, &evidence_id).await;

    assert_ne!(first_parsed_at, second_parsed_at);
    assert_ne!(second_parsed_at, third_parsed_at);
}

#[tokio::test]
async fn translation_failures_aggregate_to_partial_or_failed() {
    let (app, _state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-failures",
        "translation-failures@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Failures", "failure aggregation").await;

    provider.push_responses(
        "Alpha",
        [FakeProviderResponse::Failure {
            message: "fake provider failed alpha".to_string(),
        }],
    );
    let partial_file_id =
        upload_text_file(&app, &token, &case_id, "partial.txt", "Alpha\n\nBeta\n").await;
    wait_for_file_parse_done(&app, &token, &partial_file_id).await;
    wait_for_file_translation_status(&pool, &partial_file_id, &["partial"]).await;

    let partial_file = sqlx::query(
        r#"
        SELECT
            translation_status,
            translation_error,
            translated_chunk_count,
            failed_chunk_count
        FROM evidence_files
        WHERE id = ?1
        LIMIT 1
        "#,
    )
    .bind(&partial_file_id)
    .fetch_one(&pool)
    .await
    .expect("fetch partial file");

    assert_eq!(
        partial_file.get::<String, _>("translation_status"),
        "partial"
    );
    assert_eq!(partial_file.get::<i64, _>("translated_chunk_count"), 1);
    assert_eq!(partial_file.get::<i64, _>("failed_chunk_count"), 1);
    assert!(partial_file
        .get::<Option<String>, _>("translation_error")
        .unwrap_or_default()
        .contains("fake provider failed alpha"));

    let partial_failed_chunk = sqlx::query(
        r#"
        SELECT
            translation_status,
            retry_count,
            next_retry_at,
            translation_error
        FROM evidence_file_chunks
        WHERE evidence_id = ?1 AND source_text = 'Alpha'
        LIMIT 1
        "#,
    )
    .bind(&partial_file_id)
    .fetch_one(&pool)
    .await
    .expect("fetch failed alpha chunk");

    assert_eq!(
        partial_failed_chunk.get::<String, _>("translation_status"),
        "failed"
    );
    assert_eq!(partial_failed_chunk.get::<i64, _>("retry_count"), 1);
    assert!(partial_failed_chunk
        .get::<Option<String>, _>("next_retry_at")
        .is_some());
    assert!(partial_failed_chunk
        .get::<Option<String>, _>("translation_error")
        .unwrap_or_default()
        .contains("fake provider failed alpha"));

    provider.push_responses(
        "Gamma",
        [FakeProviderResponse::Failure {
            message: "fake provider failed gamma".to_string(),
        }],
    );
    let failed_file_id = upload_text_file(&app, &token, &case_id, "failed.txt", "Gamma\n").await;
    wait_for_file_parse_done(&app, &token, &failed_file_id).await;
    wait_for_file_translation_status(&pool, &failed_file_id, &["failed"]).await;

    let failed_file = sqlx::query(
        r#"
        SELECT
            translation_status,
            translation_error,
            translated_chunk_count,
            failed_chunk_count
        FROM evidence_files
        WHERE id = ?1
        LIMIT 1
        "#,
    )
    .bind(&failed_file_id)
    .fetch_one(&pool)
    .await
    .expect("fetch failed file");

    assert_eq!(failed_file.get::<String, _>("translation_status"), "failed");
    assert_eq!(failed_file.get::<i64, _>("translated_chunk_count"), 0);
    assert_eq!(failed_file.get::<i64, _>("failed_chunk_count"), 1);
    assert!(failed_file
        .get::<Option<String>, _>("translation_error")
        .unwrap_or_default()
        .contains("fake provider failed gamma"));
}

#[tokio::test]
async fn pending_or_processing_chunks_keep_file_status_processing() {
    let notify = Arc::new(Notify::new());
    let (app, _state, _tmp, pool, provider) =
        build_test_app_with_fake_translation_concurrency(1).await;
    let (_user_id, token) =
        register_user(&app, "translation-mix", "translation-mix@example.com").await;
    let case_id = create_case(&app, &token, "Status Mix", "status priority").await;

    provider.push_responses(
        "Alpha",
        [FakeProviderResponse::Success {
            translated_text: "ZH::Alpha".to_string(),
            source_language: Some("en".to_string()),
        }],
    );
    provider.push_responses(
        "Beta",
        [FakeProviderResponse::Failure {
            message: "fake provider failed beta".to_string(),
        }],
    );
    provider.push_responses(
        "Gamma",
        [FakeProviderResponse::Wait {
            notify: notify.clone(),
            next: Box::new(FakeProviderResponse::Success {
                translated_text: "ZH::Gamma".to_string(),
                source_language: Some("en".to_string()),
            }),
        }],
    );

    let evidence_id = upload_text_file(
        &app,
        &token,
        &case_id,
        "mix.txt",
        "Alpha\n\nBeta\n\nGamma\n",
    )
    .await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;

    let processing_row = wait_for_processing_mixed_file_state(&pool, &evidence_id, 1, 1).await;
    assert_eq!(
        processing_row.get::<String, _>("translation_status"),
        "processing"
    );

    notify.notify_waiters();
    wait_for_file_translation_status(&pool, &evidence_id, &["partial"]).await;
}

#[tokio::test]
async fn third_retry_backoff_is_preserved() {
    let (app, state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-third-retry",
        "translation-third-retry@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Third Retry", "retry tiers").await;

    provider.push_responses(
        "Gamma",
        [
            FakeProviderResponse::Failure {
                message: "attempt 1".to_string(),
            },
            FakeProviderResponse::Failure {
                message: "attempt 2".to_string(),
            },
            FakeProviderResponse::Failure {
                message: "attempt 3".to_string(),
            },
            FakeProviderResponse::Failure {
                message: "attempt 4".to_string(),
            },
        ],
    );

    let evidence_id = upload_text_file(&app, &token, &case_id, "retry3.txt", "Gamma\n").await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;

    let first = wait_for_chunk_retry_count(&pool, &evidence_id, "Gamma", 1).await;
    assert_retry_delay_in_minutes(first.get("next_retry_at"), 0, 2);

    force_failed_chunk_due_now(&pool, &evidence_id).await;
    translation::run_retry_cycle_once(state.clone())
        .await
        .expect("run retry cycle 2");
    let second = wait_for_chunk_retry_count(&pool, &evidence_id, "Gamma", 2).await;
    assert_retry_delay_in_minutes(second.get("next_retry_at"), 4, 6);

    force_failed_chunk_due_now(&pool, &evidence_id).await;
    translation::run_retry_cycle_once(state.clone())
        .await
        .expect("run retry cycle 3");
    let third = wait_for_chunk_retry_count(&pool, &evidence_id, "Gamma", 3).await;
    assert_retry_delay_in_minutes(third.get("next_retry_at"), 29, 31);

    force_failed_chunk_due_now(&pool, &evidence_id).await;
    translation::run_retry_cycle_once(state.clone())
        .await
        .expect("run retry cycle 4");
    let fourth = wait_for_chunk_retry_count(&pool, &evidence_id, "Gamma", 4).await;
    assert_eq!(fourth.get::<Option<String>, _>("next_retry_at"), None);
}

#[tokio::test]
async fn successful_chunks_are_not_overwritten_on_retry() {
    let (app, state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-idempotent",
        "translation-idempotent@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Retry", "idempotent retry").await;

    provider.push_responses(
        "Alpha",
        [FakeProviderResponse::Success {
            translated_text: "ZH::Alpha::first".to_string(),
            source_language: Some("en".to_string()),
        }],
    );
    provider.push_responses(
        "Beta",
        [
            FakeProviderResponse::Failure {
                message: "fake provider failed beta".to_string(),
            },
            FakeProviderResponse::Success {
                translated_text: "ZH::Beta::retry".to_string(),
                source_language: Some("en".to_string()),
            },
        ],
    );

    let evidence_id =
        upload_text_file(&app, &token, &case_id, "retry.txt", "Alpha\n\nBeta\n").await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    wait_for_file_translation_status(&pool, &evidence_id, &["partial"]).await;

    let alpha_before = chunk_translation_text(&pool, &evidence_id, "Alpha").await;
    assert_eq!(alpha_before.as_deref(), Some("ZH::Alpha::first"));
    assert_eq!(provider.call_count("Alpha"), 1);
    assert_eq!(provider.call_count("Beta"), 1);

    sqlx::query(
        "UPDATE evidence_file_chunks SET next_retry_at = CURRENT_TIMESTAMP WHERE evidence_id = ?1 AND translation_status = 'failed'",
    )
    .bind(&evidence_id)
    .execute(&pool)
    .await
    .expect("make failed chunk retryable now");

    translation::run_retry_cycle_once(state.clone())
        .await
        .expect("run retry cycle");
    wait_for_file_translation_status(&pool, &evidence_id, &["done"]).await;

    let alpha_after = chunk_translation_text(&pool, &evidence_id, "Alpha").await;
    let beta_after = chunk_translation_text(&pool, &evidence_id, "Beta").await;

    assert_eq!(alpha_after.as_deref(), Some("ZH::Alpha::first"));
    assert_eq!(beta_after.as_deref(), Some("ZH::Beta::retry"));
    assert_eq!(provider.call_count("Alpha"), 1);
    assert_eq!(provider.call_count("Beta"), 2);
}

#[tokio::test]
async fn file_detail_and_chunk_endpoints_expose_translation_state() {
    let (app, _state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-file-detail",
        "translation-file-detail@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "File Detail", "task 4 file detail").await;

    provider.push_responses(
        "Alpha",
        [FakeProviderResponse::Success {
            translated_text: "ZH::Alpha".to_string(),
            source_language: Some("en".to_string()),
        }],
    );
    provider.push_responses(
        "Beta",
        [FakeProviderResponse::Success {
            translated_text: "ZH::Beta".to_string(),
            source_language: Some("en".to_string()),
        }],
    );

    let upload = request_multipart_text(
        &app,
        &format!("/api/v1/cases/{case_id}/files"),
        &token,
        "detail.txt",
        "Alpha\n\nBeta\n",
    )
    .await;
    assert_eq!(upload.0, StatusCode::OK);

    let upload_data = &upload.1["data"];
    let evidence_id = upload_data["id"]
        .as_str()
        .expect("upload file id")
        .to_string();
    assert_eq!(upload_data["translation_status"].as_str(), Some("pending"));
    assert!(upload_data["translation_error"].is_null());
    assert!(upload_data["source_language"].is_null());
    assert!(upload_data["target_language"].is_null());
    assert_eq!(upload_data["chunk_count"].as_i64(), Some(0));
    assert_eq!(upload_data["translated_chunk_count"].as_i64(), Some(0));
    assert_eq!(upload_data["failed_chunk_count"].as_i64(), Some(0));
    assert!(upload_data["translation_provider"].is_null());
    assert!(upload_data["translation_model"].is_null());
    assert_eq!(upload_data["translation_incomplete"].as_bool(), Some(true));

    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    wait_for_file_translation_status(&pool, &evidence_id, &["done"]).await;

    let detail = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/files/{evidence_id}"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(detail.0, StatusCode::OK);
    let detail_data = &detail.1["data"];
    assert_eq!(detail_data["translation_status"].as_str(), Some("done"));
    assert!(detail_data["translation_error"].is_null());
    assert_eq!(detail_data["source_language"].as_str(), Some("en"));
    assert_eq!(detail_data["target_language"].as_str(), Some("zh-CN"));
    assert_eq!(detail_data["chunk_count"].as_i64(), Some(2));
    assert_eq!(detail_data["translated_chunk_count"].as_i64(), Some(2));
    assert_eq!(detail_data["failed_chunk_count"].as_i64(), Some(0));
    assert_eq!(detail_data["translation_provider"].as_str(), Some("fake"));
    assert_eq!(
        detail_data["translation_model"].as_str(),
        Some("fake-legal-v1")
    );
    assert_eq!(detail_data["translation_incomplete"].as_bool(), Some(false));

    let translation = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/files/{evidence_id}/translation"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(translation.0, StatusCode::OK);
    assert_eq!(
        translation.1["data"]["translation_status"].as_str(),
        Some("done")
    );
    assert_eq!(translation.1["data"]["chunk_count"].as_i64(), Some(2));
    assert_eq!(
        translation.1["data"]["translation_incomplete"].as_bool(),
        Some(false)
    );

    let source_chunks = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/files/{evidence_id}/chunks?page=1&page_size=1&view_mode=source"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(source_chunks.0, StatusCode::OK);
    assert_eq!(source_chunks.1["data"]["total"].as_i64(), Some(2));
    assert_eq!(source_chunks.1["data"]["page"].as_i64(), Some(1));
    assert_eq!(source_chunks.1["data"]["page_size"].as_i64(), Some(1));
    assert_eq!(
        source_chunks.1["data"]["view_mode"].as_str(),
        Some("source")
    );
    let first_chunk = &source_chunks.1["data"]["items"][0];
    assert_eq!(first_chunk["chunk_index"].as_i64(), Some(0));
    assert_eq!(first_chunk["source_text"].as_str(), Some("Alpha"));
    assert!(first_chunk["translated_text"].is_null());
    assert_eq!(first_chunk["translation_status"].as_str(), Some("done"));
    assert_eq!(
        first_chunk["anchor_json"]["display_label"].as_str(),
        first_chunk["display_label"].as_str()
    );

    let translated_chunks = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/files/{evidence_id}/chunks?page=2&page_size=1&view_mode=zh"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(translated_chunks.0, StatusCode::OK);
    assert_eq!(
        translated_chunks.1["data"]["view_mode"].as_str(),
        Some("zh")
    );
    let second_chunk = &translated_chunks.1["data"]["items"][0];
    assert_eq!(second_chunk["chunk_index"].as_i64(), Some(1));
    assert!(second_chunk["source_text"].is_null());
    assert_eq!(second_chunk["translated_text"].as_str(), Some("ZH::Beta"));

    let bilingual_chunks = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/files/{evidence_id}/chunks?view_mode=bilingual"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(bilingual_chunks.0, StatusCode::OK);
    assert_eq!(
        bilingual_chunks.1["data"]["items"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        bilingual_chunks.1["data"]["items"][0]["source_text"].as_str(),
        Some("Alpha")
    );
    assert_eq!(
        bilingual_chunks.1["data"]["items"][0]["translated_text"].as_str(),
        Some("ZH::Alpha")
    );

    let case_files = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/files"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(case_files.0, StatusCode::OK);
    let listed = &case_files.1["data"]["files"][0];
    assert_eq!(listed["id"].as_str(), Some(evidence_id.as_str()));
    assert_eq!(listed["translation_status"].as_str(), Some("done"));
    assert_eq!(listed["chunk_count"].as_i64(), Some(2));
    assert_eq!(listed["translated_chunk_count"].as_i64(), Some(2));
    assert_eq!(listed["failed_chunk_count"].as_i64(), Some(0));
    assert_eq!(listed["translation_incomplete"].as_bool(), Some(false));
}

#[tokio::test]
async fn evidence_search_supports_zh_source_and_bilingual_modes() {
    let (app, _state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-search-modes",
        "translation-search-modes@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Search Modes", "evidence search modes").await;

    provider.push_responses(
        "Alpha",
        [FakeProviderResponse::Success {
            translated_text: "阿尔法证据".to_string(),
            source_language: Some("en".to_string()),
        }],
    );
    provider.push_responses(
        "Beta",
        [FakeProviderResponse::Success {
            translated_text: "贝塔证据".to_string(),
            source_language: Some("en".to_string()),
        }],
    );

    let evidence_id =
        upload_text_file(&app, &token, &case_id, "modes.txt", "Alpha\n\nBeta\n").await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    wait_for_file_translation_status(&pool, &evidence_id, &["done"]).await;

    let source = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/search/evidence?keyword=Alpha&case_id={case_id}&language_mode=source"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(source.0, StatusCode::OK);
    let source_hit = find_evidence_result(&source.1["data"]["results"], &evidence_id);
    assert_eq!(source_hit["language_mode"].as_str(), Some("source"));
    assert_eq!(source_hit["matched_language"].as_str(), Some("source"));
    assert_eq!(source_hit["source_fallback"].as_bool(), Some(false));
    assert_eq!(source_hit["translation_incomplete"].as_bool(), Some(false));
    assert_eq!(source_hit["file_id"].as_str(), Some(evidence_id.as_str()));
    assert_eq!(source_hit["file_name"].as_str(), Some("modes.txt"));
    assert!(source_hit["chunk_id"].is_string());
    assert_eq!(source_hit["chunk_index"].as_i64(), Some(0));
    assert!(source_hit["display_label"].as_str().unwrap_or("").len() > 0);
    assert!(source_hit["anchor_json"].is_object());
    assert_eq!(source_hit["snippet_source"].as_str(), Some("Alpha"));
    assert_eq!(
        source_hit["snippet_translated"].as_str(),
        Some("阿尔法证据")
    );
    assert_eq!(source_hit["match_start_offset"].as_i64(), Some(0));
    assert_eq!(source_hit["match_end_offset"].as_i64(), Some(5));

    let zh = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/search/evidence?keyword=阿尔法&case_id={case_id}&language_mode=zh"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(zh.0, StatusCode::OK);
    let zh_hit = find_evidence_result(&zh.1["data"]["results"], &evidence_id);
    assert_eq!(zh_hit["language_mode"].as_str(), Some("zh"));
    assert_eq!(zh_hit["matched_language"].as_str(), Some("translated"));
    assert_eq!(zh_hit["source_fallback"].as_bool(), Some(false));
    assert_eq!(zh_hit["translation_incomplete"].as_bool(), Some(false));
    assert_eq!(zh_hit["snippet_source"].as_str(), Some("Alpha"));
    assert_eq!(zh_hit["snippet_translated"].as_str(), Some("阿尔法证据"));
    assert_eq!(zh_hit["match_start_offset"].as_i64(), Some(0));
    assert_eq!(zh_hit["match_end_offset"].as_i64(), Some(3));

    let bilingual = request_json(
        &app,
        Method::GET,
        &format!(
            "/api/v1/search/evidence?keyword=阿尔法&case_id={case_id}&language_mode=bilingual"
        ),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(bilingual.0, StatusCode::OK);
    let bilingual_hit = find_evidence_result(&bilingual.1["data"]["results"], &evidence_id);
    assert_eq!(bilingual_hit["language_mode"].as_str(), Some("bilingual"));
    assert_eq!(
        bilingual_hit["matched_language"].as_str(),
        Some("translated")
    );
    assert_eq!(bilingual_hit["source_fallback"].as_bool(), Some(false));
    assert_eq!(bilingual_hit["snippet_source"].as_str(), Some("Alpha"));
    assert_eq!(
        bilingual_hit["snippet_translated"].as_str(),
        Some("阿尔法证据")
    );
}

#[tokio::test]
async fn global_search_accepts_language_mode_for_evidence_hits() {
    let (app, _state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-global-search",
        "translation-global-search@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Global Search", "language mode passthrough").await;

    provider.push_responses(
        "Alpha",
        [FakeProviderResponse::Success {
            translated_text: "阿尔法法庭记录".to_string(),
            source_language: Some("en".to_string()),
        }],
    );

    let evidence_id = upload_text_file(&app, &token, &case_id, "global.txt", "Alpha\n").await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    wait_for_file_translation_status(&pool, &evidence_id, &["done"]).await;

    let search = request_json(
        &app,
        Method::GET,
        "/api/v1/search?keyword=阿尔法&language_mode=zh",
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(search.0, StatusCode::OK);

    let hit = find_evidence_result(&search.1["data"]["results"], &evidence_id);
    assert_eq!(hit["object_type"].as_str(), Some("evidence"));
    assert_eq!(hit["language_mode"].as_str(), Some("zh"));
    assert_eq!(hit["matched_language"].as_str(), Some("translated"));
    assert_eq!(hit["source_fallback"].as_bool(), Some(false));
}

#[tokio::test]
async fn evidence_search_defaults_to_zh_mode() {
    let (app, _state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-search-default-zh",
        "translation-search-default-zh@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Default Zh", "default zh search mode").await;

    provider.push_responses(
        "Alpha",
        [FakeProviderResponse::Success {
            translated_text: "阿尔法默认搜索".to_string(),
            source_language: Some("en".to_string()),
        }],
    );

    let evidence_id = upload_text_file(&app, &token, &case_id, "default-zh.txt", "Alpha\n").await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    wait_for_file_translation_status(&pool, &evidence_id, &["done"]).await;

    let search = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/search/evidence?keyword=阿尔法&case_id={case_id}"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(search.0, StatusCode::OK);
    let hit = find_evidence_result(&search.1["data"]["results"], &evidence_id);
    assert_eq!(hit["language_mode"].as_str(), Some("zh"));
    assert_eq!(hit["matched_language"].as_str(), Some("translated"));
    assert_eq!(hit["source_fallback"].as_bool(), Some(false));
}

#[tokio::test]
async fn evidence_search_bilingual_total_dedupes_same_chunk() {
    let (app, _state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-search-bilingual-total",
        "translation-search-bilingual-total@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Bilingual Total", "dedupe same chunk").await;

    provider.push_responses(
        "Alpha",
        [FakeProviderResponse::Success {
            translated_text: "Alpha 中文".to_string(),
            source_language: Some("en".to_string()),
        }],
    );

    let evidence_id = upload_text_file(&app, &token, &case_id, "dedupe.txt", "Alpha\n").await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    wait_for_file_translation_status(&pool, &evidence_id, &["done"]).await;

    let bilingual = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/search/evidence?keyword=Alpha&case_id={case_id}&language_mode=bilingual"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(bilingual.0, StatusCode::OK);
    assert_eq!(bilingual.1["data"]["total"].as_i64(), Some(1));
    assert_eq!(
        bilingual.1["data"]["results"].as_array().map(Vec::len),
        Some(1)
    );
    let hit = find_evidence_result(&bilingual.1["data"]["results"], &evidence_id);
    assert_eq!(hit["language_mode"].as_str(), Some("bilingual"));
    assert_eq!(hit["matched_language"].as_str(), Some("translated"));
}

#[tokio::test]
async fn evidence_search_matches_file_name_for_chunked_files() {
    let (app, _state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-search-filename",
        "translation-search-filename@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Filename Search", "file name search").await;

    provider.push_responses(
        "Alpha evidence body",
        [FakeProviderResponse::Success {
            translated_text: "阿尔法正文".to_string(),
            source_language: Some("en".to_string()),
        }],
    );

    let evidence_id = upload_text_file(
        &app,
        &token,
        &case_id,
        "invoice-2026-bridge.txt",
        "Alpha evidence body\n",
    )
    .await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    wait_for_file_translation_status(&pool, &evidence_id, &["done"]).await;

    let search = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/search/evidence?keyword=invoice-2026&case_id={case_id}"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(search.0, StatusCode::OK);
    let hit = find_evidence_result(&search.1["data"]["results"], &evidence_id);
    assert_eq!(hit["file_name"].as_str(), Some("invoice-2026-bridge.txt"));
    assert_eq!(hit["source_fallback"].as_bool(), Some(false));
}

#[tokio::test]
async fn evidence_search_legacy_files_fall_back_to_parsed_text_for_source_and_bilingual() {
    let (app, _state, _tmp, pool, _provider) = build_test_app_with_disabled_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-legacy-fallback",
        "translation-legacy-fallback@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Legacy Fallback", "legacy parsed_text search").await;

    let evidence_id = upload_text_file(
        &app,
        &token,
        &case_id,
        "legacy.txt",
        "Legacy Alpha evidence line\n",
    )
    .await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;

    sqlx::query(
        r#"
        UPDATE evidence_files
        SET chunk_count = 0,
            translated_chunk_count = 0,
            failed_chunk_count = 0
        WHERE id = ?1
        "#,
    )
    .bind(&evidence_id)
    .execute(&pool)
    .await
    .expect("reset chunk counters for legacy fallback");
    sqlx::query("DELETE FROM evidence_file_chunks WHERE evidence_id = ?1")
        .bind(&evidence_id)
        .execute(&pool)
        .await
        .expect("delete chunks for legacy fallback");

    let source = request_json(
        &app,
        Method::GET,
        &format!(
            "/api/v1/search/evidence?keyword=Legacy%20Alpha&case_id={case_id}&language_mode=source"
        ),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(source.0, StatusCode::OK);
    let source_hit = find_evidence_result(&source.1["data"]["results"], &evidence_id);
    assert_eq!(source_hit["language_mode"].as_str(), Some("source"));
    assert_eq!(source_hit["source_fallback"].as_bool(), Some(true));
    assert_eq!(source_hit["matched_language"].as_str(), Some("source"));
    assert_eq!(source_hit["translation_incomplete"].as_bool(), Some(true));
    assert!(source_hit.get("chunk_id").is_some());
    assert!(source_hit.get("chunk_index").is_some());
    assert!(source_hit.get("snippet_translated").is_some());
    assert!(source_hit["chunk_id"].is_null());
    assert!(source_hit["chunk_index"].is_null());

    let bilingual = request_json(
        &app,
        Method::GET,
        &format!(
            "/api/v1/search/evidence?keyword=Legacy%20Alpha&case_id={case_id}&language_mode=bilingual"
        ),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(bilingual.0, StatusCode::OK);
    let bilingual_hit = find_evidence_result(&bilingual.1["data"]["results"], &evidence_id);
    assert_eq!(bilingual_hit["language_mode"].as_str(), Some("bilingual"));
    assert_eq!(bilingual_hit["source_fallback"].as_bool(), Some(true));
    assert_eq!(bilingual_hit["matched_language"].as_str(), Some("source"));
}

#[tokio::test]
async fn evidence_search_legacy_files_fall_back_to_parsed_text_for_zh() {
    let (app, _state, _tmp, pool, _provider) = build_test_app_with_disabled_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-legacy-zh-fallback",
        "translation-legacy-zh-fallback@example.com",
    )
    .await;
    let case_id = create_case(
        &app,
        &token,
        "Legacy Zh Fallback",
        "legacy zh parsed_text search",
    )
    .await;

    let evidence_id = upload_text_file(
        &app,
        &token,
        &case_id,
        "legacy-zh.txt",
        "Legacy source text\n",
    )
    .await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;

    sqlx::query(
        r#"
        UPDATE evidence_files
        SET chunk_count = 0,
            translated_chunk_count = 0,
            failed_chunk_count = 0
        WHERE id = ?1
        "#,
    )
    .bind(&evidence_id)
    .execute(&pool)
    .await
    .expect("reset chunk counters for legacy zh fallback");
    sqlx::query("DELETE FROM evidence_file_chunks WHERE evidence_id = ?1")
        .bind(&evidence_id)
        .execute(&pool)
        .await
        .expect("delete chunks for legacy zh fallback");

    let zh = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/search/evidence?keyword=Legacy&case_id={case_id}&language_mode=zh"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(zh.0, StatusCode::OK);
    let zh_hit = find_evidence_result(&zh.1["data"]["results"], &evidence_id);
    assert_eq!(zh_hit["language_mode"].as_str(), Some("zh"));
    assert_eq!(zh_hit["matched_language"].as_str(), Some("source"));
    assert_eq!(zh_hit["source_fallback"].as_bool(), Some(true));
    assert_eq!(zh_hit["translation_incomplete"].as_bool(), Some(true));
}

#[tokio::test]
async fn evidence_search_zh_marks_translation_incomplete_without_source_fallback() {
    let (app, _state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-search-incomplete",
        "translation-search-incomplete@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Incomplete Zh", "partial translation search").await;

    provider.push_responses(
        "Alpha",
        [FakeProviderResponse::Success {
            translated_text: "阿尔法已翻译".to_string(),
            source_language: Some("en".to_string()),
        }],
    );
    provider.push_responses(
        "Beta",
        [FakeProviderResponse::Failure {
            message: "beta translation failed".to_string(),
        }],
    );

    let evidence_id =
        upload_text_file(&app, &token, &case_id, "partial.txt", "Alpha\n\nBeta\n").await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    wait_for_file_translation_status(&pool, &evidence_id, &["partial"]).await;

    let zh = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/search/evidence?keyword=阿尔法&case_id={case_id}&language_mode=zh"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(zh.0, StatusCode::OK);
    let zh_hit = find_evidence_result(&zh.1["data"]["results"], &evidence_id);
    assert_eq!(zh_hit["language_mode"].as_str(), Some("zh"));
    assert_eq!(zh_hit["matched_language"].as_str(), Some("translated"));
    assert_eq!(zh_hit["translation_incomplete"].as_bool(), Some(true));
    assert_eq!(zh_hit["source_fallback"].as_bool(), Some(false));

    let no_source_fallback = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/search/evidence?keyword=Beta&case_id={case_id}&language_mode=zh"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(no_source_fallback.0, StatusCode::OK);
    assert!(
        find_optional_evidence_result(&no_source_fallback.1["data"]["results"], &evidence_id)
            .is_none(),
        "zh mode should not fall back to source text when translation is incomplete"
    );
}

#[tokio::test]
async fn chunks_endpoint_returns_409_when_parse_is_incomplete() {
    let (app, _state, _tmp, pool, _provider) = build_test_app_with_disabled_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-parse-incomplete",
        "translation-parse-incomplete@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Parse Incomplete", "chunks guard").await;
    let evidence_id = upload_text_file(&app, &token, &case_id, "pending.txt", "Alpha\n").await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;

    sqlx::query(
        r#"
        UPDATE evidence_files
        SET
            parse_status = 'processing',
            parsed_text = NULL,
            page_count = NULL,
            duration = NULL,
            parsed_at = NULL,
            chunk_count = 0
        WHERE id = ?1
        "#,
    )
    .bind(&evidence_id)
    .execute(&pool)
    .await
    .expect("force parse incomplete state");
    sqlx::query("DELETE FROM evidence_file_chunks WHERE evidence_id = ?1")
        .bind(&evidence_id)
        .execute(&pool)
        .await
        .expect("delete parsed chunks");

    let chunks = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/files/{evidence_id}/chunks"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(chunks.0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn retry_selected_requires_chunk_ids() {
    let (app, _state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-retry-selected",
        "translation-retry-selected@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Retry Selected", "chunk ids required").await;

    provider.push_responses(
        "Alpha",
        [FakeProviderResponse::Failure {
            message: "alpha fail".to_string(),
        }],
    );

    let evidence_id =
        upload_text_file(&app, &token, &case_id, "retry-selected.txt", "Alpha\n").await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    wait_for_file_translation_status(&pool, &evidence_id, &["failed", "partial"]).await;

    let retry = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/files/{evidence_id}/translate/retry"),
        Some(&token),
        serde_json::json!({
            "scope": "selected"
        }),
    )
    .await;
    assert_eq!(retry.0, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn retry_all_and_selected_skip_done_chunks_without_overwriting_text() {
    let (app, _state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-retry-skip-done",
        "translation-retry-skip-done@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Retry Skip Done", "retry skip done").await;

    provider.push_responses(
        "Alpha",
        [FakeProviderResponse::Success {
            translated_text: "ZH::Alpha::first".to_string(),
            source_language: Some("en".to_string()),
        }],
    );
    provider.push_responses(
        "Beta",
        [
            FakeProviderResponse::Failure {
                message: "beta first fail".to_string(),
            },
            FakeProviderResponse::Success {
                translated_text: "ZH::Beta::retry".to_string(),
                source_language: Some("en".to_string()),
            },
        ],
    );

    let evidence_id =
        upload_text_file(&app, &token, &case_id, "retry-api.txt", "Alpha\n\nBeta\n").await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    wait_for_file_translation_status(&pool, &evidence_id, &["partial"]).await;

    let chunk_rows = sqlx::query(
        r#"
        SELECT id, source_text, translation_status
        FROM evidence_file_chunks
        WHERE evidence_id = ?1
        ORDER BY chunk_index ASC
        "#,
    )
    .bind(&evidence_id)
    .fetch_all(&pool)
    .await
    .expect("fetch retry-api chunks");

    let alpha_id = chunk_rows[0].get::<String, _>("id");
    let beta_id = chunk_rows[1].get::<String, _>("id");
    assert_eq!(chunk_rows[0].get::<String, _>("translation_status"), "done");
    assert_eq!(
        chunk_rows[1].get::<String, _>("translation_status"),
        "failed"
    );

    let retry_all = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/files/{evidence_id}/translate/retry"),
        Some(&token),
        serde_json::json!({
            "scope": "all"
        }),
    )
    .await;
    assert_eq!(retry_all.0, StatusCode::OK);
    assert_eq!(retry_all.1["data"]["retried_count"].as_i64(), Some(1));
    assert_eq!(retry_all.1["data"]["skipped_count"].as_i64(), Some(1));
    assert_eq!(
        retry_all.1["data"]["skipped_chunk_ids"][0].as_str(),
        Some(alpha_id.as_str())
    );

    wait_for_file_translation_status(&pool, &evidence_id, &["done"]).await;
    assert_eq!(
        chunk_translation_text(&pool, &evidence_id, "Alpha")
            .await
            .as_deref(),
        Some("ZH::Alpha::first")
    );
    assert_eq!(
        chunk_translation_text(&pool, &evidence_id, "Beta")
            .await
            .as_deref(),
        Some("ZH::Beta::retry")
    );
    assert_eq!(provider.call_count("Alpha"), 1);
    assert_eq!(provider.call_count("Beta"), 2);

    let retry_selected = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/files/{evidence_id}/translate/retry"),
        Some(&token),
        serde_json::json!({
            "scope": "selected",
            "chunk_ids": [alpha_id, beta_id]
        }),
    )
    .await;
    assert_eq!(retry_selected.0, StatusCode::OK);
    assert_eq!(retry_selected.1["data"]["retried_count"].as_i64(), Some(0));
    assert_eq!(retry_selected.1["data"]["skipped_count"].as_i64(), Some(2));

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        chunk_translation_text(&pool, &evidence_id, "Alpha")
            .await
            .as_deref(),
        Some("ZH::Alpha::first")
    );
    assert_eq!(
        chunk_translation_text(&pool, &evidence_id, "Beta")
            .await
            .as_deref(),
        Some("ZH::Beta::retry")
    );
    assert_eq!(provider.call_count("Alpha"), 1);
    assert_eq!(provider.call_count("Beta"), 2);
}

#[tokio::test]
async fn retry_skips_processing_chunk_without_resetting_active_attempt() {
    let notify = Arc::new(Notify::new());
    let (app, _state, _tmp, pool, provider) =
        build_test_app_with_fake_translation_concurrency(1).await;
    let (_user_id, token) = register_user(
        &app,
        "translation-retry-skip-processing",
        "translation-retry-skip-processing@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Retry Skip Processing", "retry processing").await;

    provider.push_responses(
        "Alpha",
        [FakeProviderResponse::Wait {
            notify: notify.clone(),
            next: Box::new(FakeProviderResponse::Success {
                translated_text: "ZH::Alpha::processing".to_string(),
                source_language: Some("en".to_string()),
            }),
        }],
    );

    let evidence_id =
        upload_text_file(&app, &token, &case_id, "retry-processing.txt", "Alpha\n").await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;

    let processing_before = wait_for_chunk_status(&pool, &evidence_id, "Alpha", "processing").await;
    let last_attempt_before = processing_before.get::<Option<String>, _>("last_attempt_at");
    assert!(last_attempt_before.is_some());

    let retry = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/files/{evidence_id}/translate/retry"),
        Some(&token),
        serde_json::json!({
            "scope": "all"
        }),
    )
    .await;
    assert_eq!(retry.0, StatusCode::OK);
    assert_eq!(retry.1["data"]["retried_count"].as_i64(), Some(0));
    assert_eq!(retry.1["data"]["skipped_count"].as_i64(), Some(1));

    let processing_after = fetch_chunk_state(&pool, &evidence_id, "Alpha").await;
    assert_eq!(
        processing_after.get::<String, _>("translation_status"),
        "processing"
    );
    assert_eq!(
        processing_after.get::<Option<String>, _>("last_attempt_at"),
        last_attempt_before
    );
    assert_eq!(processing_after.get::<i64, _>("retry_count"), 0);

    notify.notify_waiters();
    wait_for_file_translation_status(&pool, &evidence_id, &["done"]).await;
    assert_eq!(
        chunk_translation_text(&pool, &evidence_id, "Alpha")
            .await
            .as_deref(),
        Some("ZH::Alpha::processing")
    );
    assert_eq!(provider.call_count("Alpha"), 1);
}

#[tokio::test]
async fn stale_attempt_results_do_not_overwrite_new_attempt() {
    let notify = Arc::new(Notify::new());
    let (app, state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) =
        register_user(&app, "translation-cas", "translation-cas@example.com").await;
    let case_id = create_case(&app, &token, "Attempt CAS", "stale attempt cas").await;

    provider.push_responses(
        "Alpha",
        [
            FakeProviderResponse::Wait {
                notify: notify.clone(),
                next: Box::new(FakeProviderResponse::Success {
                    translated_text: "ZH::Alpha::stale".to_string(),
                    source_language: Some("en".to_string()),
                }),
            },
            FakeProviderResponse::Success {
                translated_text: "ZH::Alpha::fresh".to_string(),
                source_language: Some("en".to_string()),
            },
        ],
    );

    let evidence_id = upload_text_file(&app, &token, &case_id, "cas.txt", "Alpha\n").await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    wait_for_chunk_status(&pool, &evidence_id, "Alpha", "processing").await;

    mark_chunk_processing_stale(&pool, &evidence_id, "Alpha").await;
    translation::run_retry_cycle_once(state.clone())
        .await
        .expect("run stale reset cycle");
    let stale_reset_row = wait_for_chunk_retry_count(&pool, &evidence_id, "Alpha", 1).await;
    assert_eq!(
        stale_reset_row.get::<String, _>("translation_status"),
        "pending"
    );

    force_failed_or_pending_chunk_due_now(&pool, &evidence_id).await;
    translation::run_retry_cycle_once(state.clone())
        .await
        .expect("run replacement attempt cycle");
    wait_for_file_translation_status(&pool, &evidence_id, &["done"]).await;

    notify.notify_waiters();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let translated = chunk_translation_text(&pool, &evidence_id, "Alpha").await;
    assert_eq!(translated.as_deref(), Some("ZH::Alpha::fresh"));
    assert_eq!(provider.call_count("Alpha"), 2);
}

#[tokio::test]
async fn stale_processing_consumes_retry_budget() {
    let notify = Arc::new(Notify::new());
    let (app, state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) = register_user(
        &app,
        "translation-stale-budget",
        "translation-stale-budget@example.com",
    )
    .await;
    let case_id = create_case(&app, &token, "Stale Budget", "stale retry budget").await;

    provider.push_responses(
        "Alpha",
        [
            FakeProviderResponse::Wait {
                notify: notify.clone(),
                next: Box::new(FakeProviderResponse::Success {
                    translated_text: "ignored-1".to_string(),
                    source_language: Some("en".to_string()),
                }),
            },
            FakeProviderResponse::Wait {
                notify: notify.clone(),
                next: Box::new(FakeProviderResponse::Success {
                    translated_text: "ignored-2".to_string(),
                    source_language: Some("en".to_string()),
                }),
            },
            FakeProviderResponse::Wait {
                notify: notify.clone(),
                next: Box::new(FakeProviderResponse::Success {
                    translated_text: "ignored-3".to_string(),
                    source_language: Some("en".to_string()),
                }),
            },
            FakeProviderResponse::Wait {
                notify: notify.clone(),
                next: Box::new(FakeProviderResponse::Success {
                    translated_text: "ignored-4".to_string(),
                    source_language: Some("en".to_string()),
                }),
            },
        ],
    );

    let evidence_id = upload_text_file(&app, &token, &case_id, "stale.txt", "Alpha\n").await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;

    for expected_retry_count in 1..=3 {
        wait_for_chunk_status(&pool, &evidence_id, "Alpha", "processing").await;
        mark_chunk_processing_stale(&pool, &evidence_id, "Alpha").await;
        translation::run_retry_cycle_once(state.clone())
            .await
            .expect("run stale budget cycle");

        let row =
            wait_for_chunk_retry_count(&pool, &evidence_id, "Alpha", expected_retry_count).await;
        assert_eq!(row.get::<String, _>("translation_status"), "pending");
        assert!(
            row.get::<Option<String>, _>("next_retry_at").is_some(),
            "retry {expected_retry_count} should schedule another retry"
        );

        force_failed_or_pending_chunk_due_now(&pool, &evidence_id).await;
        translation::run_retry_cycle_once(state.clone())
            .await
            .expect("run claim next stale attempt");
    }

    wait_for_chunk_status(&pool, &evidence_id, "Alpha", "processing").await;
    mark_chunk_processing_stale(&pool, &evidence_id, "Alpha").await;
    translation::run_retry_cycle_once(state.clone())
        .await
        .expect("run terminal stale budget cycle");

    let terminal = wait_for_chunk_retry_count(&pool, &evidence_id, "Alpha", 4).await;
    assert_eq!(terminal.get::<String, _>("translation_status"), "failed");
    assert_eq!(terminal.get::<Option<String>, _>("next_retry_at"), None);

    translation::run_retry_cycle_once(state.clone())
        .await
        .expect("run extra idle retry cycle");
    tokio::time::sleep(Duration::from_millis(100)).await;
    let still_terminal = fetch_chunk_state(&pool, &evidence_id, "Alpha").await;
    assert_eq!(
        still_terminal.get::<String, _>("translation_status"),
        "failed"
    );
    assert_eq!(still_terminal.get::<i64, _>("retry_count"), 4);

    notify.notify_waiters();
}

#[tokio::test]
async fn retry_does_not_drift_locked_provider_or_model() {
    let (app, state, _tmp, pool, provider) = build_test_app_with_fake_translation().await;
    let (_user_id, token) =
        register_user(&app, "translation-lock", "translation-lock@example.com").await;
    let case_id = create_case(&app, &token, "Provider Lock", "provider/model lock").await;

    provider.push_responses(
        "Alpha",
        [FakeProviderResponse::Success {
            translated_text: "ZH::Alpha".to_string(),
            source_language: Some("en".to_string()),
        }],
    );
    provider.push_responses(
        "Beta",
        [FakeProviderResponse::Failure {
            message: "beta first fail".to_string(),
        }],
    );

    let evidence_id = upload_text_file(&app, &token, &case_id, "lock.txt", "Alpha\n\nBeta\n").await;
    wait_for_file_parse_done(&app, &token, &evidence_id).await;
    wait_for_file_translation_status(&pool, &evidence_id, &["partial"]).await;

    force_failed_chunk_due_now(&pool, &evidence_id).await;

    let drift_provider = FakeTranslationProvider::new("fake-drift", Some("fake-legal-v2"));
    let drift_state = AppState::new_with_translation_provider(
        state.config.clone(),
        pool.clone(),
        TranslationConfig {
            provider: "fake-drift".to_string(),
            base_url: None,
            api_key: None,
            model: Some("fake-legal-v2".to_string()),
            target_language: "zh-CN".to_string(),
            max_concurrency: 1,
            chunk_size_limit: 2_000,
        },
        Arc::new(drift_provider),
    );

    translation::run_retry_cycle_once(drift_state)
        .await
        .expect("run retry cycle with drifted provider");
    wait_for_file_translation_error(&pool, &evidence_id, "provider/model mismatch").await;

    let file_row = sqlx::query(
        r#"
        SELECT
            translation_status,
            translation_error,
            translation_provider,
            translation_model
        FROM evidence_files
        WHERE id = ?1
        LIMIT 1
        "#,
    )
    .bind(&evidence_id)
    .fetch_one(&pool)
    .await
    .expect("fetch locked file row");

    assert_eq!(
        file_row.get::<Option<String>, _>("translation_provider"),
        Some("fake".to_string())
    );
    assert_eq!(
        file_row.get::<Option<String>, _>("translation_model"),
        Some("fake-legal-v1".to_string())
    );
    assert!(file_row
        .get::<Option<String>, _>("translation_error")
        .unwrap_or_default()
        .contains("provider/model mismatch"));
    assert_eq!(file_row.get::<String, _>("translation_status"), "partial");

    let failed_chunk = sqlx::query(
        r#"
        SELECT
            translation_status,
            next_retry_at,
            translation_error
        FROM evidence_file_chunks
        WHERE evidence_id = ?1 AND source_text = 'Beta'
        LIMIT 1
        "#,
    )
    .bind(&evidence_id)
    .fetch_one(&pool)
    .await
    .expect("fetch locked failed chunk");

    assert_eq!(
        failed_chunk.get::<String, _>("translation_status"),
        "failed"
    );
    assert_eq!(failed_chunk.get::<Option<String>, _>("next_retry_at"), None);
    assert!(failed_chunk
        .get::<Option<String>, _>("translation_error")
        .unwrap_or_default()
        .contains("provider/model mismatch"));
}

async fn build_test_app_with_fake_translation() -> (
    axum::Router,
    AppState,
    TempDir,
    SqlitePool,
    FakeTranslationProvider,
) {
    build_test_app_with_named_translation(
        FakeTranslationProvider::new("fake", Some("fake-legal-v1")),
        TranslationConfig {
            provider: "fake".to_string(),
            base_url: None,
            api_key: None,
            model: Some("fake-legal-v1".to_string()),
            target_language: "zh-CN".to_string(),
            max_concurrency: 1,
            chunk_size_limit: 2_000,
        },
    )
    .await
}

async fn build_test_app_with_fake_translation_concurrency(
    max_concurrency: u16,
) -> (
    axum::Router,
    AppState,
    TempDir,
    SqlitePool,
    FakeTranslationProvider,
) {
    build_test_app_with_named_translation(
        FakeTranslationProvider::new("fake", Some("fake-legal-v1")),
        TranslationConfig {
            provider: "fake".to_string(),
            base_url: None,
            api_key: None,
            model: Some("fake-legal-v1".to_string()),
            target_language: "zh-CN".to_string(),
            max_concurrency,
            chunk_size_limit: 2_000,
        },
    )
    .await
}

async fn build_test_app_with_disabled_translation() -> (
    axum::Router,
    AppState,
    TempDir,
    SqlitePool,
    FakeTranslationProvider,
) {
    build_test_app_with_named_translation(
        FakeTranslationProvider::new("disabled", None),
        TranslationConfig {
            provider: "disabled".to_string(),
            base_url: None,
            api_key: None,
            model: None,
            target_language: "zh-CN".to_string(),
            max_concurrency: 1,
            chunk_size_limit: 2_000,
        },
    )
    .await
}

async fn build_test_app_with_named_translation(
    provider: FakeTranslationProvider,
    translation: TranslationConfig,
) -> (
    axum::Router,
    AppState,
    TempDir,
    SqlitePool,
    FakeTranslationProvider,
) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite memory");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("pragma foreign_keys");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");

    let tmp = TempDir::new().expect("tempdir");
    let storage_path = tmp.path().join("storage");
    let temp_path = storage_path.join("temp");
    let tessdata_dir = tmp.path().join("tessdata");

    tokio::fs::create_dir_all(&storage_path)
        .await
        .expect("create storage");
    tokio::fs::create_dir_all(&temp_path)
        .await
        .expect("create temp");
    tokio::fs::create_dir_all(&tessdata_dir)
        .await
        .expect("create tessdata");

    let cfg = AppConfig {
        app_env: AppEnv::Test,
        server_host: "127.0.0.1".to_string(),
        server_port: 0,
        database_url: "sqlite::memory:".to_string(),
        cors_origins: CorsOrigins::Any,
        force_https: false,
        trust_proxy_headers: false,
        jwt_secret: "test-secret-please-change-32-chars-min".to_string(),
        jwt_secret_old: None,
        access_token_expire_minutes: 60,
        refresh_token_expire_days: 7,
        storage_path: storage_path.to_string_lossy().to_string(),
        max_file_size: 10 * 1024 * 1024,
        allowed_file_types: vec![
            "txt".to_string(),
            "json".to_string(),
            "xlsx".to_string(),
            "docx".to_string(),
            "doc".to_string(),
        ],
        temp_path: temp_path.to_string_lossy().to_string(),
        tessdata_dir: tessdata_dir.to_string_lossy().to_string(),
        whisper_model_path: tmp.path().join("whisper.bin").to_string_lossy().to_string(),
        asr_language: "zh".to_string(),
        asr_threads: 1,
    };

    let state =
        AppState::new_with_translation_provider(cfg, pool, translation, Arc::new(provider.clone()));
    let pool = state.pool.clone();
    let app = router(state.clone());
    (app, state, tmp, pool, provider)
}

async fn wait_for_file_translation_status(
    pool: &SqlitePool,
    file_id: &str,
    expected_statuses: &[&str],
) -> Value {
    for _ in 0..100 {
        let row = sqlx::query(
            r#"
            SELECT
                translation_status,
                translation_error,
                translated_chunk_count,
                failed_chunk_count
            FROM evidence_files
            WHERE id = ?1
            LIMIT 1
            "#,
        )
        .bind(file_id)
        .fetch_one(pool)
        .await
        .expect("fetch file translation row");

        let status = row.get::<String, _>("translation_status");
        if expected_statuses.iter().any(|expected| *expected == status) {
            return serde_json::json!({
                "translation_status": status,
                "translation_error": row.get::<Option<String>, _>("translation_error"),
                "translated_chunk_count": row.get::<i64, _>("translated_chunk_count"),
                "failed_chunk_count": row.get::<i64, _>("failed_chunk_count"),
            });
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let row = sqlx::query(
        r#"
        SELECT
            translation_status,
            translation_error,
            translated_chunk_count,
            failed_chunk_count
        FROM evidence_files
        WHERE id = ?1
        LIMIT 1
        "#,
    )
    .bind(file_id)
    .fetch_one(pool)
    .await
    .expect("fetch final file translation row");

    panic!(
        "translation_status for {file_id} did not reach {:?}, got {}",
        expected_statuses,
        row.get::<String, _>("translation_status")
    );
}

async fn chunk_translation_text(
    pool: &SqlitePool,
    evidence_id: &str,
    source_text: &str,
) -> Option<String> {
    sqlx::query(
        "SELECT translated_text FROM evidence_file_chunks WHERE evidence_id = ?1 AND source_text = ?2 LIMIT 1",
    )
    .bind(evidence_id)
    .bind(source_text)
    .fetch_one(pool)
    .await
    .expect("fetch chunk translated text")
    .get::<Option<String>, _>("translated_text")
}

async fn wait_for_processing_mixed_file_state(
    pool: &SqlitePool,
    evidence_id: &str,
    expected_done: i64,
    expected_failed: i64,
) -> sqlx::sqlite::SqliteRow {
    for _ in 0..100 {
        let row = sqlx::query(
            r#"
            SELECT
                f.translation_status AS translation_status,
                SUM(CASE WHEN c.translation_status = 'pending' THEN 1 ELSE 0 END) AS pending_count,
                SUM(CASE WHEN c.translation_status = 'processing' THEN 1 ELSE 0 END) AS processing_count,
                SUM(CASE WHEN c.translation_status = 'done' THEN 1 ELSE 0 END) AS done_count,
                SUM(CASE WHEN c.translation_status = 'failed' THEN 1 ELSE 0 END) AS failed_count
            FROM evidence_files f
            JOIN evidence_file_chunks c ON c.evidence_id = f.id
            WHERE f.id = ?1
            GROUP BY f.id
            LIMIT 1
            "#,
        )
        .bind(evidence_id)
        .fetch_one(pool)
        .await
        .expect("fetch mixed processing row");

        let done_count = row.get::<i64, _>("done_count");
        let failed_count = row.get::<i64, _>("failed_count");
        let pending_count = row.get::<i64, _>("pending_count");
        let processing_count = row.get::<i64, _>("processing_count");
        if done_count == expected_done
            && failed_count == expected_failed
            && (pending_count > 0 || processing_count > 0)
        {
            return row;
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    panic!("file {evidence_id} did not reach mixed processing state");
}

async fn force_failed_chunk_due_now(pool: &SqlitePool, evidence_id: &str) {
    sqlx::query(
        "UPDATE evidence_file_chunks SET next_retry_at = CURRENT_TIMESTAMP WHERE evidence_id = ?1 AND translation_status = 'failed'",
    )
    .bind(evidence_id)
    .execute(pool)
    .await
    .expect("force failed chunk due now");
}

async fn wait_for_chunk_retry_count(
    pool: &SqlitePool,
    evidence_id: &str,
    source_text: &str,
    expected_retry_count: i64,
) -> sqlx::sqlite::SqliteRow {
    for _ in 0..100 {
        let row = sqlx::query(
            r#"
            SELECT
                translation_status,
                retry_count,
                next_retry_at
            FROM evidence_file_chunks
            WHERE evidence_id = ?1 AND source_text = ?2
            LIMIT 1
            "#,
        )
        .bind(evidence_id)
        .bind(source_text)
        .fetch_one(pool)
        .await
        .expect("fetch retry row");

        if row.get::<i64, _>("retry_count") == expected_retry_count {
            return row;
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    panic!(
        "chunk {source_text} in file {evidence_id} did not reach retry_count={expected_retry_count}"
    );
}

fn assert_retry_delay_in_minutes(
    next_retry_at: Option<String>,
    min_minutes: i64,
    max_minutes: i64,
) {
    let next_retry_at = next_retry_at.expect("next_retry_at should exist");
    let parsed = chrono::NaiveDateTime::parse_from_str(&next_retry_at, "%Y-%m-%d %H:%M:%S")
        .expect("parse next_retry_at");
    let now = chrono::Utc::now().naive_utc();
    let delay_minutes = (parsed - now).num_minutes();
    assert!(
        (min_minutes..=max_minutes).contains(&delay_minutes),
        "expected retry delay between {min_minutes} and {max_minutes} minutes, got {delay_minutes} from {next_retry_at}"
    );
}

fn local_source_text_hash(text: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

async fn wait_for_file_translation_error(pool: &SqlitePool, evidence_id: &str, needle: &str) {
    for _ in 0..100 {
        let error =
            sqlx::query("SELECT translation_error FROM evidence_files WHERE id = ?1 LIMIT 1")
                .bind(evidence_id)
                .fetch_one(pool)
                .await
                .expect("fetch translation_error")
                .get::<Option<String>, _>("translation_error")
                .unwrap_or_default();

        if error.contains(needle) {
            return;
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    panic!("file {evidence_id} did not reach translation_error containing {needle}");
}

async fn fetch_file_parsed_at(pool: &SqlitePool, evidence_id: &str) -> String {
    sqlx::query("SELECT parsed_at FROM evidence_files WHERE id = ?1 LIMIT 1")
        .bind(evidence_id)
        .fetch_one(pool)
        .await
        .expect("fetch parsed_at")
        .get::<String, _>("parsed_at")
}

async fn wait_for_chunk_status(
    pool: &SqlitePool,
    evidence_id: &str,
    source_text: &str,
    expected_status: &str,
) -> sqlx::sqlite::SqliteRow {
    for _ in 0..100 {
        let row = fetch_chunk_state(pool, evidence_id, source_text).await;
        if row.get::<String, _>("translation_status") == expected_status {
            return row;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    panic!("chunk {source_text} in file {evidence_id} did not reach status={expected_status}");
}

async fn fetch_chunk_state(
    pool: &SqlitePool,
    evidence_id: &str,
    source_text: &str,
) -> sqlx::sqlite::SqliteRow {
    sqlx::query(
        r#"
        SELECT
            translation_status,
            retry_count,
            next_retry_at,
            translated_text,
            last_attempt_at
        FROM evidence_file_chunks
        WHERE evidence_id = ?1 AND source_text = ?2
        LIMIT 1
        "#,
    )
    .bind(evidence_id)
    .bind(source_text)
    .fetch_one(pool)
    .await
    .expect("fetch chunk state")
}

async fn mark_chunk_processing_stale(pool: &SqlitePool, evidence_id: &str, source_text: &str) {
    sqlx::query(
        r#"
        UPDATE evidence_file_chunks
        SET last_attempt_at = '2000-01-01 00:00:00'
        WHERE evidence_id = ?1 AND source_text = ?2
        "#,
    )
    .bind(evidence_id)
    .bind(source_text)
    .execute(pool)
    .await
    .expect("mark chunk processing stale");
}

async fn force_failed_or_pending_chunk_due_now(pool: &SqlitePool, evidence_id: &str) {
    sqlx::query(
        r#"
        UPDATE evidence_file_chunks
        SET next_retry_at = CURRENT_TIMESTAMP
        WHERE evidence_id = ?1
          AND translation_status IN ('failed', 'pending')
        "#,
    )
    .bind(evidence_id)
    .execute(pool)
    .await
    .expect("force chunk due now");
}

fn find_evidence_result<'a>(results: &'a Value, evidence_id: &str) -> &'a Value {
    find_optional_evidence_result(results, evidence_id)
        .unwrap_or_else(|| panic!("expected evidence result for {evidence_id}: {results}"))
}

fn find_optional_evidence_result<'a>(results: &'a Value, evidence_id: &str) -> Option<&'a Value> {
    results.as_array().and_then(|items| {
        items.iter().find(|item| {
            item.get("object_type").and_then(Value::as_str) == Some("evidence")
                && item
                    .get("file_id")
                    .or_else(|| item.get("object_id"))
                    .and_then(Value::as_str)
                    == Some(evidence_id)
        })
    })
}
