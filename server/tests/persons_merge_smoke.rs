use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use tempfile::TempDir;
use tower::util::ServiceExt;

use sqlx::PgPool;
mod test_support;

#[tokio::test]
async fn merge_case_persons_moves_links_and_relationships() {
    let (app, _tmp) = build_test_app().await;

    // Owner creates a case.
    let (_owner_id, owner_token) = register_user(&app, "owner", "owner@example.com").await;
    let case_resp = request_json(
        &app,
        Method::POST,
        "/api/v1/cases",
        Some(&owner_token),
        json!({ "name": "Case A", "description": "d" }),
    )
    .await;
    assert_eq!(case_resp.0, StatusCode::OK);
    let case_id = case_resp.1["data"]["id"]
        .as_str()
        .expect("case id")
        .to_string();

    // Create persons: A (target), B (source), C (third).
    let p_a = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/persons"),
        Some(&owner_token),
        json!({ "name": "Alice", "role_type": "plaintiff" }),
    )
    .await;
    assert_eq!(p_a.0, StatusCode::OK);
    let a_id = p_a.1["data"]["id"].as_str().expect("a id").to_string();

    let p_b = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/persons"),
        Some(&owner_token),
        json!({ "name": "Bob", "role_type": "witness" }),
    )
    .await;
    assert_eq!(p_b.0, StatusCode::OK);
    let b_id = p_b.1["data"]["id"].as_str().expect("b id").to_string();

    let p_c = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/persons"),
        Some(&owner_token),
        json!({ "name": "Carol", "role_type": "other" }),
    )
    .await;
    assert_eq!(p_c.0, StatusCode::OK);
    let c_id = p_c.1["data"]["id"].as_str().expect("c id").to_string();

    // Create a case-local relationship: B -> C.
    let rel = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/persons/relationships"),
        Some(&owner_token),
        json!({
            "from_person_id": b_id,
            "to_person_id": c_id,
            "rel_type": "knows",
            "rel_detail": "met at work",
        }),
    )
    .await;
    assert_eq!(rel.0, StatusCode::OK);

    // Merge: B -> A (case-local).
    let merged = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/persons/merge"),
        Some(&owner_token),
        json!({
            "source_person_id": b_id,
            "target_person_id": a_id,
        }),
    )
    .await;
    assert_eq!(merged.0, StatusCode::OK);
    assert_eq!(
        merged.1["data"]["source_person_id"].as_str().unwrap_or(""),
        b_id
    );
    assert_eq!(
        merged.1["data"]["target_person_id"].as_str().unwrap_or(""),
        a_id
    );
    assert!(
        merged.1["data"]["moved_links"].as_i64().unwrap_or(0) >= 1,
        "expected moved_links >= 1"
    );
    assert!(
        merged.1["data"]["moved_relationships"]
            .as_i64()
            .unwrap_or(0)
            >= 1,
        "expected moved_relationships >= 1"
    );

    // B should no longer appear in the case persons list (its case link is removed).
    let list = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/persons?page=1&page_size=50"),
        Some(&owner_token),
        json!({}),
    )
    .await;
    assert_eq!(list.0, StatusCode::OK);
    let persons = list.1["data"]["persons"].as_array().expect("persons array");
    let ids = persons
        .iter()
        .filter_map(|p| p["id"].as_str().map(|s| s.to_string()))
        .collect::<Vec<_>>();
    assert!(ids.contains(&a_id));
    assert!(ids.contains(&c_id));
    assert!(!ids.contains(&b_id));

    // Relationship should be re-pointed to A -> C.
    let rels = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/persons/relationships"),
        Some(&owner_token),
        json!({}),
    )
    .await;
    assert_eq!(rels.0, StatusCode::OK);
    let edges = rels.1["data"]["relationships"]
        .as_array()
        .expect("relationships array");
    assert!(!edges.is_empty());

    let has_a_to_c = edges.iter().any(|e| {
        e["from_person_id"].as_str() == Some(a_id.as_str())
            && e["to_person_id"].as_str() == Some(c_id.as_str())
            && e["rel_type"].as_str() == Some("knows")
    });
    assert!(has_a_to_c, "expected an A -> C knows relationship");

    let has_b_to_c = edges.iter().any(|e| {
        e["from_person_id"].as_str() == Some(b_id.as_str())
            && e["to_person_id"].as_str() == Some(c_id.as_str())
            && e["rel_type"].as_str() == Some("knows")
    });
    assert!(!has_b_to_c, "expected no remaining B -> C relationship");
}

async fn register_user(app: &axum::Router, username: &str, email: &str) -> (String, String) {
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
