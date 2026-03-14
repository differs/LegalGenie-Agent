use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Shared core domain types intended for reuse across backend and frontend.
///
/// Note: IDs are `Uuid` to keep them strongly typed. If the API chooses
/// string IDs later, we can switch to `String` without affecting internal
/// storage strategies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventNode {
    pub id: Uuid,
    pub case_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub event_time: NaiveDate,
    pub sort_order: i32,
    pub evidence_links: Vec<EvidenceLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceLink {
    pub id: Uuid,
    pub evidence_id: Uuid,
    pub anchor_type: String,
    pub anchor_data: serde_json::Value,
}
