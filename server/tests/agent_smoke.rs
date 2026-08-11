// P2 smoke tests: agent sessions, tool pipeline, approval flow.
mod test_support;

use axum::http::{Method, StatusCode};
use serde_json::json;
use test_support::{build_test_app_with_pool, create_case, register_user, request_json};

async fn create_session(app: &axum::Router, token: &str, case_id: &str) -> String {
    let resp = request_json(
        &app,
        Method::POST,
        "/api/v1/agent/sessions",
        Some(token),
        json!({ "case_id": case_id }),
    )
    .await;
    assert_eq!(resp.0, StatusCode::OK, "create session: {:?}", resp.1);
    resp.1["data"]["id"]
        .as_str()
        .expect("session id")
        .to_string()
}

async fn post_message(
    app: &axum::Router,
    token: &str,
    session_id: &str,
    content: &str,
) -> serde_json::Value {
    let resp = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/agent/sessions/{session_id}/messages"),
        Some(token),
        json!({ "content": content }),
    )
    .await;
    assert_eq!(resp.0, StatusCode::OK, "post message: {:?}", resp.1);
    resp.1["data"].clone()
}

#[tokio::test]
async fn read_tools_execute_immediately() {
    let (app, _tmp, _pool) = build_test_app_with_pool().await;
    let (_uid, token) = register_user(&app, "agent_read", "agent-read@test.local").await;
    let case_id = create_case(&app, &token, "Agent Read", "read tools").await;

    // Node creation via direct API for setup.
    let node = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/cases/{case_id}/timeline/nodes"),
        Some(&token),
        json!({ "title": "签约", "event_time": "2024-01-12" }),
    )
    .await;
    assert_eq!(node.0, StatusCode::OK);

    let session = create_session(&app, &token, &case_id).await;

    let run = post_message(&app, &token, &session, "列出这个案件的时间轴节点").await;
    assert_eq!(
        run["assistant_text"].as_str().unwrap_or(""),
        "共 1 个节点：签约"
    );
    assert_eq!(run["tool_calls"][0]["tool"].as_str(), Some("list_nodes"));
    assert_eq!(run["tool_calls"][0]["status"].as_str(), Some("executed"));
    assert!(run["pending_approvals"].as_array().unwrap().is_empty());

    // Session detail contains the persisted messages.
    let detail = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/agent/sessions/{session}"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(detail.0, StatusCode::OK);
    assert_eq!(detail.1["data"]["messages"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn write_tool_requires_approval_then_executes() {
    let (app, _tmp, _pool) = build_test_app_with_pool().await;
    let (_uid, token) = register_user(&app, "agent_write", "agent-write@test.local").await;
    let case_id = create_case(&app, &token, "Agent Write", "approval flow").await;

    let session = create_session(&app, &token, &case_id).await;

    let run = post_message(&app, &token, &session, "新建节点「补充协议」2024-02-15").await;
    assert_eq!(run["tool_calls"][0]["tool"].as_str(), Some("create_node"));
    assert_eq!(run["tool_calls"][0]["status"].as_str(), Some("pending"));
    assert_eq!(run["tool_calls"][0]["danger_level"].as_str(), Some("write"));
    assert_eq!(run["pending_approvals"].as_array().unwrap().len(), 1);

    let approval_id = run["pending_approvals"][0]["id"]
        .as_str()
        .expect("approval id");

    // Nothing created yet.
    let nodes = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/timeline/nodes"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(
        nodes.1["data"]["total"], 0,
        "write must not happen before approval"
    );

    // Pending list exposes it.
    let pending = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/agent/approvals/pending?case_id={case_id}"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(pending.0, StatusCode::OK);
    assert_eq!(pending.1["data"]["approvals"].as_array().unwrap().len(), 1);

    // Confirm -> executed -> node exists.
    let confirm = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/agent/approvals/{approval_id}/confirm"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(confirm.0, StatusCode::OK, "confirm: {:?}", confirm.1);
    assert_eq!(confirm.1["data"]["status"].as_str(), Some("executed"));

    let nodes = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/timeline/nodes"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(nodes.1["data"]["total"], 1);
    assert_eq!(
        nodes.1["data"]["nodes"][0]["title"].as_str(),
        Some("补充协议")
    );

    // Double confirm is rejected (already decided).
    let again = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/agent/approvals/{approval_id}/confirm"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(again.0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn reject_approval_does_not_execute() {
    let (app, _tmp, _pool) = build_test_app_with_pool().await;
    let (_uid, token) = register_user(&app, "agent_reject", "agent-reject@test.local").await;
    let case_id = create_case(&app, &token, "Agent Reject", "reject flow").await;

    let session = create_session(&app, &token, &case_id).await;
    let run = post_message(&app, &token, &session, "新建节点「取消」2024-03-01").await;
    let approval_id = run["pending_approvals"][0]["id"]
        .as_str()
        .expect("approval id")
        .to_string();

    let reject = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/agent/approvals/{approval_id}/reject"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(reject.0, StatusCode::OK);
    assert_eq!(reject.1["data"]["status"].as_str(), Some("rejected"));

    let nodes = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/cases/{case_id}/timeline/nodes"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(nodes.1["data"]["total"], 0);
}

#[tokio::test]
async fn approvals_are_scoped_to_the_requesting_user() {
    let (app, _tmp, _pool) = build_test_app_with_pool().await;
    let (owner_id, owner_token) =
        register_user(&app, "agent_owner", "agent-owner@test.local").await;
    let (_other_id, other_token) =
        register_user(&app, "agent_other", "agent-other@test.local").await;
    let case_id = create_case(&app, &owner_token, "Agent Scoped", "scoping").await;

    let session = create_session(&app, &owner_token, &case_id).await;
    let run = post_message(&app, &owner_token, &session, "导出证据清单").await;
    let approval_id = run["pending_approvals"][0]["id"]
        .as_str()
        .expect("approval id");

    // Another user cannot see or confirm it.
    let pending = request_json(
        &app,
        Method::GET,
        &format!("/api/v1/agent/approvals/pending?case_id={case_id}"),
        Some(&other_token),
        json!({}),
    )
    .await;
    assert!(pending.1["data"]["approvals"]
        .as_array()
        .unwrap()
        .is_empty());

    let confirm = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/agent/approvals/{approval_id}/confirm"),
        Some(&other_token),
        json!({}),
    )
    .await;
    assert_eq!(
        confirm.0,
        StatusCode::NOT_FOUND,
        "cross-user confirm must fail"
    );
    let _ = owner_id;
}

