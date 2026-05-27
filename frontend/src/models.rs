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
pub struct EvidenceFileSummary {
    pub id: String,
    pub original_name: String,
    pub file_type: String,
    pub file_size: i64,
    pub storage_path: String,
    pub parse_status: String,
    pub parse_error: Option<String>,
    #[serde(default, flatten)]
    pub translation: EvidenceFileTranslationSummary,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct EvidenceFileListData {
    pub files: Vec<EvidenceFileSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct EvidenceFileDetail {
    pub id: String,
    pub case_id: String,
    pub original_name: String,
    pub file_type: String,
    pub file_size: i64,
    pub storage_path: String,
    pub parse_status: String,
    pub parse_error: Option<String>,
    pub parsed_text: Option<String>,
    pub page_count: Option<i64>,
    pub duration: Option<i64>,
    #[serde(default, flatten)]
    pub translation: EvidenceFileTranslationSummary,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct EvidenceFileTranslationSummary {
    #[serde(default = "default_translation_status")]
    pub translation_status: String,
    pub translation_error: Option<String>,
    pub source_language: Option<String>,
    pub target_language: Option<String>,
    #[serde(default)]
    pub chunk_count: i64,
    #[serde(default)]
    pub translated_chunk_count: i64,
    #[serde(default)]
    pub failed_chunk_count: i64,
    pub translation_provider: Option<String>,
    pub translation_model: Option<String>,
    #[serde(default)]
    pub translation_incomplete: bool,
}

impl Default for EvidenceFileTranslationSummary {
    fn default() -> Self {
        Self {
            translation_status: default_translation_status(),
            translation_error: None,
            source_language: None,
            target_language: None,
            chunk_count: 0,
            translated_chunk_count: 0,
            failed_chunk_count: 0,
            translation_provider: None,
            translation_model: None,
            translation_incomplete: false,
        }
    }
}

fn default_translation_status() -> String {
    "pending".to_string()
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct EvidenceFileTranslationDetail {
    pub id: String,
    #[serde(flatten)]
    pub translation: EvidenceFileTranslationSummary,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct EvidenceFileChunkListData {
    pub items: Vec<EvidenceFileChunkItem>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub view_mode: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct EvidenceFileChunkItem {
    pub id: String,
    pub chunk_index: i64,
    pub page_number: i64,
    pub segment_number: i64,
    pub chunk_kind: String,
    pub display_label: String,
    pub source_text: Option<String>,
    pub translated_text: Option<String>,
    pub translation_status: String,
    pub translation_error: Option<String>,
    pub anchor_json: serde_json::Value,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct RetryTranslationResult {
    pub file_id: String,
    pub scope: String,
    pub retried_count: i64,
    pub skipped_count: i64,
    pub retried_chunk_ids: Vec<String>,
    pub skipped_chunk_ids: Vec<String>,
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

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct CaseSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub created_at: String,
    pub member_count: i64,
    pub evidence_count: i64,
    pub node_count: i64,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct CaseListData {
    pub cases: Vec<CaseSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct CaseDetail {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub tags: Option<Vec<String>>,
    pub owner_id: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct CaseMemberMeData {
    pub case_id: String,
    pub user_id: String,
    pub username: String,
    pub role_in_case: String,
}

// Timeline

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct TimelineEvidenceLink {
    pub id: String,
    pub evidence_id: String,
    pub evidence_name: String,
    pub anchor_type: String,
    pub anchor_data: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct TimelineNode {
    pub id: String,
    pub case_id: String,
    pub title: String,
    pub description: Option<String>,
    pub event_time: String,
    pub sort_order: i64,
    pub tags: Vec<String>,
    pub evidence_links: Vec<TimelineEvidenceLink>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct TimelineNodeListData {
    pub nodes: Vec<TimelineNode>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

// Persons

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct CasePersonItem {
    pub id: String,
    pub name: String,
    pub gender: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub organization: Option<String>,
    pub position: Option<String>,
    pub role_type: String,
    pub role_detail: Option<String>,
    pub involved_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct CasePersonListData {
    pub persons: Vec<CasePersonItem>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct PersonCaseLink {
    pub case_id: String,
    pub role_type: String,
    pub role_detail: Option<String>,
    pub involved_date: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct PersonDetail {
    pub id: String,
    pub name: String,
    pub gender: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub organization: Option<String>,
    pub position: Option<String>,
    pub notes: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub cases: Vec<PersonCaseLink>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct PersonGraphNode {
    pub id: String,
    pub name: String,
    pub role_type: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct PersonGraphData {
    pub nodes: Vec<PersonGraphNode>,
    pub edges: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct DedupeCandidatePerson {
    pub id: String,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub organization: Option<String>,
    pub position: Option<String>,
    pub roles: Vec<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct DedupeCandidateGroup {
    pub key: String,
    pub reason: String,
    pub persons: Vec<DedupeCandidatePerson>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct DedupeCandidatesData {
    pub groups: Vec<DedupeCandidateGroup>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct MergeCasePersonsData {
    pub source_person_id: String,
    pub target_person_id: String,
    pub moved_links: i64,
    pub updated_links: i64,
    pub moved_relationships: i64,
    pub dropped_relationships: i64,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct RelationshipItem {
    pub id: String,
    pub case_id: String,
    pub from_person_id: String,
    pub from_person_name: String,
    pub to_person_id: String,
    pub to_person_name: String,
    pub rel_type: String,
    pub rel_detail: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct RelationshipListData {
    pub relationships: Vec<RelationshipItem>,
    pub total: i64,
}

// Search

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct SearchResult {
    pub object_type: String,
    pub object_id: String,
    pub case_id: Option<String>,
    pub case_name: Option<String>,
    pub title: String,
    pub content: Option<String>,
    pub highlight: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub score: f64,
    pub language_mode: Option<String>,
    pub file_id: Option<String>,
    pub file_name: Option<String>,
    pub chunk_id: Option<String>,
    pub chunk_index: Option<i64>,
    pub display_label: Option<String>,
    pub anchor_json: Option<serde_json::Value>,
    pub matched_language: Option<String>,
    pub snippet_source: Option<String>,
    pub snippet_translated: Option<String>,
    pub match_start_offset: Option<i64>,
    pub match_end_offset: Option<i64>,
    pub translation_incomplete: Option<bool>,
    pub source_fallback: Option<bool>,
    #[serde(default)]
    pub bilingual: Option<SearchBilingualSnippet>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct SearchBilingualSnippet {
    pub snippet_source: Option<String>,
    pub snippet_translated: Option<String>,
    pub matched_language: Option<String>,
    pub source_fallback: Option<bool>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct SearchResponseData {
    pub results: Vec<SearchResult>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct SuggestionsData {
    pub suggestions: Vec<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct SearchHistoryItem {
    pub id: String,
    pub keyword: String,
    pub object_types: Option<String>,
    pub case_id: Option<String>,
    pub result_count: Option<i64>,
    pub searched_at: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct SearchHistoryData {
    pub items: Vec<SearchHistoryItem>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_file_summary_accepts_flattened_translation_fields() {
        let value = serde_json::json!({
            "id": "file-1",
            "original_name": "legacy.pdf",
            "file_type": "application/pdf",
            "file_size": 123,
            "storage_path": "cases/1/legacy.pdf",
            "parse_status": "done",
            "parse_error": null,
            "translation_status": "processing",
            "translation_error": null,
            "source_language": "en",
            "target_language": "zh-CN",
            "chunk_count": 12,
            "translated_chunk_count": 5,
            "failed_chunk_count": 1,
            "translation_provider": "openai",
            "translation_model": "gpt-test",
            "translation_incomplete": true,
            "created_at": "2026-03-30T10:00:00Z"
        });

        let summary: EvidenceFileSummary =
            serde_json::from_value(value).expect("flattened translation summary");

        assert_eq!(summary.translation.translation_status, "processing");
        assert_eq!(summary.translation.chunk_count, 12);
        assert!(summary.translation.translation_incomplete);
    }
}
