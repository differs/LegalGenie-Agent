use axum::body::Body;
use axum::http::HeaderMap;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use sqlx::PgPool;
use std::time::Duration;
use tempfile::TempDir;
use tower::util::ServiceExt;

mod test_support;

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
    assert_eq!(
        register.0,
        StatusCode::OK,
        "register body: {:?}",
        register.1
    );
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

    // Wait for async parse completion so the parsed artifact is available.
    let detail = wait_for_file_parse_done(&app, &token, &evidence_id).await;
    assert_eq!(detail.0, StatusCode::OK);
    assert_eq!(
        detail.1["data"]["parse_status"].as_str().unwrap_or(""),
        "done",
        "expected parse_status=done"
    );

    let parsed = request_raw(
        &app,
        Method::GET,
        &format!("/api/v1/files/{evidence_id}/parsed"),
        Some(&token),
    )
    .await;
    assert_eq!(parsed.0, StatusCode::OK);
    let ct = parsed
        .1
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("application/json"),
        "expected application/json content-type, got {ct}"
    );
    let parsed_json: serde_json::Value = serde_json::from_slice(&parsed.2).expect("parsed json");
    assert_eq!(
        parsed_json["evidence_id"].as_str().unwrap_or(""),
        evidence_id
    );
    assert_eq!(parsed_json["case_id"].as_str().unwrap_or(""), case_id);
    assert_eq!(parsed_json["kind"].as_str().unwrap_or(""), "text");
    assert!(
        parsed_json["parsed_text"]
            .as_str()
            .unwrap_or("")
            .contains("hello from tests"),
        "expected parsed_text to contain uploaded content"
    );

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

#[tokio::test]
async fn logout_invalidates_existing_refresh_token() {
    let (app, _tmp) = build_test_app().await;

    let register = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        json!({
            "username": "logoutuser",
            "email": "logoutuser@example.com",
            "password": "Password123",
        }),
    )
    .await;
    assert_eq!(register.0, StatusCode::OK);
    let access_token = register.1["data"]["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();
    let refresh_token = register.1["data"]["refresh_token"]
        .as_str()
        .expect("refresh_token")
        .to_string();

    let logout = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/logout",
        Some(&access_token),
        json!({}),
    )
    .await;
    assert_eq!(logout.0, StatusCode::OK);

    let refresh = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/refresh",
        None,
        json!({
            "refresh_token": refresh_token,
        }),
    )
    .await;
    assert_eq!(refresh.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn change_password_invalidates_existing_refresh_token() {
    let (app, _tmp) = build_test_app().await;

    let register = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        json!({
            "username": "pwchangeuser",
            "email": "pwchangeuser@example.com",
            "password": "Password123",
        }),
    )
    .await;
    assert_eq!(register.0, StatusCode::OK);
    let access_token = register.1["data"]["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();
    let refresh_token = register.1["data"]["refresh_token"]
        .as_str()
        .expect("refresh_token")
        .to_string();

    let change = request_json(
        &app,
        Method::PUT,
        "/api/v1/auth/password",
        Some(&access_token),
        json!({
            "old_password": "Password123",
            "new_password": "NewPassword123",
        }),
    )
    .await;
    assert_eq!(change.0, StatusCode::OK);

    let refresh = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/refresh",
        None,
        json!({
            "refresh_token": refresh_token,
        }),
    )
    .await;
    assert_eq!(refresh.0, StatusCode::UNAUTHORIZED);

    let login = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        json!({
            "username": "pwchangeuser",
            "password": "NewPassword123",
        }),
    )
    .await;
    assert_eq!(login.0, StatusCode::OK);
}

