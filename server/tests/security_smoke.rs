use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use tempfile::TempDir;
use tower::util::ServiceExt;

use sqlx::PgPool;
mod test_support;

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

async fn build_test_app() -> (axum::Router, TempDir) {
    let (app, tmp, _pool) = build_test_app_with_pool().await;
    (app, tmp)
}

async fn build_test_app_with_pool() -> (axum::Router, TempDir, PgPool) {
    test_support::build_test_app_with_pool().await
}
