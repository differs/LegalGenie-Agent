mod test_support;

use axum::http::{Method, StatusCode};
use serde_json::Value;
use sqlx::Row;
use test_support::{
    build_test_app_with_pool, create_case, register_user, request_json, upload_text_file,
    wait_for_file_parse_done,
};

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

#[tokio::test]
async fn parsing_creates_chunks_for_uploaded_file() {
    let (app, _tmp, pool) = build_test_app_with_pool().await;
    let (_user_id, token) = register_user(&app, "chunk-user", "chunk-user@example.com").await;
    let case_id = create_case(&app, &token, "Chunk Case", "chunk parse checks").await;
    let evidence_id = upload_text_file(&app, &token, &case_id, "note.txt", "Alpha\n\nBeta\n").await;

    let detail = wait_for_file_parse_done(&app, &token, &evidence_id).await;
    assert_eq!(detail.0, axum::http::StatusCode::OK);
    assert_eq!(
        detail.1["data"]["parse_status"].as_str().unwrap_or(""),
        "done",
        "expected parse_status=done"
    );

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
    assert_eq!(file_row.get::<String, _>("translation_status"), "pending");
    assert_eq!(
        file_row.get::<Option<String>, _>("translation_error"),
        None,
        "translation_error should be reset to NULL"
    );
    assert_eq!(
        file_row.get::<Option<String>, _>("source_language"),
        None,
        "source_language should be reset to NULL"
    );
    assert_eq!(
        file_row.get::<Option<String>, _>("translation_model"),
        None,
        "translation_model should be reset to NULL"
    );
    assert_eq!(
        file_row.get::<Option<String>, _>("translation_provider"),
        None,
        "translation_provider should be reset to NULL"
    );
    assert_eq!(file_row.get::<i64, _>("translated_chunk_count"), 0);
    assert_eq!(file_row.get::<i64, _>("failed_chunk_count"), 0);

    let chunk_rows = sqlx::query(
        r#"
        SELECT
            chunk_index,
            segment_number,
            chunk_kind,
            display_label,
            source_text,
            anchor_json,
            translation_status
        FROM evidence_file_chunks
        WHERE evidence_id = ?1
        ORDER BY chunk_index ASC
        "#,
    )
    .bind(&evidence_id)
    .fetch_all(&pool)
    .await
    .expect("fetch file chunks");

    assert!(
        chunk_rows.len() >= 2,
        "expected at least two chunks for Alpha/Beta paragraphs, got {}",
        chunk_rows.len()
    );

    let mut combined_source = String::new();
    for (expected_index, row) in chunk_rows.iter().enumerate() {
        assert_eq!(
            row.get::<i64, _>("chunk_index"),
            expected_index as i64,
            "chunk_index must be sequential and ascending"
        );
        assert!(
            row.get::<i64, _>("segment_number") > 0,
            "text chunk segment_number must be > 0"
        );
        assert!(
            !row.get::<String, _>("chunk_kind").trim().is_empty(),
            "chunk_kind should not be empty"
        );
        assert!(
            !row.get::<String, _>("display_label").trim().is_empty(),
            "display_label should not be empty"
        );
        let source_text = row.get::<String, _>("source_text");
        assert!(
            !source_text.trim().is_empty(),
            "source_text should not be empty"
        );
        combined_source.push_str(&source_text);
        combined_source.push('\n');

        let anchor_raw = row.get::<String, _>("anchor_json");
        assert!(
            !anchor_raw.trim().is_empty(),
            "anchor_json should not be empty"
        );
        let anchor: Value = serde_json::from_str(&anchor_raw).expect("anchor_json must be valid");
        assert!(
            anchor
                .get("locator_type")
                .and_then(|v| v.as_str())
                .is_some(),
            "anchor_json missing locator_type"
        );
        assert!(
            anchor
                .get("display_label")
                .and_then(|v| v.as_str())
                .is_some(),
            "anchor_json missing display_label"
        );
        assert!(
            anchor
                .get("segment_number")
                .and_then(|v| v.as_i64())
                .is_some(),
            "anchor_json missing segment_number"
        );

        assert_eq!(row.get::<String, _>("translation_status"), "pending");
    }

    assert!(
        combined_source.contains("Alpha") && combined_source.contains("Beta"),
        "chunk source_text should include original paragraphs"
    );
    let initial_chunk_count = chunk_rows.len() as i64;
    assert_eq!(
        file_row.get::<i64, _>("chunk_count"),
        initial_chunk_count,
        "file.chunk_count should match actual chunk rows"
    );

    sqlx::query(
        r#"
        UPDATE evidence_files
        SET
            translation_status = 'failed',
            translation_error = 'stale error',
            source_language = 'en',
            translation_model = 'old-model',
            translation_provider = 'old-provider',
            translated_chunk_count = 7,
            failed_chunk_count = 1
        WHERE id = ?1
        "#,
    )
    .bind(&evidence_id)
    .execute(&pool)
    .await
    .expect("mark file stale before reparse");

    let stale_chunk_id = "stale-chunk-for-reparse";
    sqlx::query(
        r#"
        INSERT INTO evidence_file_chunks (
            id,
            evidence_id,
            case_id,
            chunk_index,
            page_number,
            segment_number,
            chunk_kind,
            display_label,
            source_text,
            char_count,
            token_estimate,
            anchor_json,
            source_text_hash,
            translation_status
        )
        VALUES (?1, ?2, ?3, 999, 0, 999, 'text', 'stale', 'stale chunk', 11, 3, ?4, 'stalehash', 'done')
        "#,
    )
    .bind(stale_chunk_id)
    .bind(&evidence_id)
    .bind(&case_id)
    .bind(serde_json::json!({
        "locator_type": "segment",
        "display_label": "stale",
        "segment_number": 999
    }).to_string())
    .execute(&pool)
    .await
    .expect("insert stale chunk before reparse");

    let reparse = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/files/{evidence_id}/parse"),
        Some(&token),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(reparse.0, StatusCode::OK, "reparse trigger should succeed");
    let reparsed_detail = wait_for_file_parse_done(&app, &token, &evidence_id).await;
    assert_eq!(reparsed_detail.0, StatusCode::OK);
    assert_eq!(
        reparsed_detail.1["data"]["parse_status"]
            .as_str()
            .unwrap_or(""),
        "done",
        "expected parse_status=done after reparse"
    );

    let stale_chunk_count: i64 =
        sqlx::query("SELECT COUNT(1) AS cnt FROM evidence_file_chunks WHERE id = ?1")
            .bind(stale_chunk_id)
            .fetch_one(&pool)
            .await
            .expect("count stale chunk")
            .get("cnt");
    assert_eq!(
        stale_chunk_count, 0,
        "stale chunk should be deleted during reparse"
    );

    let file_row_after = sqlx::query(
        r#"
        SELECT
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
    .expect("fetch file row after reparse");

    assert_eq!(
        file_row_after.get::<String, _>("translation_status"),
        "pending"
    );
    assert_eq!(
        file_row_after.get::<Option<String>, _>("translation_error"),
        None
    );
    assert_eq!(
        file_row_after.get::<Option<String>, _>("source_language"),
        None
    );
    assert_eq!(
        file_row_after.get::<Option<String>, _>("translation_model"),
        None
    );
    assert_eq!(
        file_row_after.get::<Option<String>, _>("translation_provider"),
        None
    );
    assert_eq!(file_row_after.get::<i64, _>("translated_chunk_count"), 0);
    assert_eq!(file_row_after.get::<i64, _>("failed_chunk_count"), 0);

    let chunk_count_after: i64 =
        sqlx::query("SELECT COUNT(1) AS cnt FROM evidence_file_chunks WHERE evidence_id = ?1")
            .bind(&evidence_id)
            .fetch_one(&pool)
            .await
            .expect("count chunks after reparse")
            .get("cnt");
    assert_eq!(
        chunk_count_after, initial_chunk_count,
        "reparse should replace chunks, not append extra chunks"
    );
    assert_eq!(
        file_row_after.get::<i64, _>("chunk_count"),
        chunk_count_after,
        "file.chunk_count should match chunks after reparse"
    );
}
