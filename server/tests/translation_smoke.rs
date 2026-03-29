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
        "evidence_file_id",
        "chunk_index",
        "display_label",
        "source_text",
        "source_text_hash",
        "translated_text",
        "translation_status",
        "retry_count",
        "last_retry_at",
        "translation_error",
        "created_at",
        "updated_at",
    ] {
        assert!(
            chunk_columns.iter().any(|c| c == expected),
            "missing evidence_file_chunks column: {expected}"
        );
    }
}
