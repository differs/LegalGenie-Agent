use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use tempfile::TempDir;
use tower::util::ServiceExt;

use sqlx::PgPool;
mod test_support;

#[tokio::test]
async fn viewer_is_read_only_for_case_mutations() {
    let (app, _tmp) = build_test_app().await;

    // Owner user creates a case.
    let (owner_id, owner_token) = register_user(&app, "owner", "owner@example.com").await;
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

    // Member user can mutate.
    let (member_id, member_token) = register_user(&app, "member", "member@example.com").await;
    let add_member = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/members"),
        Some(&owner_token),
        json!({ "user_id": member_id, "role_in_case": "member" }),
    )
    .await;
    assert_eq!(add_member.0, StatusCode::OK);

    let member_node = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/timeline/nodes"),
        Some(&member_token),
        json!({ "title": "t", "event_time": "2024-01-01" }),
    )
    .await;
    assert_eq!(member_node.0, StatusCode::OK);

    // Viewer user is read-only.
    let (viewer_id, viewer_token) = register_user(&app, "viewer", "viewer@example.com").await;

    // Member cannot manage case members (owner-only).
    let member_add = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/members"),
        Some(&member_token),
        json!({ "user_id": viewer_id, "role_in_case": "viewer" }),
    )
    .await;
    assert_eq!(member_add.0, StatusCode::FORBIDDEN);

    // Member cannot delete case (owner-only).
    let member_delete_case = request_json(
        &app,
        Method::DELETE,
        &format!("/api/v1/cases/{case_id}"),
        Some(&member_token),
        json!({}),
    )
    .await;
    assert_eq!(member_delete_case.0, StatusCode::FORBIDDEN);

    let member_upload = request_multipart_text(
        &app,
        &format!("/api/v1/cases/{case_id}/files"),
        &member_token,
        "note.txt",
        "member upload\n",
    )
    .await;
    assert_eq!(member_upload.0, StatusCode::OK);

    let add_viewer = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/members"),
        Some(&owner_token),
        json!({ "user_id": viewer_id, "role_in_case": "viewer" }),
    )
    .await;
    assert_eq!(add_viewer.0, StatusCode::OK);

    // /members/me should reflect the user's role in this case.
    let owner_me = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/members/me"),
        Some(&owner_token),
        json!({}),
    )
    .await;
    assert_eq!(owner_me.0, StatusCode::OK);
    assert_eq!(
        owner_me.1["data"]["role_in_case"].as_str().unwrap_or(""),
        "owner"
    );

    let member_me = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/members/me"),
        Some(&member_token),
        json!({}),
    )
    .await;
    assert_eq!(member_me.0, StatusCode::OK);
    assert_eq!(
        member_me.1["data"]["role_in_case"].as_str().unwrap_or(""),
        "member"
    );

    let viewer_me = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/members/me"),
        Some(&viewer_token),
        json!({}),
    )
    .await;
    assert_eq!(viewer_me.0, StatusCode::OK);
    assert_eq!(
        viewer_me.1["data"]["role_in_case"].as_str().unwrap_or(""),
        "viewer"
    );

    // Viewer can read case detail.
    let viewer_get_case = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}"),
        Some(&viewer_token),
        json!({}),
    )
    .await;
    assert_eq!(viewer_get_case.0, StatusCode::OK);

    // Viewer cannot update case.
    let viewer_update_case = request_json(
        &app,
        Method::PUT,
        &format!("/api/v1/cases/{case_id}"),
        Some(&viewer_token),
        json!({ "name": "nope" }),
    )
    .await;
    assert_eq!(viewer_update_case.0, StatusCode::FORBIDDEN);

    // Viewer cannot upload evidence file.
    let viewer_upload = request_multipart_text(
        &app,
        &format!("/api/v1/cases/{case_id}/files"),
        &viewer_token,
        "note.txt",
        "viewer upload\n",
    )
    .await;
    assert_eq!(viewer_upload.0, StatusCode::FORBIDDEN);

    // Viewer cannot create timeline node.
    let viewer_create_node = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/timeline/nodes"),
        Some(&viewer_token),
        json!({ "title": "x", "event_time": "2024-01-02" }),
    )
    .await;
    assert_eq!(viewer_create_node.0, StatusCode::FORBIDDEN);

    // Owner uploads a file so we can test viewer deletes/links.
    let owner_upload = request_multipart_text(
        &app,
        &format!("/api/v1/cases/{case_id}/files"),
        &owner_token,
        "note.txt",
        "owner upload\n",
    )
    .await;
    assert_eq!(owner_upload.0, StatusCode::OK);
    let evidence_id = owner_upload.1["data"]["id"]
        .as_str()
        .expect("evidence id")
        .to_string();

    // Owner creates a node.
    let owner_node = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/timeline/nodes"),
        Some(&owner_token),
        json!({ "title": "owner node", "event_time": "2024-01-03" }),
    )
    .await;
    assert_eq!(owner_node.0, StatusCode::OK);
    let node_id = owner_node.1["data"]["id"]
        .as_str()
        .expect("node id")
        .to_string();

    // Owner links evidence so we can test viewer unlink.
    let owner_link = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/timeline/nodes/{node_id}/evidence"),
        Some(&owner_token),
        json!({ "evidence_id": evidence_id, "anchor_type": "page", "anchor_data": { "page_num": 1 } }),
    )
    .await;
    assert_eq!(owner_link.0, StatusCode::OK);
    let link_id = owner_link.1["data"]["id"]
        .as_str()
        .expect("link id")
        .to_string();

    // Viewer can read a node detail (read-only access).
    let viewer_get_node = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/timeline/nodes/{node_id}"),
        Some(&viewer_token),
        json!({}),
    )
    .await;
    assert_eq!(viewer_get_node.0, StatusCode::OK);
    assert_eq!(
        viewer_get_node.1["data"]["id"].as_str().unwrap_or(""),
        node_id
    );

    // Viewer cannot delete evidence file.
    let viewer_delete_file = request_json(
        &app,
        Method::DELETE,
        &format!("/api/v1/files/{evidence_id}"),
        Some(&viewer_token),
        json!({}),
    )
    .await;
    assert_eq!(viewer_delete_file.0, StatusCode::FORBIDDEN);

    // Viewer cannot parse evidence file.
    let viewer_parse = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/files/{evidence_id}/parse"),
        Some(&viewer_token),
        json!({}),
    )
    .await;
    assert_eq!(viewer_parse.0, StatusCode::FORBIDDEN);

    // Viewer cannot link evidence to node.
    let viewer_link = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/timeline/nodes/{node_id}/evidence"),
        Some(&viewer_token),
        json!({ "evidence_id": evidence_id, "anchor_type": "page", "anchor_data": { "page_num": 1 } }),
    )
    .await;
    assert_eq!(viewer_link.0, StatusCode::FORBIDDEN);

    // Viewer cannot unlink evidence from node.
    let viewer_unlink = request_json(
        &app,
        Method::DELETE,
        &format!("/api/v1/timeline/nodes/{node_id}/evidence/{link_id}"),
        Some(&viewer_token),
        json!({}),
    )
    .await;
    assert_eq!(viewer_unlink.0, StatusCode::FORBIDDEN);

    // Viewer cannot update/move/delete timeline node.
    let viewer_update_node = request_json(
        &app,
        Method::PUT,
        &format!("/api/v1/timeline/nodes/{node_id}"),
        Some(&viewer_token),
        json!({ "title": "nope" }),
    )
    .await;
    assert_eq!(viewer_update_node.0, StatusCode::FORBIDDEN);

    let viewer_move_node = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/timeline/nodes/{node_id}/move"),
        Some(&viewer_token),
        json!({ "new_time": "2024-01-04" }),
    )
    .await;
    assert_eq!(viewer_move_node.0, StatusCode::FORBIDDEN);

    let viewer_delete_node = request_json(
        &app,
        Method::DELETE,
        &format!("/api/v1/timeline/nodes/{node_id}"),
        Some(&viewer_token),
        json!({}),
    )
    .await;
    assert_eq!(viewer_delete_node.0, StatusCode::FORBIDDEN);

    // Owner creates a person; viewer cannot update or link cases for that person.
    let owner_person = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/persons"),
        Some(&owner_token),
        json!({ "name": "Zhang San", "role_type": "plaintiff" }),
    )
    .await;
    assert_eq!(owner_person.0, StatusCode::OK);
    let person_id = owner_person.1["data"]["id"]
        .as_str()
        .expect("person id")
        .to_string();

    let viewer_update_person = request_json(
        &app,
        Method::PUT,
        &format!("/api/v1/persons/{person_id}"),
        Some(&viewer_token),
        json!({ "notes": "should fail" }),
    )
    .await;
    assert_eq!(viewer_update_person.0, StatusCode::FORBIDDEN);

    let viewer_delete_person = request_json(
        &app,
        Method::DELETE,
        &format!("/api/v1/persons/{person_id}"),
        Some(&viewer_token),
        json!({}),
    )
    .await;
    assert_eq!(viewer_delete_person.0, StatusCode::FORBIDDEN);

    let viewer_link_person_case = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/persons/{person_id}/cases"),
        Some(&viewer_token),
        json!({ "case_id": case_id, "role_type": "witness" }),
    )
    .await;
    assert_eq!(viewer_link_person_case.0, StatusCode::FORBIDDEN);

    // Owner creates another person with the same name so dedupe candidates can return groups.
    let owner_person_2 = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/persons"),
        Some(&owner_token),
        json!({ "name": "Zhang San", "role_type": "witness" }),
    )
    .await;
    assert_eq!(owner_person_2.0, StatusCode::OK);
    let person_id_2 = owner_person_2.1["data"]["id"]
        .as_str()
        .expect("person id 2")
        .to_string();

    // Viewer can read dedupe suggestions (read-only).
    let viewer_dedupe = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/persons/dedupe"),
        Some(&viewer_token),
        json!({}),
    )
    .await;
    assert_eq!(viewer_dedupe.0, StatusCode::OK);
    assert!(viewer_dedupe.1["data"]["groups"].is_array());

    // Owner creates a relationship.
    let owner_rel = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/persons/relationships"),
        Some(&owner_token),
        json!({
            "from_person_id": person_id,
            "to_person_id": person_id_2,
            "rel_type": "knows",
        }),
    )
    .await;
    assert_eq!(owner_rel.0, StatusCode::OK);
    let rel_id = owner_rel.1["data"]["id"]
        .as_str()
        .expect("relationship id")
        .to_string();

    // Viewer can list relationships (read-only).
    let viewer_list_rels = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/persons/relationships"),
        Some(&viewer_token),
        json!({}),
    )
    .await;
    assert_eq!(viewer_list_rels.0, StatusCode::OK);

    // Viewer cannot create relationships.
    let viewer_create_rel = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/persons/relationships"),
        Some(&viewer_token),
        json!({
            "from_person_id": person_id,
            "to_person_id": person_id_2,
            "rel_type": "knows",
        }),
    )
    .await;
    assert_eq!(viewer_create_rel.0, StatusCode::FORBIDDEN);

    // Viewer cannot delete relationships.
    let viewer_delete_rel = request_json(
        &app,
        Method::DELETE,
        &format!("/api/v1/cases/{case_id}/persons/relationships/{rel_id}"),
        Some(&viewer_token),
        json!({}),
    )
    .await;
    assert_eq!(viewer_delete_rel.0, StatusCode::FORBIDDEN);

    // Viewer cannot merge persons (case-local write operation).
    let viewer_merge = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/persons/merge"),
        Some(&viewer_token),
        json!({
            "source_person_id": person_id_2,
            "target_person_id": person_id,
        }),
    )
    .await;
    assert_eq!(viewer_merge.0, StatusCode::FORBIDDEN);

    // Sanity: owner_id is only used for additional confidence that the test exercised distinct users.
    assert_ne!(owner_id, ""); // keep clippy from complaining about unused
}

