// P3 smoke tests: shared rate limiting, permission matrix, approval policy.
mod test_support;

use axum::http::{header, Method, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use test_support::{
    build_test_app_with_pool, create_case, register_user, request_json, request_raw,
};
use tower::ServiceExt;

#[tokio::test]
async fn rate_limiter_is_shared_across_instances() {
    let (_app, _tmp, pool) = build_test_app_with_pool().await;
    let limiter = legalminds_server::rate_limit::RateLimiter::new(pool.clone());
    let limiter2 = legalminds_server::rate_limit::RateLimiter::new(pool.clone());

    // Two "instances" share one counter through PostgreSQL.
    let key = "login:shared-test";
    for i in 0..5 {
        let (allowed, _) = limiter
            .check_and_record(key, 5, std::time::Duration::from_secs(60))
            .await;
        assert!(allowed, "attempt {i} allowed via instance 1");
    }
    let (allowed, retry_after) = limiter2
        .check_and_record(key, 5, std::time::Duration::from_secs(60))
        .await;
    assert!(!allowed, "6th attempt limited via instance 2");
    assert!(retry_after >= 1);
}

#[tokio::test]
async fn rate_limiter_resets_after_window() {
    let (_app, _tmp, pool) = build_test_app_with_pool().await;
    let limiter = legalminds_server::rate_limit::RateLimiter::new(pool.clone());

    let key = "login:window-test";
    let window = std::time::Duration::from_secs(1);
    for _ in 0..3 {
        let (allowed, _) = limiter.check_and_record(key, 3, window).await;
        assert!(allowed);
    }
    let (allowed, _) = limiter.check_and_record(key, 3, window).await;
    assert!(!allowed);

    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    let (allowed, _) = limiter.check_and_record(key, 3, window).await;
    assert!(allowed, "window expires and counter resets");
}

#[tokio::test]
async fn permission_matrix_revocation_is_enforced() {
    let (app, _tmp, pool) = build_test_app_with_pool().await;
    let (owner_id, owner_token) = register_user(&app, "p3_owner", "p3-owner@test.local").await;
    let (member_id, member_token) = register_user(&app, "p3_member", "p3-member@test.local").await;
    let case_id = create_case(&app, &owner_token, "Matrix Case", "permission matrix").await;

    // Add member.
    let add = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/members"),
        Some(&owner_token),
        json!({ "user_id": member_id, "role_in_case": "member" }),
    )
    .await;
    assert_eq!(add.0, StatusCode::OK);

    // Member export is allowed by default (matrix seed).
    let exported = request_raw(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/exports/evidence-list"),
        Some(&member_token),
    )
    .await;
    assert_eq!(exported.0, StatusCode::OK);

    // Revoke member case:export from the matrix → now denied (owner untouched).
    sqlx::query("DELETE FROM permissions WHERE role = 'member' AND operation = 'case:export'")
        .execute(&pool)
        .await
        .unwrap();

    let denied = request_raw(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/exports/evidence-list"),
        Some(&member_token),
    )
    .await;
    assert_eq!(
        denied.0,
        StatusCode::FORBIDDEN,
        "matrix revocation enforced"
    );

    let owner_ok = request_raw(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/exports/evidence-list"),
        Some(&owner_token),
    )
    .await;
    assert_eq!(owner_ok.0, StatusCode::OK, "owner bypasses matrix");
    let _ = owner_id;
}

#[tokio::test]
async fn approval_policy_auto_executes_write_tools_immediately() {
    // Build app with APPROVAL_POLICY=auto.
    let (_app, _tmp, pool) = build_test_app_with_pool().await;
    // The router was built with the default (ask) config; for auto we build a
    // custom state + router via test_support internals.
    let (pool2, tmp) = test_support::test_pool_and_tmp().await;
    let cfg = legalminds_server::AppConfig {
        app_env: legalminds_server::AppEnv::Test,
        server_host: "127.0.0.1".to_string(),
        server_port: 0,
        database_url: "postgres://legalgenie:legalgenie_dev@127.0.0.1:5432/legalgenie".to_string(),
        cors_origins: legalminds_server::CorsOrigins::Any,
        force_https: false,
        trust_proxy_headers: false,
        jwt_secret: "test-secret-please-change-32-chars-min".to_string(),
        jwt_secret_old: None,
        access_token_expire_minutes: 60,
        refresh_token_expire_days: 7,
        storage_path: tmp.path().join("storage").to_string_lossy().to_string(),
        max_file_size: 10 * 1024 * 1024,
        allowed_file_types: vec!["txt".to_string()],
        temp_path: tmp
            .path()
            .join("storage")
            .join("temp")
            .to_string_lossy()
            .to_string(),
        tessdata_dir: tmp.path().join("tessdata").to_string_lossy().to_string(),
        whisper_model_path: tmp.path().join("whisper.bin").to_string_lossy().to_string(),
        asr_language: "zh".to_string(),
        asr_threads: 1,
        approval_policy: "auto".to_string(),
    };
    let state = legalminds_server::AppState::new(cfg, pool2.clone());
    let app = legalminds_server::router(state);

    let (_uid, token) = register_user(&app, "p3_auto", "p3-auto@test.local").await;
    let case_id = create_case(&app, &token, "Auto Policy", "approval auto").await;

    let session = request_json(
        &app,
        Method::POST,
        "/api/v1/agent/sessions",
        Some(&token),
        json!({ "case_id": case_id }),
    )
    .await;
    let session_id = session.1["data"]["id"]
        .as_str()
        .expect("session")
        .to_string();

    // Write intent executes immediately under auto policy.
    let run = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/agent/sessions/{session_id}/messages"),
        Some(&token),
        json!({ "content": "新建节点「自动执行」2024-06-01" }),
    )
    .await;
    assert_eq!(run.0, StatusCode::OK, "auto run: {:?}", run.1);
    assert_eq!(
        run.1["data"]["tool_calls"][0]["status"].as_str(),
        Some("executed")
    );
    assert!(run.1["data"]["pending_approvals"]
        .as_array()
        .unwrap()
        .is_empty());

    let nodes = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/timeline/nodes"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(
        nodes.1["data"]["total"], 1,
        "auto policy writes immediately"
    );
    let _ = pool;
}

#[tokio::test]
async fn login_retry_after_uses_shared_limiter() {
    let (app, _tmp, _pool) = build_test_app_with_pool().await;
    for _ in 0..5 {
        let (status, _) = request_json(
            &app,
            Method::POST,
            "/api/v1/auth/login",
            None,
            json!({ "username": "nobody-p3", "password": "WrongPass123", "remember_me": false }),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, headers, _body) = request_json_headers(
        &app,
        "/api/v1/auth/login",
        json!({ "username": "nobody-p3", "password": "WrongPass123", "remember_me": false }),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let retry_after = headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .expect("retry-after present");
    assert!(retry_after.parse::<u64>().unwrap_or(0) >= 1);
}

async fn request_json_headers(
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
