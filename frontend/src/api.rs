use crate::models::ApiEnvelope;
use serde::de::DeserializeOwned;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct DownloadedFile {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
    pub filename: Option<String>,
}

pub fn build_url(base: &str, path_and_query: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    if base.is_empty() {
        return path_and_query.to_string();
    }
    if path_and_query.starts_with('/') {
        format!("{base}{path_and_query}")
    } else {
        format!("{base}/{path_and_query}")
    }
}

pub async fn post_login(
    base: &str,
    username: &str,
    password: &str,
) -> Result<crate::models::LoginResponseData, String> {
    let url = build_url(base, "/api/v1/auth/login");
    let env: ApiEnvelope<crate::models::LoginResponseData> = request_json(
        "POST",
        &url,
        None,
        Some(json!({
            "username": username,
            "password": password,
        })),
    )
    .await?;
    env.into_data()
}

pub async fn post_register(
    base: &str,
    username: &str,
    email: &str,
    password: &str,
    real_name: Option<&str>,
) -> Result<crate::models::LoginResponseData, String> {
    let url = build_url(base, "/api/v1/auth/register");
    let env: ApiEnvelope<crate::models::LoginResponseData> = request_json(
        "POST",
        &url,
        None,
        Some(json!({
            "username": username,
            "email": email,
            "password": password,
            "real_name": real_name,
        })),
    )
    .await?;
    env.into_data()
}

