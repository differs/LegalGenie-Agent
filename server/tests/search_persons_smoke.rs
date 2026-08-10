use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use tempfile::TempDir;
use tower::util::ServiceExt;

use sqlx::PgPool;
mod test_support;

#[tokio::test]
async fn search_can_return_person_results() {
    let (app, _tmp) = build_test_app().await;

    let reg = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        json!({
            "username": "searchuser",
            "email": "searchuser@example.com",
            "password": "Password123",
        }),
    )
    .await;
    assert_eq!(reg.0, StatusCode::OK);
    let token = reg.1["data"]["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();

    let case_resp = request_json(
        &app,
        Method::POST,
        "/api/v1/cases",
        Some(&token),
        json!({ "name": "Search Case" }),
    )
    .await;
    assert_eq!(case_resp.0, StatusCode::OK);
    let case_id = case_resp.1["data"]["id"]
        .as_str()
        .expect("case id")
        .to_string();

    let person_resp = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/persons"),
        Some(&token),
        json!({ "name": "Zhang San", "role_type": "plaintiff" }),
    )
    .await;
    assert_eq!(person_resp.0, StatusCode::OK);
    let person_id = person_resp.1["data"]["id"]
        .as_str()
        .expect("person id")
        .to_string();

    let search = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/search?keyword=Zhang&object_types=person&case_id={case_id}"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(search.0, StatusCode::OK);
    let results = search.1["data"]["results"]
        .as_array()
        .expect("results array");

    assert!(
        results.iter().any(|r| {
            r.get("object_type").and_then(|v| v.as_str()) == Some("person")
                && r.get("object_id").and_then(|v| v.as_str()) == Some(person_id.as_str())
        }),
        "expected person result to be present"
    );
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

async fn build_test_app() -> (axum::Router, TempDir) {
    let (app, tmp, _pool) = build_test_app_with_pool().await;
    (app, tmp)
}

async fn build_test_app_with_pool() -> (axum::Router, TempDir, PgPool) {
    test_support::build_test_app_with_pool().await
}