#[tokio::test]
async fn deleted_case_detail_returns_not_found() {
    let (app, _tmp) = build_test_app().await;

    let register = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        json!({
            "username": "deletedcaseuser",
            "email": "deletedcaseuser@example.com",
            "password": "Password123",
        }),
    )
    .await;
    assert_eq!(register.0, StatusCode::OK);
    let token = register.1["data"]["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();

    let case_resp = request_json(
        &app,
        Method::POST,
        "/api/v1/cases",
        Some(&token),
        json!({
            "name": "Deleted Case",
        }),
    )
    .await;
    assert_eq!(case_resp.0, StatusCode::OK);
    let case_id = case_resp.1["data"]["id"]
        .as_str()
        .expect("case id")
        .to_string();

    let delete_resp = request_json(
        &app,
        Method::DELETE,
        &format!("/api/v1/cases/{case_id}"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(delete_resp.0, StatusCode::OK);

    let detail = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(detail.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn move_timeline_node_resequences_same_day_order() {
    let (app, _tmp) = build_test_app().await;

    let register = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        json!({
            "username": "timelineuser",
            "email": "timelineuser@example.com",
            "password": "Password123",
        }),
    )
    .await;
    assert_eq!(register.0, StatusCode::OK);
    let token = register.1["data"]["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();

    let case_resp = request_json(
        &app,
        Method::POST,
        "/api/v1/cases",
        Some(&token),
        json!({
            "name": "Timeline Sort Case",
            "description": "desc",
        }),
    )
    .await;
    assert_eq!(case_resp.0, StatusCode::OK);
    let case_id = case_resp.1["data"]["id"]
        .as_str()
        .expect("case id")
        .to_string();

    let mut ids = Vec::new();
    for title in ["A", "B", "C"] {
        let node_resp = request_json(
            &app,
            Method::POST,
            &format!("/api/v1/cases/{case_id}/timeline/nodes"),
            Some(&token),
            json!({
                "title": title,
                "event_time": "2024-01-01",
            }),
        )
        .await;
        assert_eq!(node_resp.0, StatusCode::OK);
        ids.push(
            node_resp.1["data"]["id"]
                .as_str()
                .expect("node id")
                .to_string(),
        );
    }

    let move_resp = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/timeline/nodes/{}/move", ids[2]),
        Some(&token),
        json!({
            "new_time": "2024-01-01",
            "new_sort_order": 1,
        }),
    )
    .await;
    assert_eq!(move_resp.0, StatusCode::OK);

    let list_resp = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/timeline/nodes?page=1&page_size=20"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(list_resp.0, StatusCode::OK);

    let nodes = list_resp.1["data"]["nodes"]
        .as_array()
        .expect("nodes array");
    let titles = nodes
        .iter()
        .map(|node| node["title"].as_str().unwrap_or("").to_string())
        .collect::<Vec<_>>();
    assert_eq!(titles, vec!["C", "A", "B"]);

    let sort_orders = nodes
        .iter()
        .map(|node| node["sort_order"].as_i64().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(sort_orders, vec![100, 200, 300]);
}

#[tokio::test]
async fn timeline_tags_filter_returns_expected_nodes() {
    let (app, _tmp) = build_test_app().await;

    let register = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        json!({
            "username": "taguser",
            "email": "taguser@example.com",
            "password": "Password123",
        }),
    )
    .await;
    assert_eq!(register.0, StatusCode::OK);
    let token = register.1["data"]["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();

    let case_resp = request_json(
        &app,
        Method::POST,
        "/api/v1/cases",
        Some(&token),
        json!({
            "name": "Tags Filter Case",
        }),
    )
    .await;
    assert_eq!(case_resp.0, StatusCode::OK);
    let case_id = case_resp.1["data"]["id"]
        .as_str()
        .expect("case id")
        .to_string();

    let tagged = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/timeline/nodes"),
        Some(&token),
        json!({
            "title": "Tagged",
            "event_time": "2024-01-10",
            "tags": ["alpha", "beta"],
        }),
    )
    .await;
    assert_eq!(tagged.0, StatusCode::OK);
    let tagged_id = tagged.1["data"]["id"]
        .as_str()
        .expect("tagged node id")
        .to_string();

    let alpha_only = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/timeline/nodes"),
        Some(&token),
        json!({
            "title": "Alpha only",
            "event_time": "2024-01-11",
            "tags": ["alpha"],
        }),
    )
    .await;
    assert_eq!(alpha_only.0, StatusCode::OK);

    let list = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/timeline/nodes?tags=alpha,beta&page=1&page_size=50"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(list.0, StatusCode::OK);
    let nodes = list.1["data"]["nodes"].as_array().expect("nodes array");
    assert_eq!(nodes.len(), 1);
    assert_eq!(
        nodes[0]["id"].as_str().unwrap_or(""),
        tagged_id,
        "expected only the alpha+beta node to match tags=alpha,beta"
    );
}

#[tokio::test]
async fn upload_over_2mb_is_allowed() {
    let (app, _tmp) = build_test_app().await;

    let register = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        json!({
            "username": "biguser",
            "email": "biguser@example.com",
            "password": "Password123",
        }),
    )
    .await;
    assert_eq!(register.0, StatusCode::OK);
    let token = register.1["data"]["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();

    let case_resp = request_json(
        &app,
        Method::POST,
        "/api/v1/cases",
        Some(&token),
        json!({
            "name": "Big Upload Case",
        }),
    )
    .await;
    assert_eq!(case_resp.0, StatusCode::OK);
    let case_id = case_resp.1["data"]["id"]
        .as_str()
        .expect("case id")
        .to_string();

    // Regression test: Axum's default body limit is 2MB. Our upload route must allow larger uploads.
    let big = "a".repeat(2 * 1024 * 1024 + 10 * 1024);
    let upload = request_multipart_text(
        &app,
        &format!("/api/v1/cases/{case_id}/files"),
        &token,
        "big.txt",
        &big,
    )
    .await;
    assert_eq!(upload.0, StatusCode::OK);

    let file_size = upload.1["data"]["file_size"].as_i64().expect("file_size");
    assert!(
        file_size > (2 * 1024 * 1024) as i64,
        "expected file_size >2MB, got {file_size}"
    );
}

#[tokio::test]
async fn office_files_produce_parsed_artifacts() {
    let (app, _tmp) = build_test_app().await;

    // Register (also returns tokens).
    let register = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        json!({
            "username": "officeuser",
            "email": "officeuser@example.com",
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
            "name": "Office Parse Case",
        }),
    )
    .await;
    assert_eq!(case_resp.0, StatusCode::OK);
    let case_id = case_resp.1["data"]["id"]
        .as_str()
        .expect("case id")
        .to_string();

    // Upload XLSX and verify parsed artifact.
    let xlsx = make_test_xlsx_bytes();
    let upload_xlsx = request_multipart_bytes(
        &app,
        &format!("/api/v1/cases/{case_id}/files"),
        &token,
        "table.xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        &xlsx,
    )
    .await;
    assert_eq!(upload_xlsx.0, StatusCode::OK);
    let xlsx_id = upload_xlsx.1["data"]["id"]
        .as_str()
        .expect("xlsx evidence id")
        .to_string();

    let xlsx_detail = wait_for_file_parse_done(&app, &token, &xlsx_id).await;
    assert_eq!(xlsx_detail.0, StatusCode::OK);
    assert_eq!(
        xlsx_detail.1["data"]["parse_status"].as_str().unwrap_or(""),
        "done"
    );

    let xlsx_parsed = request_raw(
        &app,
        Method::GET,
        &format!("/api/v1/files/{xlsx_id}/parsed"),
        Some(&token),
    )
    .await;
    assert_eq!(xlsx_parsed.0, StatusCode::OK);
    let ct = xlsx_parsed
        .1
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("application/json"),
        "expected application/json content-type, got {ct}"
    );
    let xlsx_json: serde_json::Value =
        serde_json::from_slice(&xlsx_parsed.2).expect("xlsx parsed json");
    assert_eq!(xlsx_json["case_id"].as_str().unwrap_or(""), case_id);
    assert_eq!(xlsx_json["evidence_id"].as_str().unwrap_or(""), xlsx_id);
    assert_eq!(xlsx_json["kind"].as_str().unwrap_or(""), "excel");
    let xlsx_text = xlsx_json["parsed_text"].as_str().unwrap_or("");
    assert!(xlsx_text.contains("Alpha"), "expected Alpha in parsed xlsx");
    assert!(xlsx_text.contains("Beta"), "expected Beta in parsed xlsx");
    assert!(
        xlsx_json["extra"]["sheet_names"]
            .as_array()
            .map(|a| !a.is_empty())
            == Some(true),
        "expected extra.sheet_names to be a non-empty array"
    );

    // Upload DOCX and verify parsed artifact.
    let docx = make_test_docx_bytes("Hello DOCX");
    let upload_docx = request_multipart_bytes(
        &app,
        &format!("/api/v1/cases/{case_id}/files"),
        &token,
        "doc.docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        &docx,
    )
    .await;
    assert_eq!(upload_docx.0, StatusCode::OK);
    let docx_id = upload_docx.1["data"]["id"]
        .as_str()
        .expect("docx evidence id")
        .to_string();

    let docx_detail = wait_for_file_parse_done(&app, &token, &docx_id).await;
    assert_eq!(docx_detail.0, StatusCode::OK);
    assert_eq!(
        docx_detail.1["data"]["parse_status"].as_str().unwrap_or(""),
        "done"
    );

    let docx_parsed = request_raw(
        &app,
        Method::GET,
        &format!("/api/v1/files/{docx_id}/parsed"),
        Some(&token),
    )
    .await;
    assert_eq!(docx_parsed.0, StatusCode::OK);
    let docx_json: serde_json::Value =
        serde_json::from_slice(&docx_parsed.2).expect("docx parsed json");
    assert_eq!(docx_json["case_id"].as_str().unwrap_or(""), case_id);
    assert_eq!(docx_json["evidence_id"].as_str().unwrap_or(""), docx_id);
    assert_eq!(docx_json["kind"].as_str().unwrap_or(""), "docx");
    let docx_text = docx_json["parsed_text"].as_str().unwrap_or("");
    assert!(
        docx_text.contains("Hello DOCX"),
        "expected docx parsed text to contain content"
    );

    // Upload legacy DOC and verify it fails (unsupported) and has no parsed artifact.
    let upload_doc = request_multipart_bytes(
        &app,
        &format!("/api/v1/cases/{case_id}/files"),
        &token,
        "legacy.doc",
        "application/msword",
        b"not a real doc",
    )
    .await;
    assert_eq!(upload_doc.0, StatusCode::OK);
    let doc_id = upload_doc.1["data"]["id"]
        .as_str()
        .expect("doc evidence id")
        .to_string();

    let doc_detail = wait_for_file_parse_failed(&app, &token, &doc_id).await;
    assert_eq!(doc_detail.0, StatusCode::OK);
    assert_eq!(
        doc_detail.1["data"]["parse_status"].as_str().unwrap_or(""),
        "failed"
    );
    let err = doc_detail.1["data"]["parse_error"].as_str().unwrap_or("");
    assert!(
        err.contains("not supported") || err.contains(".doc"),
        "expected parse_error to mention unsupported doc, got: {err}"
    );

    let doc_parsed = request_raw(
        &app,
        Method::GET,
        &format!("/api/v1/files/{doc_id}/parsed"),
        Some(&token),
    )
    .await;
    assert_eq!(doc_parsed.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn legacy_file_without_chunks_still_remains_usable() {
    let (app, _tmp, pool) = build_test_app_with_pool().await;

    let register = request_json(
        &app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        json!({
            "username": "legacy_user",
            "email": "legacy_user@example.com",
            "password": "Password123",
        }),
    )
    .await;
    assert_eq!(register.0, StatusCode::OK);
    let token = register.1["data"]["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();

    let case_resp = request_json(
        &app,
        Method::POST,
        "/api/v1/cases",
        Some(&token),
        json!({
            "name": "Legacy Case",
        }),
    )
    .await;
    assert_eq!(case_resp.0, StatusCode::OK);
    let case_id = case_resp.1["data"]["id"]
        .as_str()
        .expect("case id")
        .to_string();

    let upload = request_multipart_text(
        &app,
        &format!("/api/v1/cases/{case_id}/files"),
        &token,
        "legacy.txt",
        "Legacy source text\n",
    )
    .await;
    assert_eq!(upload.0, StatusCode::OK);
    let file_id = upload.1["data"]["id"]
        .as_str()
        .expect("file id")
        .to_string();

    let detail = wait_for_file_parse_done(&app, &token, &file_id).await;
    assert_eq!(detail.0, StatusCode::OK);

    sqlx::query(
        r#"
        UPDATE evidence_files
        SET
            translation_status = 'failed',
            translation_error = 'reparse required for bilingual translation',
            chunk_count = 0,
            translated_chunk_count = 0,
            failed_chunk_count = 0
        WHERE id = $1
        "#,
    )
    .bind(&file_id)
    .execute(&pool)
    .await
    .expect("reset translation aggregate for legacy fallback");
    sqlx::query("DELETE FROM evidence_file_chunks WHERE evidence_id = $1")
        .bind(&file_id)
        .execute(&pool)
        .await
        .expect("delete chunks for legacy fallback");

    let detail = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/files/{file_id}"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(detail.0, StatusCode::OK);
    assert_eq!(detail.1["data"]["chunk_count"].as_i64(), Some(0));
    assert_eq!(
        detail.1["data"]["translation_status"].as_str(),
        Some("failed")
    );
    assert!(detail.1["data"]["parsed_text"]
        .as_str()
        .unwrap_or("")
        .contains("Legacy source text"));

    let search = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/search/evidence?keyword=Legacy&case_id={case_id}&language_mode=zh"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(search.0, StatusCode::OK);
    let results = search.1["data"]["results"]
        .as_array()
        .expect("results array");
    assert!(
        results.iter().any(|item| {
            item["file_id"].as_str() == Some(file_id.as_str())
                && item["source_fallback"].as_bool() == Some(true)
        }),
        "expected legacy fallback evidence hit in zh mode: {results:?}"
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

async fn request_multipart_bytes(
    app: &axum::Router,
    uri: &str,
    bearer: &str,
    filename: &str,
    content_type: &str,
    content: &[u8],
) -> (StatusCode, serde_json::Value) {
    let boundary = "XBOUNDARY";
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
Content-Type: {content_type}\r\n\
\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

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

async fn wait_for_file_parse_done(
    app: &axum::Router,
    bearer: &str,
    file_id: &str,
) -> (StatusCode, serde_json::Value) {
    wait_for_file_parse_status(app, bearer, file_id, "done").await
}

async fn wait_for_file_parse_failed(
    app: &axum::Router,
    bearer: &str,
    file_id: &str,
) -> (StatusCode, serde_json::Value) {
    wait_for_file_parse_status(app, bearer, file_id, "failed").await
}

async fn wait_for_file_parse_status(
    app: &axum::Router,
    bearer: &str,
    file_id: &str,
    expected: &str,
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
        if resp.0 == StatusCode::OK && status == expected {
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

fn make_test_xlsx_bytes() -> Vec<u8> {
    use rust_xlsxwriter::Workbook;

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    worksheet
        .write_string(0, 0, "Alpha")
        .expect("write xlsx cell");
    worksheet
        .write_string(0, 1, "Beta")
        .expect("write xlsx cell");

    workbook.save_to_buffer().expect("save xlsx to buffer")
}

fn make_test_docx_bytes(text: &str) -> Vec<u8> {
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    // Minimal docx-like zip payload: we only need word/document.xml for our parser.
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>{text}</w:t></w:r></w:p>
  </w:body>
</w:document>
"#
    );

    let cursor = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(cursor);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("word/document.xml", opts)
        .expect("start docx document.xml");
    zip.write_all(xml.as_bytes()).expect("write docx xml");
    let cursor = zip.finish().expect("finish docx zip");
    cursor.into_inner()
}

async fn build_test_app() -> (axum::Router, TempDir) {
    let (app, tmp, _pool) = build_test_app_with_pool().await;
    (app, tmp)
}

async fn build_test_app_with_pool() -> (axum::Router, TempDir, PgPool) {
    test_support::build_test_app_with_pool().await
}