#[tokio::test]
async fn updating_member_role_preserves_original_joined_by() {
    let (app, _tmp, pool) = build_test_app_with_pool().await;

    let (owner_id, owner_token) =
        register_user(&app, "owner_audit", "owner_audit@example.com").await;
    let (next_owner_id, next_owner_token) =
        register_user(&app, "owner_next", "owner_next@example.com").await;
    let (member_id, _member_token) =
        register_user(&app, "member_audit", "member_audit@example.com").await;

    let case_resp = request_json(
        &app,
        Method::POST,
        "/api/v1/cases",
        Some(&owner_token),
        json!({ "name": "Audit Case", "description": "d" }),
    )
    .await;
    assert_eq!(case_resp.0, StatusCode::OK);
    let case_id = case_resp.1["data"]["id"]
        .as_str()
        .expect("case id")
        .to_string();

    let add_member = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/members"),
        Some(&owner_token),
        json!({ "user_id": member_id, "role_in_case": "viewer" }),
    )
    .await;
    assert_eq!(add_member.0, StatusCode::OK);
    assert_eq!(
        add_member.1["data"]["joined_by"].as_str().unwrap_or(""),
        owner_id
    );

    sqlx::query("UPDATE cases SET owner_id = $1 WHERE id = $2")
        .bind(&next_owner_id)
        .bind(&case_id)
        .execute(&pool)
        .await
        .expect("transfer owner in test setup");

    let update_member = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/members"),
        Some(&next_owner_token),
        json!({ "user_id": member_id, "role_in_case": "member" }),
    )
    .await;
    assert_eq!(update_member.0, StatusCode::OK);
    assert_eq!(
        update_member.1["data"]["joined_by"].as_str().unwrap_or(""),
        owner_id
    );
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

async fn request_multipart_text(
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

async fn build_test_app() -> (axum::Router, TempDir) {
    let (app, tmp, _pool) = build_test_app_with_pool().await;
    (app, tmp)
}

async fn build_test_app_with_pool() -> (axum::Router, TempDir, PgPool) {
    test_support::build_test_app_with_pool().await
}
