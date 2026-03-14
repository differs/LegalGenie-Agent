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

fn sanitize_filename(raw: &str) -> String {
    let mut s = raw.replace('\\', "_").replace('/', "_").replace('"', "_");
    if s.trim().is_empty() {
        s = "download".to_string();
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
    use wasm_bindgen::JsValue;
    use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

    let array = Uint8Array::from(bytes);
    let parts = Array::new();
    parts.push(&array.buffer());

    let mut bag = BlobPropertyBag::new();
    if let Some(ct) = content_type {
        bag.type_(ct);
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
    a.style().set_property("display", "none").ok();

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