pub async fn get_me(base: &str, token: &str) -> Result<crate::models::UserInfo, String> {
    let url = build_url(base, "/api/v1/auth/me");
    let env: ApiEnvelope<crate::models::UserInfo> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

#[allow(dead_code)]
pub async fn get_cases(
    base: &str,
    token: &str,
    page: i64,
    page_size: i64,
) -> Result<crate::models::CaseListData, String> {
    let url = build_url(
        base,
        &format!("/api/v1/cases?page={page}&page_size={page_size}"),
    );
    let env: ApiEnvelope<crate::models::CaseListData> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

#[allow(dead_code)]
pub async fn post_create_case(
    base: &str,
    token: &str,
    name: &str,
    description: Option<&str>,
) -> Result<crate::models::CaseDetail, String> {
    let url = build_url(base, "/api/v1/cases");
    let env: ApiEnvelope<crate::models::CaseDetail> = request_json(
        "POST",
        &url,
        Some(token),
        Some(json!({
            "name": name,
            "description": description,
        })),
    )
    .await?;
    env.into_data()
}

pub async fn get_case_member_me(
    base: &str,
    token: &str,
    case_id: &str,
) -> Result<crate::models::CaseMemberMeData, String> {
    let url = build_url(base, &format!("/api/v1/cases/{case_id}/members/me"));
    let env: ApiEnvelope<crate::models::CaseMemberMeData> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn post_upload_case_file(
    base: &str,
    token: &str,
    case_id: &str,
    filename: &str,
    bytes: &[u8],
) -> Result<crate::models::EvidenceFileDetail, String> {
    let url = build_url(base, &format!("/api/v1/cases/{case_id}/files"));
    let boundary = "LMBOUNDARYf3a5d8e2b7c14b9aa1e4c7f0d9b2a6c1";
    let content_type = format!("multipart/form-data; boundary={boundary}");
    let body = build_upload_multipart(boundary, filename, bytes);

    #[cfg(target_arch = "wasm32")]
    {
        use gloo_net::http::Request;
        use js_sys::Uint8Array;

        let arr = Uint8Array::from(body.as_slice());
        let req = Request::post(&url)
            .header("Authorization", &format!("Bearer {token}"))
            .header("Content-Type", &content_type)
            .body(arr)
            .map_err(|e| e.to_string())?;

        let resp = req.send().await.map_err(|e| e.to_string())?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        let env: ApiEnvelope<crate::models::EvidenceFileDetail> = serde_json::from_str(&text)
            .map_err(|e| {
                format!(
                    "http {status}: failed to parse json: {e}; body={}",
                    truncate(&text, 512)
                )
            })?;
        env.into_data()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use reqwest::Client;

        let client = Client::new();
        let resp = client
            .post(&url)
            .bearer_auth(token)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        let env: ApiEnvelope<crate::models::EvidenceFileDetail> = serde_json::from_str(&text)
            .map_err(|e| {
                format!(
                    "http {status}: failed to parse json: {e}; body={}",
                    truncate(&text, 512)
                )
            })?;
        env.into_data()
    }
}

pub async fn get_case_files(
    base: &str,
    token: &str,
    case_id: &str,
    page: i64,
    page_size: i64,
) -> Result<crate::models::EvidenceFileListData, String> {
    let url = build_url(
        base,
        &format!("/api/v1/cases/{case_id}/files?page={page}&page_size={page_size}"),
    );
    let env: ApiEnvelope<crate::models::EvidenceFileListData> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn get_file_detail(
    base: &str,
    token: &str,
    file_id: &str,
) -> Result<crate::models::EvidenceFileDetail, String> {
    let url = build_url(base, &format!("/api/v1/files/{file_id}"));
    let env: ApiEnvelope<crate::models::EvidenceFileDetail> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn get_file_chunks(
    base: &str,
    token: &str,
    file_id: &str,
    page: i64,
    page_size: i64,
    view_mode: Option<&str>,
) -> Result<crate::models::EvidenceFileChunkListData, String> {
    let mut parts = Vec::new();
    parts.push(format!("page={}", page.max(1)));
    parts.push(format!("page_size={}", page_size.clamp(1, 200)));
    if let Some(mode) = view_mode {
        let mode = mode.trim();
        if !mode.is_empty() {
            parts.push(format!("view_mode={}", urlencoding::encode(mode)));
        }
    }

    let url = build_url(
        base,
        &format!("/api/v1/files/{file_id}/chunks?{}", parts.join("&")),
    );
    let env: ApiEnvelope<crate::models::EvidenceFileChunkListData> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn get_file_translation(
    base: &str,
    token: &str,
    file_id: &str,
) -> Result<crate::models::EvidenceFileTranslationDetail, String> {
    let url = build_url(base, &format!("/api/v1/files/{file_id}/translation"));
    let env: ApiEnvelope<crate::models::EvidenceFileTranslationDetail> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn post_retry_translation(
    base: &str,
    token: &str,
    file_id: &str,
    scope: &str,
    chunk_ids: Option<Vec<String>>,
) -> Result<crate::models::RetryTranslationResult, String> {
    let url = build_url(base, &format!("/api/v1/files/{file_id}/translate/retry"));
    let body = json!({
        "scope": scope,
        "chunk_ids": chunk_ids,
    });
    let env: ApiEnvelope<crate::models::RetryTranslationResult> =
        request_json("POST", &url, Some(token), Some(body)).await?;
    env.into_data()
}

pub async fn post_parse_file(
    base: &str,
    token: &str,
    file_id: &str,
) -> Result<serde_json::Value, String> {
    let url = build_url(base, &format!("/api/v1/files/{file_id}/parse"));
    let env: ApiEnvelope<serde_json::Value> = request_json("POST", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn get_export_history(
    base: &str,
    token: &str,
    case_id: &str,
    page: i64,
    page_size: i64,
) -> Result<crate::models::ExportHistoryData, String> {
    let url = build_url(
        base,
        &format!("/api/v1/cases/{case_id}/exports/history?page={page}&page_size={page_size}"),
    );
    let env: ApiEnvelope<crate::models::ExportHistoryData> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn get_logs(
    base: &str,
    token: &str,
    query: &str,
) -> Result<crate::models::LogsListData, String> {
    let url = build_url(base, &format!("/api/v1/logs?{query}"));
    let env: ApiEnvelope<crate::models::LogsListData> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn get_target_history(
    base: &str,
    token: &str,
    target_type: &str,
    target_id: &str,
) -> Result<crate::models::TargetHistoryData, String> {
    let url = build_url(
        base,
        &format!("/api/v1/logs/{target_type}/{target_id}/history"),
    );
    let env: ApiEnvelope<crate::models::TargetHistoryData> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn download_file(
    base: &str,
    token: &str,
    path_and_query: &str,
) -> Result<DownloadedFile, String> {
    let url = build_url(base, path_and_query);
    request_bytes("GET", &url, Some(token)).await
}

// Timeline

pub async fn get_timeline_nodes(
    base: &str,
    token: &str,
    case_id: &str,
    start_date: Option<&str>,
    end_date: Option<&str>,
    tags: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<crate::models::TimelineNodeListData, String> {
    let mut parts = Vec::new();
    parts.push(format!("page={}", page.max(1)));
    parts.push(format!("page_size={}", page_size.clamp(1, 500)));
    if let Some(s) = start_date {
        let s = s.trim();
        if !s.is_empty() {
            parts.push(format!("start_date={}", urlencoding::encode(s)));
        }
    }
    if let Some(s) = end_date {
        let s = s.trim();
        if !s.is_empty() {
            parts.push(format!("end_date={}", urlencoding::encode(s)));
        }
    }
    if let Some(raw) = tags {
        let raw = raw.trim();
        if !raw.is_empty() {
            parts.push(format!("tags={}", urlencoding::encode(raw)));
        }
    }

    let url = build_url(
        base,
        &format!("/api/v1/cases/{case_id}/timeline/nodes?{}", parts.join("&")),
    );
    let env: ApiEnvelope<crate::models::TimelineNodeListData> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn get_timeline_node_detail(
    base: &str,
    token: &str,
    node_id: &str,
) -> Result<crate::models::TimelineNode, String> {
    let url = build_url(base, &format!("/api/v1/timeline/nodes/{node_id}"));
    let env: ApiEnvelope<crate::models::TimelineNode> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn post_create_timeline_node(
    base: &str,
    token: &str,
    case_id: &str,
    title: &str,
    description: Option<&str>,
    event_time: &str,
    tags: Vec<String>,
) -> Result<crate::models::TimelineNode, String> {
    let url = build_url(base, &format!("/api/v1/cases/{case_id}/timeline/nodes"));
    let body = json!({
        "title": title,
        "description": description,
        "event_time": event_time,
        "tags": tags,
    });
    let env: ApiEnvelope<crate::models::TimelineNode> =
        request_json("POST", &url, Some(token), Some(body)).await?;
    env.into_data()
}

pub async fn put_update_timeline_node(
    base: &str,
    token: &str,
    node_id: &str,
    title: Option<&str>,
    description: Option<&str>,
    event_time: Option<&str>,
    tags: Option<Vec<String>>,
) -> Result<crate::models::TimelineNode, String> {
    let url = build_url(base, &format!("/api/v1/timeline/nodes/{node_id}"));
    let body = json!({
        "title": title,
        "description": description,
        "event_time": event_time,
        "tags": tags,
    });
    let env: ApiEnvelope<crate::models::TimelineNode> =
        request_json("PUT", &url, Some(token), Some(body)).await?;
    env.into_data()
}

pub async fn post_move_timeline_node(
    base: &str,
    token: &str,
    node_id: &str,
    new_time: &str,
    new_sort_order: Option<i64>,
) -> Result<crate::models::TimelineNode, String> {
    let url = build_url(base, &format!("/api/v1/timeline/nodes/{node_id}/move"));
    let body = json!({
        "new_time": new_time,
        "new_sort_order": new_sort_order,
    });
    let env: ApiEnvelope<crate::models::TimelineNode> =
        request_json("POST", &url, Some(token), Some(body)).await?;
    env.into_data()
}

pub async fn delete_timeline_node(
    base: &str,
    token: &str,
    node_id: &str,
) -> Result<serde_json::Value, String> {
    let url = build_url(base, &format!("/api/v1/timeline/nodes/{node_id}"));
    let env: ApiEnvelope<serde_json::Value> =
        request_json("DELETE", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn post_link_node_evidence(
    base: &str,
    token: &str,
    node_id: &str,
    evidence_id: &str,
    anchor_type: &str,
    anchor_data: Option<serde_json::Value>,
) -> Result<crate::models::TimelineEvidenceLink, String> {
    let url = build_url(base, &format!("/api/v1/timeline/nodes/{node_id}/evidence"));
    let body = json!({
        "evidence_id": evidence_id,
        "anchor_type": anchor_type,
        "anchor_data": anchor_data,
    });
    let env: ApiEnvelope<crate::models::TimelineEvidenceLink> =
        request_json("POST", &url, Some(token), Some(body)).await?;
    env.into_data()
}

pub async fn delete_unlink_node_evidence(
    base: &str,
    token: &str,
    node_id: &str,
    link_id: &str,
) -> Result<serde_json::Value, String> {
    let url = build_url(
        base,
        &format!("/api/v1/timeline/nodes/{node_id}/evidence/{link_id}"),
    );
    let env: ApiEnvelope<serde_json::Value> =
        request_json("DELETE", &url, Some(token), None).await?;
    env.into_data()
}

// Persons

pub async fn get_case_persons(
    base: &str,
    token: &str,
    case_id: &str,
    page: i64,
    page_size: i64,
) -> Result<crate::models::CasePersonListData, String> {
    let url = build_url(
        base,
        &format!("/api/v1/cases/{case_id}/persons?page={page}&page_size={page_size}"),
    );
    let env: ApiEnvelope<crate::models::CasePersonListData> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn get_case_person_graph(
    base: &str,
    token: &str,
    case_id: &str,
) -> Result<crate::models::PersonGraphData, String> {
    let url = build_url(base, &format!("/api/v1/cases/{case_id}/persons/graph"));
    let env: ApiEnvelope<crate::models::PersonGraphData> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn post_create_case_person(
    base: &str,
    token: &str,
    case_id: &str,
    name: &str,
    gender: Option<&str>,
    phone: Option<&str>,
    email: Option<&str>,
    organization: Option<&str>,
    position: Option<&str>,
    role_type: Option<&str>,
    role_detail: Option<&str>,
    involved_date: Option<&str>,
) -> Result<crate::models::CasePersonItem, String> {
    let url = build_url(base, &format!("/api/v1/cases/{case_id}/persons"));
    let body = json!({
        "name": name,
        "gender": gender,
        "phone": phone,
        "email": email,
        "organization": organization,
        "position": position,
        "role_type": role_type,
        "role_detail": role_detail,
        "involved_date": involved_date,
    });
    let env: ApiEnvelope<crate::models::CasePersonItem> =
        request_json("POST", &url, Some(token), Some(body)).await?;
    env.into_data()
}

pub async fn get_person_detail(
    base: &str,
    token: &str,
    person_id: &str,
) -> Result<crate::models::PersonDetail, String> {
    let url = build_url(base, &format!("/api/v1/persons/{person_id}"));
    let env: ApiEnvelope<crate::models::PersonDetail> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn put_update_person(
    base: &str,
    token: &str,
    person_id: &str,
    name: Option<&str>,
    gender: Option<&str>,
    phone: Option<&str>,
    email: Option<&str>,
    organization: Option<&str>,
    position: Option<&str>,
    notes: Option<&str>,
) -> Result<crate::models::PersonDetail, String> {
    let url = build_url(base, &format!("/api/v1/persons/{person_id}"));
    let body = json!({
        "name": name,
        "gender": gender,
        "phone": phone,
        "email": email,
        "organization": organization,
        "position": position,
        "notes": notes,
    });
    let env: ApiEnvelope<crate::models::PersonDetail> =
        request_json("PUT", &url, Some(token), Some(body)).await?;
    env.into_data()
}

pub async fn delete_person(
    base: &str,
    token: &str,
    person_id: &str,
) -> Result<serde_json::Value, String> {
    let url = build_url(base, &format!("/api/v1/persons/{person_id}"));
    let env: ApiEnvelope<serde_json::Value> =
        request_json("DELETE", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn post_person_link_case(
    base: &str,
    token: &str,
    person_id: &str,
    case_id: &str,
    role_type: &str,
    role_detail: Option<&str>,
    involved_date: Option<&str>,
) -> Result<serde_json::Value, String> {
    let url = build_url(base, &format!("/api/v1/persons/{person_id}/cases"));
    let body = json!({
        "case_id": case_id,
        "role_type": role_type,
        "role_detail": role_detail,
        "involved_date": involved_date,
    });
    let env: ApiEnvelope<serde_json::Value> =
        request_json("POST", &url, Some(token), Some(body)).await?;
    env.into_data()
}

pub async fn get_case_persons_dedupe(
    base: &str,
    token: &str,
    case_id: &str,
) -> Result<crate::models::DedupeCandidatesData, String> {
    let url = build_url(base, &format!("/api/v1/cases/{case_id}/persons/dedupe"));
    let env: ApiEnvelope<crate::models::DedupeCandidatesData> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn post_merge_case_persons(
    base: &str,
    token: &str,
    case_id: &str,
    source_person_id: &str,
    target_person_id: &str,
) -> Result<crate::models::MergeCasePersonsData, String> {
    let url = build_url(base, &format!("/api/v1/cases/{case_id}/persons/merge"));
    let body = json!({
        "source_person_id": source_person_id,
        "target_person_id": target_person_id,
    });
    let env: ApiEnvelope<crate::models::MergeCasePersonsData> =
        request_json("POST", &url, Some(token), Some(body)).await?;
    env.into_data()
}

pub async fn get_case_person_relationships(
    base: &str,
    token: &str,
    case_id: &str,
) -> Result<crate::models::RelationshipListData, String> {
    let url = build_url(
        base,
        &format!("/api/v1/cases/{case_id}/persons/relationships"),
    );
    let env: ApiEnvelope<crate::models::RelationshipListData> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn post_create_case_person_relationship(
    base: &str,
    token: &str,
    case_id: &str,
    from_person_id: &str,
    to_person_id: &str,
    rel_type: &str,
    rel_detail: Option<&str>,
) -> Result<crate::models::RelationshipItem, String> {
    let url = build_url(
        base,
        &format!("/api/v1/cases/{case_id}/persons/relationships"),
    );
    let body = json!({
        "from_person_id": from_person_id,
        "to_person_id": to_person_id,
        "rel_type": rel_type,
        "rel_detail": rel_detail,
    });
    let env: ApiEnvelope<crate::models::RelationshipItem> =
        request_json("POST", &url, Some(token), Some(body)).await?;
    env.into_data()
}

pub async fn delete_case_person_relationship(
    base: &str,
    token: &str,
    case_id: &str,
    relationship_id: &str,
) -> Result<serde_json::Value, String> {
    let url = build_url(
        base,
        &format!("/api/v1/cases/{case_id}/persons/relationships/{relationship_id}"),
    );
    let env: ApiEnvelope<serde_json::Value> =
        request_json("DELETE", &url, Some(token), None).await?;
    env.into_data()
}

// Search

pub async fn get_search(
    base: &str,
    token: &str,
    keyword: &str,
    object_types: Option<&str>,
    case_id: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<crate::models::SearchResponseData, String> {
    get_search_with_language_mode(
        base,
        token,
        keyword,
        object_types,
        case_id,
        None,
        page,
        page_size,
    )
    .await
}

pub async fn get_search_with_language_mode(
    base: &str,
    token: &str,
    keyword: &str,
    object_types: Option<&str>,
    case_id: Option<&str>,
    language_mode: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<crate::models::SearchResponseData, String> {
    let url = build_search_url(
        base,
        keyword,
        object_types,
        case_id,
        language_mode,
        page,
        page_size,
    );
    let env: ApiEnvelope<crate::models::SearchResponseData> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

fn build_search_url(
    base: &str,
    keyword: &str,
    object_types: Option<&str>,
    case_id: Option<&str>,
    language_mode: Option<&str>,
    page: i64,
    page_size: i64,
) -> String {
    let mut parts = Vec::new();
    parts.push(format!("keyword={}", urlencoding::encode(keyword.trim())));
    parts.push(format!("page={}", page.max(1)));
    parts.push(format!("page_size={}", page_size.clamp(1, 200)));

    if let Some(mode) = language_mode {
        let mode = mode.trim();
        if !mode.is_empty() {
            parts.push(format!("language_mode={}", urlencoding::encode(mode)));
        }
    }
    if let Some(ot) = object_types {
        let ot = ot.trim();
        if !ot.is_empty() {
            parts.push(format!("object_types={}", urlencoding::encode(ot)));
        }
    }
    if let Some(cid) = case_id {
        let cid = cid.trim();
        if !cid.is_empty() {
            parts.push(format!("case_id={}", urlencoding::encode(cid)));
        }
    }

    build_url(base, &format!("/api/v1/search?{}", parts.join("&")))
}

pub async fn get_search_suggestions(
    base: &str,
    token: &str,
    keyword: &str,
) -> Result<crate::models::SuggestionsData, String> {
    let kw = keyword.trim();
    let url = build_url(
        base,
        &format!(
            "/api/v1/search/suggestions?keyword={}",
            urlencoding::encode(kw)
        ),
    );
    let env: ApiEnvelope<crate::models::SuggestionsData> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn get_search_history(
    base: &str,
    token: &str,
    page: i64,
    page_size: i64,
) -> Result<crate::models::SearchHistoryData, String> {
    let url = build_url(
        base,
        &format!("/api/v1/search/history?page={page}&page_size={page_size}"),
    );
    let env: ApiEnvelope<crate::models::SearchHistoryData> =
        request_json("GET", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn delete_search_history(base: &str, token: &str) -> Result<serde_json::Value, String> {
    let url = build_url(base, "/api/v1/search/history");
    let env: ApiEnvelope<serde_json::Value> =
        request_json("DELETE", &url, Some(token), None).await?;
    env.into_data()
}

pub async fn download_and_save(
    base: &str,
    token: &str,
    path_and_query: &str,
    fallback_name: &str,
) -> Result<String, String> {
    let file = download_file(base, token, path_and_query).await?;
    let filename = file
        .filename
        .as_deref()
        .unwrap_or(fallback_name)
        .to_string();
    save_download(&filename, file.content_type.as_deref(), &file.bytes)
}

pub fn save_download(
    filename: &str,
    content_type: Option<&str>,
    bytes: &[u8],
) -> Result<String, String> {
    let filename = sanitize_filename(filename);

    #[cfg(target_arch = "wasm32")]
    {
        trigger_browser_download(&filename, content_type, bytes)?;
        Ok(format!("download started: {filename}"))
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = content_type;
        let dir = std::path::PathBuf::from("downloads");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(&filename);
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(format!("saved: {}", path.display()))
    }
}

async fn request_json<T: DeserializeOwned>(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> Result<T, String> {
    #[cfg(target_arch = "wasm32")]
    {
        use gloo_net::http::Request;

        let mut req = match method {
            "GET" => Request::get(url),
            "POST" => Request::post(url),
            "PUT" => Request::put(url),
            "DELETE" => Request::delete(url),
            other => return Err(format!("unsupported method (wasm): {other}")),
        };
        if let Some(t) = token {
            req = req.header("Authorization", &format!("Bearer {t}"));
        }
        let resp = match body {
            Some(b) => req
                .header("Content-Type", "application/json")
                .body(b.to_string())
                .map_err(|e| e.to_string())?
                .send()
                .await
                .map_err(|e| e.to_string())?,
            None => req.send().await.map_err(|e| e.to_string())?,
        };

        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        serde_json::from_str::<T>(&text).map_err(|e| {
            format!(
                "http {status}: failed to parse json: {e}; body={}",
                truncate(&text, 512)
            )
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use reqwest::Client;

        let client = Client::new();
        let mut req = client.request(
            method
                .parse()
                .map_err(|_| format!("invalid method: {method}"))?,
            url,
        );
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        if let Some(b) = body {
            req = req.json(&b);
        }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        serde_json::from_str::<T>(&text).map_err(|e| {
            format!(
                "http {status}: failed to parse json: {e}; body={}",
                truncate(&text, 512)
            )
        })
    }
}

async fn request_bytes(
    method: &str,
    url: &str,
    token: Option<&str>,
) -> Result<DownloadedFile, String> {
    #[cfg(target_arch = "wasm32")]
    {
        use gloo_net::http::Request;

        let mut req = match method {
            "GET" => Request::get(url),
            other => return Err(format!("unsupported method (wasm): {other}")),
        };
        if let Some(t) = token {
            req = req.header("Authorization", &format!("Bearer {t}"));
        }
        let resp = req.send().await.map_err(|e| e.to_string())?;

        let content_type = resp.headers().get("content-type").map(|s| s.to_string());
        let filename = resp
            .headers()
            .get("content-disposition")
            .as_deref()
            .and_then(filename_from_content_disposition);

        let bytes = resp.binary().await.map_err(|e| e.to_string())?;
        Ok(DownloadedFile {
            bytes,
            content_type,
            filename,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use reqwest::Client;

        let client = Client::new();
        let mut req = client.request(
            method
                .parse()
                .map_err(|_| format!("invalid method: {method}"))?,
            url,
        );
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let filename = resp
            .headers()
            .get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .and_then(filename_from_content_disposition);

        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        Ok(DownloadedFile {
            bytes: bytes.to_vec(),
            content_type,
            filename,
        })
    }
}

fn filename_from_content_disposition(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let idx = raw.to_ascii_lowercase().find("filename=")?;
    let rest = raw[idx + "filename=".len()..].trim_start();
    let rest = rest.strip_prefix('"').unwrap_or(rest);
    let end = rest
        .find('"')
        .or_else(|| rest.find(';'))
        .unwrap_or(rest.len());
    let name = rest[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut out = s[..max].to_string();
        out.push_str("...");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_language_mode_toggle_updates_request() {
        let base = "http://localhost:8080";
        let url_zh = build_search_url(base, "alpha", None, None, Some("zh"), 1, 20);
        let url_src = build_search_url(base, "alpha", None, None, Some("source"), 1, 20);

        assert!(url_zh.contains("language_mode=zh"));
        assert!(url_src.contains("language_mode=source"));
        assert_ne!(url_zh, url_src);
    }
}

fn sanitize_filename(raw: &str) -> String {
    let mut s = raw.replace('\\', "_").replace('/', "_").replace('"', "_");
    if s.trim().is_empty() {
        s = "download".to_string();
    }
    s
}

fn build_upload_multipart(boundary: &str, filename: &str, bytes: &[u8]) -> Vec<u8> {
    // Build a simple multipart/form-data payload:
    // field name must be `file` to match backend.
    let filename = sanitize_upload_filename(filename);

    let mut out = Vec::with_capacity(bytes.len().saturating_add(512));
    out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    out.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    out.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    out.extend_from_slice(bytes);
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    out
}

fn sanitize_upload_filename(raw: &str) -> String {
    // Prevent header injection in Content-Disposition.
    let mut s = raw
        .replace('\r', "_")
        .replace('\n', "_")
        .replace('\\', "_")
        .replace('"', "_");
    if s.trim().is_empty() {
        s = "upload.bin".to_string();
    }
    s
}

#[cfg(target_arch = "wasm32")]
fn trigger_browser_download(
    filename: &str,
    content_type: Option<&str>,
    bytes: &[u8],
) -> Result<(), String> {
    use js_sys::{Array, Uint8Array};
    use wasm_bindgen::JsCast;
    use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

    let array = Uint8Array::from(bytes);
    let parts = Array::new();
    parts.push(&array.buffer());

    let bag = BlobPropertyBag::new();
    if let Some(ct) = content_type {
        bag.set_type(ct);
    }
    let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &bag)
        .map_err(|_| "failed to create blob".to_string())?;

    let url = Url::create_object_url_with_blob(&blob)
        .map_err(|_| "failed to create object URL".to_string())?;

    let document = web_sys::window()
        .ok_or_else(|| "no window".to_string())?
        .document()
        .ok_or_else(|| "no document".to_string())?;

    let a: HtmlAnchorElement = document
        .create_element("a")
        .map_err(|_| "failed to create <a>".to_string())?
        .dyn_into::<HtmlAnchorElement>()
        .map_err(|_| "failed to cast <a>".to_string())?;

    a.set_href(&url);
    a.set_download(filename);
    // Keep the download anchor invisible without requiring extra web-sys features.
    a.set_attribute("style", "display:none;").ok();

    document
        .body()
        .ok_or_else(|| "no body".to_string())?
        .append_child(&a)
        .ok();
    a.click();
    a.remove();

    Url::revoke_object_url(&url).map_err(|_| "failed to revoke object URL".to_string())?;
    Ok(())
}
