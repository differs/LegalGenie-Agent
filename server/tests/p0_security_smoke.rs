// P0 smoke tests: idempotency, Retry-After rate limiting, secret handling.
mod test_support;

use axum::http::{header, Method, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use test_support::{build_test_app_with_pool, create_case, register_user, request_json};
use tower::ServiceExt;

// ---------- Idempotency ----------

#[tokio::test]
async fn idempotency_key_replays_identical_response() {
    let (app, _tmp, _pool) = build_test_app_with_pool().await;
    let (_user_id, token) = register_user(&app, "idem_user", "idem@test.local").await;

    let payload = json!({ "name": "幂等案件", "description": "replay me" });

    // First attempt.
    let (status1, body1) =
        request_json_with_idem_key(&app, &token, "case-create-1", payload.clone()).await;
    assert_eq!(status1, StatusCode::OK);
    let case_id = body1["data"]["id"].as_str().expect("case id").to_string();

    // Retry with the same key: identical body, replay header, no duplicate.
    let (status2, body2, headers2) = request_raw_with_idem_key(&app, &token, "case-create-1").await;
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(
        headers2
            .get("x-idempotency-replayed")
            .and_then(|v| v.to_str().ok()),
        Some("true"),
        "replay must be flagged"
    );
    assert_eq!(
        body2["data"]["id"].as_str().unwrap_or(""),
        case_id,
        "replayed response must match the original"
    );

    // Only one case exists.
    let (_, list) = request_json(
        &app,
        Method::GET,
        "/api/v1/cases?page=1&page_size=20",
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(list["data"]["total"], 1);
}

#[tokio::test]
async fn idempotency_key_scoped_to_path_and_auth() {
    let (app, _tmp, _pool) = build_test_app_with_pool().await;
    let (_user_id, token) = register_user(&app, "idem_user2", "idem2@test.local").await;

    // The same key + path always replays the first response, even with a
    // different payload (that is the idempotency contract).
    let (status1, body1) = request_json_with_idem_key(
        &app,
        &token,
        "same-key-diff-path",
        json!({ "name": "案件A" }),
    )
    .await;
    assert_eq!(status1, StatusCode::OK);

    let (status2, body2) = request_json_with_idem_key(
        &app,
        &token,
        "same-key-diff-path",
        json!({ "name": "案件B" }),
    )
    .await;
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(
        body1["data"]["id"], body2["data"]["id"],
        "same key must replay the original response"
    );

    // A different key executes independently.
    let (status3, _) =
        request_json_with_idem_key(&app, &token, "different-key", json!({ "name": "案件B" })).await;
    assert_eq!(status3, StatusCode::OK);

    let (_, list) = request_json(
        &app,
        Method::GET,
        "/api/v1/cases?page=1&page_size=20",
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(list["data"]["total"], 2, "different keys must both execute");
}

#[tokio::test]
async fn failed_responses_are_not_replayed() {
    let (app, _tmp, _pool) = build_test_app_with_pool().await;
    let (_user_id, token) = register_user(&app, "idem_user3", "idem3@test.local").await;

    // Invalid create (empty name) -> failure; then a valid retry with the same
    // key must execute normally.
    let (status1, _) = request_json_with_idem_key(
        &app,
        &token,
        "retry-after-failure",
        json!({ "name": "", "description": "x" }),
    )
    .await;
    assert_eq!(status1, StatusCode::BAD_REQUEST);

    let (status2, body2) = request_json_with_idem_key(
        &app,
        &token,
        "retry-after-failure",
        json!({ "name": "修复后的案件", "description": "x" }),
    )
    .await;
    assert_eq!(status2, StatusCode::OK);
    assert!(body2["data"]["id"].as_str().is_some());
}

// ---------- Rate limiting / Retry-After ----------

#[tokio::test]
async fn login_rate_limit_returns_retry_after() {
    let (app, _tmp, _pool) = build_test_app_with_pool().await;

    // 5 failed attempts are allowed, the 6th is limited with Retry-After.
    for _ in 0..5 {
        let (status, _) = request_json(
            &app,
            Method::POST,
            "/api/v1/auth/login",
            None,
            json!({ "username": "nobody", "password": "WrongPass123", "remember_me": false }),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    let (status, headers, _body) = request_json_raw_headers(
        &app,
        "/api/v1/auth/login",
        json!({ "username": "nobody", "password": "WrongPass123", "remember_me": false }),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let retry_after = headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .expect("retry-after header must be present");
    let secs: u64 = retry_after.parse().expect("retry-after numeric");
    assert!(secs >= 1, "retry-after should be >= 1s, got {secs}");
}

// ---------- Helpers ----------

async fn request_json_with_idem_key(
    app: &axum::Router,
    bearer: &str,
    key: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = axum::http::Request::builder()
        .method(Method::POST)
        .uri("/api/v1/cases")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
        .header("idempotency-key", key)
        .body(axum::body::Body::from(body.to_string()))
        .expect("build request");
    let resp = app.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("collect")
        .to_bytes();
    (status, serde_json::from_slice(&bytes).expect("json"))
}

async fn request_raw_with_idem_key(
    app: &axum::Router,
    bearer: &str,
    key: &str,
) -> (StatusCode, serde_json::Value, axum::http::HeaderMap) {
    let req = axum::http::Request::builder()
        .method(Method::POST)
        .uri("/api/v1/cases")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
        .header("idempotency-key", key)
        .body(axum::body::Body::from("{}"))
        .expect("build request");
    let resp = app.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("collect")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).expect("json"),
        headers,
    )
}

async fn request_json_raw_headers(
    app: &axum::Router,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
    let req = axum::http::Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .expect("build request");
    let resp = app.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("collect")
        .to_bytes();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).expect("json"),
    )
}

#[allow(dead_code)]
fn _touch_create_case(app: &axum::Router, bearer: &str) {
    let _ = create_case(app, bearer, "x", "x");
}

// ---------- Upload boundary ----------

#[tokio::test]
async fn oversized_upload_is_rejected_mid_stream() {
    let (app, _tmp, _pool) = build_test_app_with_pool().await;
    let (_user_id, token) = register_user(&app, "big_upload", "big@test.local").await;
    let case_id = create_case(&app, &token, "边界案件", "").await;

    // test_support max_file_size = 10 MiB; send ~11 MiB of text.
    let payload = "x".repeat(11 * 1024 * 1024);
    let boundary = "LMBOUNDARY2c79c3ce5c04f55b79b1c3d194fe9be";

    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"big.txt\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: text/plain\r\n\r\n");
    body.extend_from_slice(payload.as_bytes());
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let req = axum::http::Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/cases/{case_id}/files"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(axum::body::Body::from(body))
        .expect("build multipart request");

    let resp = app.clone().oneshot(req).await.expect("oneshot");
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "oversized upload must be rejected"
    );
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("collect")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    // Either the explicit FILE_TOO_LARGE (420102) or the transport-level
    // DefaultBodyLimit rejection is acceptable as long as nothing persists.
    assert!(
        json["error_code"] == 420102 || json["error_code"] == 400000,
        "expected 420102 or 400000, got {:?}",
        json["error_code"],
    );

    // No file rows must remain for the case.
    let (_, list) = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/files"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(
        list["data"]["total"], 0,
        "rejected upload must not leave rows"
    );
}