#[tokio::test]
async fn search_tool_and_export_tool_flow() {
    let (app, _tmp, _pool) = build_test_app_with_pool().await;
    let (_uid, token) = register_user(&app, "agent_search", "agent-search@test.local").await;
    let case_id = create_case(&app, &token, "Agent Search", "search tool").await;

    let session = create_session(&app, &token, &case_id).await;

    let run = post_message(&app, &token, &session, "搜索 不存在关键词xyz").await;
    assert_eq!(run["tool_calls"][0]["tool"].as_str(), Some("search"));
    assert!(run["assistant_text"]
        .as_str()
        .unwrap_or("")
        .contains("没有找到"));

    // Export evidence list -> write tool -> pending.
    let run = post_message(&app, &token, &session, "导出证据清单").await;
    assert_eq!(
        run["tool_calls"][0]["tool"].as_str(),
        Some("export_evidence_list")
    );
    assert_eq!(run["tool_calls"][0]["status"].as_str(), Some("pending"));
    let approval_id = run["pending_approvals"][0]["id"]
        .as_str()
        .expect("approval id");

    let confirm = request_json(
        &app,
        Method::POST,
        &format!("/api/v1/agent/approvals/{approval_id}/confirm"),
        Some(&token),
        json!({}),
    )
    .await;
    assert_eq!(confirm.0, StatusCode::OK, "export confirm: {:?}", confirm.1);
    assert_eq!(confirm.1["data"]["status"].as_str(), Some("executed"));
    assert!(confirm.1["data"]["output"]["file_name"]
        .as_str()
        .unwrap_or("")
        .ends_with(".xlsx"));
}
