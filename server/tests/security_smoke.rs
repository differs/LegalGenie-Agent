use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use legalminds_server::{router, AppConfig, AppEnv, AppState, CorsOrigins};
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;
use tower::util::ServiceExt;

#[tokio::test]
async fn rejects_weak_password_on_register() {
    let (app, _tmp) = build_test_app().await;

    let resp = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        json!({
            "username": "weakuser",
            "email": "weakuser@example.com",
            "password": "password123",
        }),
    )
    .await;
    assert_eq!(resp.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn login_rate_limited_after_too_many_attempts() {
    let (app, _tmp) = build_test_app().await;

    let reg = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        json!({
            "username": "rateuser",
            "email": "rateuser@example.com",
            "password": "Password123",
        }),
    )
    .await;
    assert_eq!(reg.0, StatusCode::OK);

    for _ in 0..5 {
        let login = request_json(
            &app,
            Method::POST,
            "/api/v1/auth/login",
            None,
            json!({
                "username": "rateuser",
                "password": "WrongPass123",
            }),
        )
        .await;
        assert_eq!(login.0, StatusCode::UNAUTHORIZED);
    }

    let limited = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        json!({
            "username": "rateuser",
            "password": "WrongPass123",
        }),
    )
    .await;
    assert_eq!(limited.0, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn successful_logins_do_not_trigger_rate_limit() {
    let (app, _tmp) = build_test_app().await;

    let reg = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        json!({
            "username": "successrateuser",
            "email": "successrateuser@example.com",
            "password": "Password123",
        }),
    )
    .await;
    assert_eq!(reg.0, StatusCode::OK);

    for _ in 0..6 {
        let login = request_json(
            &app,
            Method::POST,
            "/api/v1/auth/login",
            None,
            json!({
                "username": "successrateuser",
                "password": "Password123",
            }),
        )
        .await;
        assert_eq!(login.0, StatusCode::OK);
    }
}

#[tokio::test]
async fn spoofed_forwarded_ip_does_not_bypass_rate_limit_by_default() {
    let (app, _tmp) = build_test_app().await;

    let reg = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        json!({
            "username": "spoofrateuser",
            "email": "spoofrateuser@example.com",
            "password": "Password123",
        }),
    )
    .await;
    assert_eq!(reg.0, StatusCode::OK);

    for _ in 0..5 {
        let login = request_json_with_headers(
            &app,
            Method::POST,
            "/api/v1/auth/login",
            None,
            json!({
                "username": "spoofrateuser",
                "password": "WrongPass123",
            }),
            &[("x-forwarded-for", "1.1.1.1")],
        )
        .await;
        assert_eq!(login.0, StatusCode::UNAUTHORIZED);
    }

    let limited = request_json_with_headers(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        json!({
            "username": "spoofrateuser",
            "password": "WrongPass123",
        }),
        &[("x-forwarded-for", "2.2.2.2"), ("x-real-ip", "3.3.3.3")],
    )
    .await;
    assert_eq!(limited.0, StatusCode::TOO_MANY_REQUESTS);
}

async fn build_test_app() -> (axum::Router, TempDir) {
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
        access_token_expire_minutes: 60,
        refresh_token_expire_days: 7,
        storage_path: storage_path.to_string_lossy().to_string(),
        max_file_size: 10 * 1024 * 1024,
        allowed_file_types: vec!["txt".to_string(), "json".to_string()],
        temp_path: temp_path.to_string_lossy().to_string(),
        tessdata_dir: tessdata_dir.to_string_lossy().to_string(),
        whisper_model_path: tmp.path().join("whisper.bin").to_string_lossy().to_string(),
        asr_language: "zh".to_string(),
        asr_threads: 1,
    };

    let state = AppState::new(cfg, pool);
    (router(state), tmp)
}

async fn request_json(
    app: &axum::Router,
    method: Method,
    uri: &str,
    bearer: Option<&str>,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    request_json_with_headers(app, method, uri, bearer, body, &[]).await
}

async fn request_json_with_headers(
    app: &axum::Router,
    method: Method,
    uri: &str,
    bearer: Option<&str>,
    body: serde_json::Value,
    extra_headers: &[(&str, &str)],
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    for (name, value) in extra_headers {
        builder = builder.header(*name, *value);
    }
    let req = builder
        .body(Body::from(body.to_string()))
        .expect("build request");

    let resp = app.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let body = resp
        .into_body()
        .collect()
        .await
        .expect("collect")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    (status, json)
}
