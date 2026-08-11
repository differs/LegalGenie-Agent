#![allow(dead_code)]

use axum::body::Body;
use axum::http::HeaderMap;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use legalminds_server::{router, AppConfig, AppEnv, AppState, CorsOrigins};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::OnceLock;
use std::time::Duration;
use tempfile::TempDir;
use tower::util::ServiceExt;

/// Test database URL; the dev/CI postgres must accept these credentials.
const TEST_DB_URL: &str = "postgres://legalgenie:legalgenie_dev@127.0.0.1:5432/legalgenie";

static PG_EXT_READY: OnceLock<()> = OnceLock::new();

/// Builds an isolated-schema pool + temp dirs without an AppState/router.
/// Used by tests that need a custom AppState (e.g. fake translation provider).
pub async fn test_pool_and_tmp() -> (PgPool, TempDir) {
    let (pool, tmp) = {
        let schema = unique_schema_name();
        ensure_pg_trgm().await;

        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(TEST_DB_URL)
            .await
            .expect("connect postgres admin");

        sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
            .execute(&admin)
            .await
            .expect("create schema");

        let pool = PgPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(10))
            .after_connect(move |conn, _meta| {
                let schema = schema.clone();
                Box::pin(async move {
                    sqlx::query(&format!("SET search_path = \"{schema}\", public"))
                        .execute(&mut *conn)
                        .await?;
                    Ok(())
                })
            })
            .connect(TEST_DB_URL)
            .await
            .expect("connect postgres");

        sqlx::migrate!("./migrations_pg")
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

        (pool, tmp)
    };
    (pool, tmp)
}

async fn ensure_pg_trgm() {
    let bootstrap = PgPoolOptions::new()
        .max_connections(1)
        .connect(TEST_DB_URL)
        .await
        .expect("connect postgres (is docker legalgenie-pg running?)");
    sqlx::query("CREATE EXTENSION IF NOT EXISTS pg_trgm")
        .execute(&bootstrap)
        .await
        .expect("create pg_trgm extension");
}

fn unique_schema_name() -> String {
    format!("t_{}_{:08x}", std::process::id(), fast_random_u32())
}

/// Builds a standard test app/router with an isolated schema.
pub async fn build_test_app_with_pool() -> (axum::Router, TempDir, PgPool) {
    // The trigram extension is database-global; create it once up front to
    // avoid CREATE EXTENSION races between parallel tests.
    PG_EXT_READY.get_or_init(|| ());
    ensure_pg_trgm().await;

    // Unique schema per app instance (parallel-safe).
    let schema = unique_schema_name();

    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(TEST_DB_URL)
        .await
        .expect("connect postgres admin");

    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&admin)
        .await
        .expect("create schema");

    // App pool scoped to the schema. `after_connect` re-applies the search
    // path to every pooled connection.
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .after_connect(move |conn, _meta| {
            let schema = schema.clone();
            Box::pin(async move {
                sqlx::query(&format!("SET search_path = \"{schema}\", public"))
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect(TEST_DB_URL)
        .await
        .expect("connect postgres");

    sqlx::migrate!("./migrations_pg")
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
        database_url: TEST_DB_URL.to_string(),
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
        approval_policy: "ask".to_string(),
    };

    let state = AppState::new(cfg, pool);
    let pool = state.pool.clone();
    // P1b: parse/translate now run through the durable job queue; give tests a
    // real worker so parsing completes without manual intervention.
    legalminds_server::job_worker::spawn_job_workers(
        state.clone(),
        tokio_util::sync::CancellationToken::new(),
        2,
    );
    (router(state), tmp, pool)
}

/// Small non-cryptographic unique suffix for schema names.
fn fast_random_u32() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    // Mix in a cheap counter so same-nanosecond calls still differ.
    nanos ^ (nanos.rotate_left(13)) ^ std::process::id().rotate_left(17)
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
    assert_eq!(resp.0, StatusCode::OK, "register failed: {:?}", resp.1);

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
