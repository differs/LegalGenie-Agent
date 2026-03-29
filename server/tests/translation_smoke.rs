mod test_support;

use sqlx::Row;
use test_support::build_test_app_with_pool;

#[tokio::test]
async fn translated_chunk_schema_is_available() {
    let (_app, _tmp, pool) = build_test_app_with_pool().await;

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
