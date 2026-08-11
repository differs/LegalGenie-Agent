// P4 smoke tests: metrics endpoint, health probes, audit append-only.
mod test_support;

use axum::http::{Method, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use test_support::{build_test_app_with_pool, register_user, request_json};
use tower::ServiceExt;

#[tokio::test]
async fn metrics_endpoint_exposes_prometheus_text() {
    let (app, _tmp, _pool) = build_test_app_with_pool().await;
    let (_uid, token) = register_user(&app, "p4_metrics", "p4-metrics@test.local").await;

    // Exercise the API so counters are non-zero.
    let _ = request_json(&app, Method::GET, "/api/v1/health", None, json!({})).await;
    let _ = request_json(
        &app,
        Method::GET,
        "/api/v1/cases?page=1&page_size=5",
        Some(&token),
        json!({}),
    )
    .await;

    // Metrics live at the top-level /metrics (outside /api/v1).
    let req = axum::http::Request::builder()
        .method(Method::GET)
        .uri("/metrics")
        .body(axum::body::Body::empty())
        .expect("build");
    let resp = app.clone().oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("collect")
        .to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("legalgenie_http_requests_total"),
        "http counter present"
    );
    assert!(
        text.contains("legalgenie_jobs_pending_total"),
        "job gauge present"
    );
    assert!(
        text.contains("legalgenie_workers_active"),
        "worker gauge present"
    );
}

#[tokio::test]
async fn health_probes_live_and_ready() {
    let (app, _tmp, _pool) = build_test_app_with_pool().await;

    let req = axum::http::Request::builder()
        .method(Method::GET)
        .uri("/health/live")
        .body(axum::body::Body::empty())
        .expect("build");
    let resp = app.clone().oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);

    let req = axum::http::Request::builder()
        .method(Method::GET)
        .uri("/health/ready")
        .body(axum::body::Body::empty())
        .expect("build");
    let resp = app.clone().oneshot(req).await.expect("oneshot");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "ready must pass with healthy deps"
    );
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("collect")
        .to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(body["status"].as_str(), Some("ok"));
}

#[tokio::test]
async fn operation_logs_are_append_only() {
    let (app, _tmp, pool) = build_test_app_with_pool().await;
    let (_uid, token) = register_user(&app, "p4_audit", "p4-audit@test.local").await;

    // Trigger a log entry.
    let _ = request_json(
        &app,
        Method::POST,
        "/api/v1/cases",
        Some(&token),
        json!({ "name": "Audit Case" }),
    )
    .await;

    // Direct mutation must be rejected by the trigger.
    let res = sqlx::query("UPDATE operation_logs SET action = 'HACKED' WHERE action = 'CREATE'")
        .execute(&pool)
        .await;
    assert!(res.is_err(), "append-only trigger must block UPDATE");
    let res = sqlx::query("DELETE FROM operation_logs WHERE 1=1")
        .execute(&pool)
        .await;
    assert!(res.is_err(), "append-only trigger must block DELETE");
}
