use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ApiEnvelope<T> {
    pub code: u16,
    pub message: String,
    pub data: Option<T>,
    #[allow(dead_code)]
    pub timestamp: i64,
    pub error_code: Option<i32>,
}

impl<T> ApiEnvelope<T> {
    pub fn into_data(self) -> Result<T, String> {
        if self.code >= 200 && self.code < 300 {
            self.data.ok_or_else(|| "missing data".to_string())
        } else {
            Err(format!(
                "{} (http_code={}, error_code={:?})",
                self.message, self.code, self.error_code
            ))
        }
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub email: String,
    pub real_name: Option<String>,
    pub roles: Vec<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct LoginResponseData {
    pub user: UserInfo,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ExportRecordItem {
    pub id: String,
    pub case_id: String,
    pub export_type: String,
    pub file_name: String,
    pub storage_path: String,
    pub file_size: Option<i64>,
    pub generated_by: String,
    pub generated_at: String,
    pub node_ids: Option<Vec<String>>,
    pub evidence_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ExportHistoryData {
    pub records: Vec<ExportRecordItem>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct OperationLogItem {
    pub id: String,
    pub user_id: String,
    pub user_name: String,
    pub case_id: Option<String>,
    pub action: String,
    pub module: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub target_title: Option<String>,
    pub changed_fields: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct LogsListData {
    pub logs: Vec<OperationLogItem>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct HistoryItem {
    pub action: String,
    pub user_name: String,
    pub created_at: String,
    pub changes: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct TargetHistoryData {
    pub target_type: String,
    pub target_id: String,
    pub history: Vec<HistoryItem>,
}
