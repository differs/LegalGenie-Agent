#![allow(dead_code)]

use axum::body::Body;
use axum::http::HeaderMap;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use legalminds_server::{router, AppConfig, AppEnv, AppState, CorsOrigins};
use serde_json::json;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::time::Duration;
use tempfile::TempDir;
use tower::util::ServiceExt;

pub async fn build_test_app_with_pool() -> (axum::Router, TempDir, SqlitePool) {
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

    let state = AppState::new(cfg, pool);
    let pool = state.pool.clone();
    (router(state), tmp, pool)
}

pub async fn request_json(
    app: &axum::Router,
    method: Method,
    uri: &str,
    bearer: Option<&str>,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
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

pub async fn request_raw(
    app: &axum::Router,
    method: Method,
    uri: &str,
    bearer: Option<&str>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let req = builder.body(Body::empty()).expect("build request");

    let resp = app.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp
        .into_body()
        .collect()
        .await
        .expect("collect")
        .to_bytes();
    (status, headers, body.to_vec())
}

pub async fn request_multipart_text(
    app: &axum::Router,
    uri: &str,
    bearer: &str,
    filename: &str,
    content: &str,
) -> (StatusCode, serde_json::Value) {
    let boundary = "LMTESTBOUNDARYb2d79c3ce5c04f55b79b1c3d194fe9be";
    let content_type = format!("multipart/form-data; boundary={boundary}");

    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: text/plain\r\n\r\n");
    body.extend_from_slice(content.as_bytes());
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(body))
        .expect("build multipart request");

    let resp = app.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("collect")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    (status, json)
}

pub async fn register_user(app: &axum::Router, username: &str, email: &str) -> (String, String) {
    let resp = request_json(
        app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        json!({
            "username": username,
            "email": email,
            "password": "Password123",
        }),
    )
    .await;
    assert_eq!(resp.0, StatusCode::OK);

    let user_id = resp.1["data"]["user"]["id"]
        .as_str()
        .expect("user id")
        .to_string();
    let token = resp.1["data"]["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();
    (user_id, token)
}

pub async fn create_case(
    app: &axum::Router,
    bearer: &str,
    name: &str,
    description: &str,
) -> String {
    let resp = request_json(
        app,
        Method::POST,
        "/api/v1/cases",
        Some(bearer),
        json!({
            "name": name,
            "description": description,
        }),
    )
    .await;
    assert_eq!(resp.0, StatusCode::OK);
    resp.1["data"]["id"].as_str().expect("case id").to_string()
}

pub async fn upload_text_file(
    app: &axum::Router,
    bearer: &str,
    case_id: &str,
    filename: &str,
    content: &str,
) -> String {
    let upload = request_multipart_text(
        app,
        &format!("/api/v1/cases/{case_id}/files"),
        bearer,
        filename,
        content,
    )
    .await;
    assert_eq!(upload.0, StatusCode::OK);
    upload.1["data"]["id"]
        .as_str()
        .expect("evidence id")
        .to_string()
}

pub async fn wait_for_file_parse_done(
    app: &axum::Router,
    bearer: &str,
    file_id: &str,
) -> (StatusCode, serde_json::Value) {
    for _ in 0..50 {
        let resp = request_json(
            app,
            Method::GET,
            &format!("/api/v1/files/{file_id}"),
            Some(bearer),
            json!({}),
        )
        .await;
        let status = resp.1["data"]["parse_status"].as_str().unwrap_or("");
        if resp.0 == StatusCode::OK && status == "done" {
            return resp;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    request_json(
        app,
        Method::GET,
        &format!("/api/v1/files/{file_id}"),
        Some(bearer),
        json!({}),
    )
    .await
}
