use axum::body::Body;
use axum::http::HeaderMap;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use legalminds_server::{router, AppConfig, AppEnv, AppState, CorsOrigins};
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;
use std::time::Duration;
use tempfile::TempDir;
use tower::util::ServiceExt;

#[tokio::test]
async fn smoke_flow_creates_audit_logs() {
    let (app, _tmp) = build_test_app().await;

    // Register (also returns tokens).
    let register = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        json!({
            "username": "testuser",
            "email": "testuser@example.com",
            "password": "Password123",
        }),
    )
    .await;
    assert_eq!(register.0, StatusCode::OK);
    let token = register.1["data"]["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();

    // Create case.
    let case_resp = request_json(
        &app,
        Method::POST,
        "/api/v1/cases",
        Some(&token),
        json!({
            "name": "Test Case",
            "description": "desc",
            "tags": ["t1", "t2"],
        }),
    )
    .await;
    assert_eq!(case_resp.0, StatusCode::OK);
    let case_id = case_resp.1["data"]["id"]
        .as_str()
        .expect("case id")
        .to_string();

    // Create timeline node.
    let node_resp = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/timeline/nodes"),
        Some(&token),
        json!({
            "title": "Contract Signed",
            "event_time": "2024-01-01",
            "tags": ["important"],
        }),
    )
    .await;
    assert_eq!(node_resp.0, StatusCode::OK);
    let node_id = node_resp.1["data"]["id"]
        .as_str()
        .expect("node id")
        .to_string();

    // Create a person in the case.
    let person_resp = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/persons"),
        Some(&token),
        json!({
            "name": "Zhang San",
            "gender": "male",
            "phone": "13800138000",
            "organization": "ACME",
            "position": "CEO",
            "role_type": "plaintiff",
        }),
    )
    .await;
    assert_eq!(person_resp.0, StatusCode::OK);
    let person_id = person_resp.1["data"]["id"]
        .as_str()
        .expect("person id")
        .to_string();

    let person_detail = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/persons/{person_id}"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(person_detail.0, StatusCode::OK);
    assert_eq!(
        person_detail.1["data"]["name"].as_str().unwrap_or(""),
        "Zhang San"
    );

    // Upload a small text file (no external parsers required).
    let upload_resp = request_multipart_text(
        &app,
        &format!("/api/v1/cases/{case_id}/files"),
        &token,
        "note.txt",
        "hello from tests\n",
    )
    .await;
    assert_eq!(upload_resp.0, StatusCode::OK);
    let evidence_id = upload_resp.1["data"]["id"]
        .as_str()
        .expect("evidence id")
        .to_string();

    // Link evidence to node.
    let link_resp = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/timeline/nodes/{node_id}/evidence"),
        Some(&token),
        json!({
            "evidence_id": evidence_id,
            "anchor_type": "page",
            "anchor_data": { "page_num": 1 },
        }),
    )
    .await;
    assert_eq!(link_resp.0, StatusCode::OK);

    // Export evidence list (CSV download + record creation).
    let export = request_raw(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/exports/evidence-list"),
        Some(&token),
    )
    .await;
    assert_eq!(export.0, StatusCode::OK);
    let ct = export
        .1
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        "expected xlsx content-type, got {ct}"
    );
    assert!(export.2.starts_with(b"PK"), "expected XLSX signature");

    // Export timeline (PNG download + record creation).
    let timeline = request_raw(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/exports/timeline?format=png"),
        Some(&token),
    )
    .await;
    assert_eq!(timeline.0, StatusCode::OK);
    let ct = timeline
        .1
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("image/png"),
        "expected image/png content-type, got {ct}"
    );
    assert!(
        timeline.2.starts_with(b"\x89PNG\r\n\x1a\n"),
        "expected PNG signature"
    );

    // Export timeline report (PDF download + record creation).
    let report = request_raw(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/exports/timeline-report?format=pdf"),
        Some(&token),
    )
    .await;
    assert_eq!(report.0, StatusCode::OK);
    let ct = report
        .1
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("application/pdf"),
        "expected application/pdf content-type, got {ct}"
    );
    assert!(report.2.starts_with(b"%PDF"), "expected PDF signature");

    let exports_history = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/exports/history"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(exports_history.0, StatusCode::OK);
    let records = exports_history.1["data"]["records"]
        .as_array()
        .expect("records array");
    let evidence_export_id = records
        .iter()
        .find(|r| r.get("export_type").and_then(|v| v.as_str()) == Some("evidence_list"))
        .and_then(|r| r.get("id").and_then(|v| v.as_str()))
        .expect("evidence_list export id")
        .to_string();
    let timeline_export_id = records
        .iter()
        .find(|r| r.get("export_type").and_then(|v| v.as_str()) == Some("timeline"))
        .and_then(|r| r.get("id").and_then(|v| v.as_str()))
        .expect("timeline export id")
        .to_string();
    let report_export_id = records
        .iter()
        .find(|r| r.get("export_type").and_then(|v| v.as_str()) == Some("report"))
        .and_then(|r| r.get("id").and_then(|v| v.as_str()))
        .expect("report export id")
        .to_string();

    let download = request_raw(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/exports/{evidence_export_id}/download"),
        Some(&token),
    )
    .await;
    assert_eq!(download.0, StatusCode::OK);

    let timeline_download = request_raw(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/exports/{timeline_export_id}/download"),
        Some(&token),
    )
    .await;
    assert_eq!(timeline_download.0, StatusCode::OK);
    let ct = timeline_download
        .1
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(ct.starts_with("image/png"));

    let report_download = request_raw(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/exports/{report_export_id}/download"),
        Some(&token),
    )
    .await;
    assert_eq!(report_download.0, StatusCode::OK);
    let ct = report_download
        .1
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("application/pdf"),
        "expected application/pdf content-type, got {ct}"
    );
    assert!(
        report_download.2.starts_with(b"%PDF"),
        "expected PDF signature"
    );

    let logs_export = request_raw(
        &app,
        Method::GET,
        &format!("/api/v1/logs/export?case_id={case_id}"),
        Some(&token),
    )
    .await;
    assert_eq!(logs_export.0, StatusCode::OK);
    let ct = logs_export
        .1
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(ct.starts_with("text/csv"));

    let logs_export_xlsx = request_raw(
        &app,
        Method::GET,
        &format!("/api/v1/logs/export?case_id={case_id}&format=excel"),
        Some(&token),
    )
    .await;
    assert_eq!(logs_export_xlsx.0, StatusCode::OK);
    let ct = logs_export_xlsx
        .1
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        "expected xlsx content-type, got {ct}"
    );
    assert!(
        logs_export_xlsx.2.starts_with(b"PK"),
        "expected XLSX signature"
    );

    // Audit logs are inserted asynchronously; wait until they show up.
    let logs = wait_for_case_logs(&app, &token, &case_id, 3).await;
    assert_eq!(logs.0, StatusCode::OK);
    let total = logs.1["data"]["total"].as_i64().unwrap_or(0);
    assert!(total >= 3, "expected >=3 case logs, got {total}");

    // Target history for the node should contain at least the CREATE action.
    let history = wait_for_target_history(&app, &token, "event_node", &node_id).await;
    assert_eq!(history.0, StatusCode::OK);
    let actions = history.1["data"]["history"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.get("action").and_then(|x| x.as_str()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    assert!(
        actions.iter().any(|a| *a == "CREATE"),
        "expected CREATE in node history; got {actions:?}"
    );
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
        jwt_secret: "test-secret-please-change-32-chars-min".to_string(),
        access_token_expire_minutes: 60,
        refresh_token_expire_days: 7,
        storage_path: storage_path.to_string_lossy().to_string(),
        max_file_size: 10 * 1024 * 1024,
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

async fn request_multipart_text(
    app: &axum::Router,
    uri: &str,
    bearer: &str,
    filename: &str,
    content: &str,
) -> (StatusCode, serde_json::Value) {
    let boundary = "XBOUNDARY";
    let body = format!(
        "--{boundary}\r\n\
Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
Content-Type: text/plain\r\n\
\r\n\
{content}\r\n\
--{boundary}--\r\n"
    );

    let req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
        .body(Body::from(body))
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

async fn request_raw(
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

async fn wait_for_case_logs(
    app: &axum::Router,
    bearer: &str,
    case_id: &str,
    min_total: i64,
) -> (StatusCode, serde_json::Value) {
    for _ in 0..20 {
        let resp = request_json(
            app,
            Method::GET,
            &format!("/api/v1/logs?case_id={case_id}"),
            Some(bearer),
            json!({}),
        )
        .await;
        let total = resp.1["data"]["total"].as_i64().unwrap_or(0);
        if resp.0 == StatusCode::OK && total >= min_total {
            return resp;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    request_json(
        app,
        Method::GET,
        &format!("/api/v1/logs?case_id={case_id}"),
        Some(bearer),
        json!({}),
    )
    .await
}

async fn wait_for_target_history(
    app: &axum::Router,
    bearer: &str,
    target_type: &str,
    target_id: &str,
) -> (StatusCode, serde_json::Value) {
    for _ in 0..20 {
        let resp = request_json(
            app,
            Method::GET,
            &format!("/api/v1/logs/{target_type}/{target_id}/history"),
            Some(bearer),
            json!({}),
        )
        .await;
        let history_len = resp.1["data"]["history"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0);
        if resp.0 == StatusCode::OK && history_len > 0 {
            return resp;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    request_json(
        app,
        Method::GET,
        &format!("/api/v1/logs/{target_type}/{target_id}/history"),
        Some(bearer),
        json!({}),
    )
    .await
}
