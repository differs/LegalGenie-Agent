//! Agent tool registry (P2): every tool the runtime can execute.
//!
//! Tools declare a `danger_level`; anything above `read` requires human
//! approval before execution (server-enforced, persisted in agent_tool_calls).
use crate::state::AppState;
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DangerLevel {
    Read,
    Write,
    Sensitive,
    Destructive,
}

impl DangerLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            DangerLevel::Read => "read",
            DangerLevel::Write => "write",
            DangerLevel::Sensitive => "sensitive",
            DangerLevel::Destructive => "destructive",
        }
    }

    pub fn requires_approval(&self) -> bool {
        !matches!(self, DangerLevel::Read)
    }
}

pub struct ToolContext<'a> {
    pub state: &'a AppState,
    pub user_id: &'a str,
    pub username: &'a str,
    pub case_id: Option<&'a str>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn danger_level(&self) -> DangerLevel;
    /// Minimal JSON schema for input validation / documentation.
    fn input_schema(&self) -> Value;
    async fn execute(&self, ctx: &ToolContext<'_>, input: Value) -> anyhow::Result<Value>;
}

// ---------- Helpers ----------

fn normalize_uuid(raw: &str, what: &str) -> anyhow::Result<String> {
    Uuid::parse_str(raw.trim())
        .map(|u| u.to_string())
        .map_err(|_| anyhow::anyhow!("invalid {what}: {raw}"))
}

fn require_case(ctx: &ToolContext<'_>) -> anyhow::Result<String> {
    ctx.case_id
        .map(|c| c.to_string())
        .ok_or_else(|| anyhow::anyhow!("case_id required"))
}

async fn ensure_case_access(state: &AppState, user_id: &str, case_id: &str) -> anyhow::Result<()> {
    crate::access::ensure_case_access(&state.pool, uuid_from(user_id)?, case_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))
}

fn uuid_from(raw: &str) -> anyhow::Result<Uuid> {
    Uuid::parse_str(raw).map_err(|_| anyhow::anyhow!("invalid user id"))
}

// ---------- Read tools ----------

struct ListNodesTool;
struct ListFilesTool;
struct ListPersonsTool;
struct DedupePersonsTool;
struct SearchTool;

// ---------- Write tools ----------

struct CreateNodeTool;
struct MoveNodeTool;
struct DeleteNodeTool;
struct ParseFileTool;
struct CreatePersonTool;
struct MergePersonsTool;
struct ExportEvidenceListTool;
struct ExportTimelineReportTool;

pub fn all_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(ListNodesTool),
        Box::new(ListFilesTool),
        Box::new(ListPersonsTool),
        Box::new(DedupePersonsTool),
        Box::new(SearchTool),
        Box::new(CreateNodeTool),
        Box::new(MoveNodeTool),
        Box::new(DeleteNodeTool),
        Box::new(ParseFileTool),
        Box::new(CreatePersonTool),
        Box::new(MergePersonsTool),
        Box::new(ExportEvidenceListTool),
        Box::new(ExportTimelineReportTool),
    ]
}

pub fn find_tool(name: &str) -> Option<Box<dyn Tool>> {
    all_tools().into_iter().find(|t| t.name() == name)
}


#[async_trait]
impl Tool for ListNodesTool {
    fn name(&self) -> &'static str {
        "list_nodes"
    }
    fn description(&self) -> &'static str {
        "列出当前案件的时间轴节点"
    }
    fn danger_level(&self) -> DangerLevel {
        DangerLevel::Read
    }
    fn input_schema(&self) -> Value {
        json!({ "case_id": { "type": "string" } })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, _input: Value) -> anyhow::Result<Value> {
        let case_id = require_case(ctx)?;
        ensure_case_access(ctx.state, ctx.user_id, &case_id).await?;
        let rows: Vec<Value> = sqlx::query_as::<
            _,
            (String, String, Option<String>, String, i64, String),
        >(
            r#"
            SELECT id, title, description, event_time, sort_order, created_at
            FROM event_nodes
            WHERE case_id = $1 AND status != 'deleted'
            ORDER BY event_time ASC, sort_order ASC
            LIMIT 200
            "#,
        )
        .bind(&case_id)
        .fetch_all(&ctx.state.pool)
        .await?
        .into_iter()
        .map(
            |(id, title, description, event_time, sort_order, created_at)| {
                json!({
                    "id": id, "title": title, "description": description,
                    "event_time": event_time, "sort_order": sort_order, "created_at": created_at,
                })
            },
        )
        .collect();
        Ok(json!({ "nodes": rows, "total": rows.len() }))
    }
}

#[async_trait]
impl Tool for ListFilesTool {
    fn name(&self) -> &'static str {
        "list_files"
    }
    fn description(&self) -> &'static str {
        "列出当前案件的证据文件与解析/翻译状态"
    }
    fn danger_level(&self) -> DangerLevel {
        DangerLevel::Read
    }
    fn input_schema(&self) -> Value {
        json!({ "case_id": { "type": "string" } })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, _input: Value) -> anyhow::Result<Value> {
        let case_id = require_case(ctx)?;
        ensure_case_access(ctx.state, ctx.user_id, &case_id).await?;
        let rows: Vec<Value> = sqlx::query_as::<_, (String, String, String, i64, String, String)>(
            r#"
            SELECT id, original_name, file_type, file_size, parse_status, translation_status
            FROM evidence_files
            WHERE case_id = $1 AND status != 'deleted'
            ORDER BY created_at DESC
            LIMIT 100
            "#,
        )
        .bind(&case_id)
        .fetch_all(&ctx.state.pool)
        .await?
        .into_iter()
        .map(|(id, name, ftype, size, parse, trans)| {
            json!({ "id": id, "original_name": name, "file_type": ftype, "file_size": size, "parse_status": parse, "translation_status": trans })
        })
        .collect();
        Ok(json!({ "files": rows, "total": rows.len() }))
    }
}

#[async_trait]
impl Tool for ListPersonsTool {
    fn name(&self) -> &'static str {
        "list_persons"
    }
    fn description(&self) -> &'static str {
        "列出当前案件的人物"
    }
    fn danger_level(&self) -> DangerLevel {
        DangerLevel::Read
    }
    fn input_schema(&self) -> Value {
        json!({ "case_id": { "type": "string" } })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, _input: Value) -> anyhow::Result<Value> {
        let case_id = require_case(ctx)?;
        ensure_case_access(ctx.state, ctx.user_id, &case_id).await?;
        let rows: Vec<Value> = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>)>(
            r#"
            SELECT p.id, p.name, l.role_type, p.phone, p.organization
            FROM persons p
            JOIN person_case_links l ON l.person_id = p.id
            WHERE l.case_id = $1 AND p.status != 'deleted'
            ORDER BY p.name ASC
            LIMIT 200
            "#,
        )
        .bind(&case_id)
        .fetch_all(&ctx.state.pool)
        .await?
        .into_iter()
        .map(|(id, name, role, phone, org)| {
            json!({ "id": id, "name": name, "role_type": role, "phone": phone, "organization": org })
        })
        .collect();
        Ok(json!({ "persons": rows, "total": rows.len() }))
    }
}

#[async_trait]
impl Tool for DedupePersonsTool {
    fn name(&self) -> &'static str {
        "dedupe_persons"
    }
    fn description(&self) -> &'static str {
        "给出当前案件的人物去重建议"
    }
    fn danger_level(&self) -> DangerLevel {
        DangerLevel::Read
    }
    fn input_schema(&self) -> Value {
        json!({ "case_id": { "type": "string" } })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, _input: Value) -> anyhow::Result<Value> {
        let case_id = require_case(ctx)?;
        ensure_case_access(ctx.state, ctx.user_id, &case_id).await?;
        // Simple grouping by normalized name (case-local).
        let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT p.id, p.name, p.phone
            FROM persons p
            JOIN person_case_links l ON l.person_id = p.id
            WHERE l.case_id = $1 AND p.status != 'deleted'
            ORDER BY p.name
            "#,
        )
        .bind(&case_id)
        .fetch_all(&ctx.state.pool)
        .await?;

        let mut groups: Vec<Value> = Vec::new();
        let mut index: std::collections::HashMap<String, Vec<Value>> = Default::default();
        for (id, name, phone) in rows {
            let key = name.trim().to_lowercase();
            index
                .entry(key)
                .or_default()
                .push(json!({ "id": id, "name": name, "phone": phone }));
        }
        for (_key, persons) in index {
            if persons.len() > 1 {
                groups.push(json!({ "reason": "同名分组", "persons": persons }));
            }
        }
        Ok(json!({ "groups": groups }))
    }
}

#[async_trait]
impl Tool for SearchTool {
    fn name(&self) -> &'static str {
        "search"
    }
    fn description(&self) -> &'static str {
        "在案件材料中搜索（节点/证据/人物）"
    }
    fn danger_level(&self) -> DangerLevel {
        DangerLevel::Read
    }
    fn input_schema(&self) -> Value {
        json!({ "keyword": { "type": "string" }, "case_id": { "type": "string" } })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, input: Value) -> anyhow::Result<Value> {
        let keyword = input
            .get("keyword")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if keyword.is_empty() {
            anyhow::bail!("keyword required");
        }
        let case_id = require_case(ctx)?;
        ensure_case_access(ctx.state, ctx.user_id, &case_id).await?;

        let pattern = format!("%{keyword}%");
        let nodes: Vec<Value> = sqlx::query_as::<_, (String, String, String, String)>(
            r#"
            SELECT id, title, event_time, 'node' AS kind
            FROM event_nodes
            WHERE case_id = $1 AND status != 'deleted' AND (title ILIKE $2 OR COALESCE(description,'') ILIKE $2)
            LIMIT 10
            "#,
        )
        .bind(&case_id)
        .bind(&pattern)
        .fetch_all(&ctx.state.pool)
        .await?
        .into_iter()
        .map(|(id, title, time, kind)| json!({ "object_type": kind, "object_id": id, "title": title, "extra": time }))
        .collect();

        let files: Vec<Value> = sqlx::query_as::<_, (String, String, String)>(
            r#"
            SELECT id, original_name, 'evidence' AS kind
            FROM evidence_files
            WHERE case_id = $1 AND status != 'deleted' AND original_name ILIKE $2
            LIMIT 10
            "#,
        )
        .bind(&case_id)
        .bind(&pattern)
        .fetch_all(&ctx.state.pool)
        .await?
        .into_iter()
        .map(|(id, title, kind)| json!({ "object_type": kind, "object_id": id, "title": title }))
        .collect();

        let persons: Vec<Value> = sqlx::query_as::<_, (String, String, String)>(
            r#"
            SELECT p.id, p.name, 'person' AS kind
            FROM persons p
            JOIN person_case_links l ON l.person_id = p.id
            WHERE l.case_id = $1 AND p.status != 'deleted' AND p.name ILIKE $2
            LIMIT 10
            "#,
        )
        .bind(&case_id)
        .bind(&pattern)
        .fetch_all(&ctx.state.pool)
        .await?
        .into_iter()
        .map(|(id, title, kind)| json!({ "object_type": kind, "object_id": id, "title": title }))
        .collect();

        let mut results = nodes;
        results.extend(files);
        results.extend(persons);
        Ok(json!({ "results": results, "total": results.len() }))
    }
}

// ---------- Write tools ----------

#[async_trait]
impl Tool for CreateNodeTool {
    fn name(&self) -> &'static str {
        "create_node"
    }
    fn description(&self) -> &'static str {
        "在当前案件创建时间轴节点"
    }
    fn danger_level(&self) -> DangerLevel {
        DangerLevel::Write
    }
    fn input_schema(&self) -> Value {
        json!({ "case_id": { "type": "string" }, "title": { "type": "string" }, "event_time": { "type": "string" }, "description": { "type": "string" } })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, input: Value) -> anyhow::Result<Value> {
        let case_id = require_case(ctx)?;
        ensure_case_access(ctx.state, ctx.user_id, &case_id).await?;
        crate::access::ensure_case_write_access(&ctx.state.pool, uuid_from(ctx.user_id)?, &case_id)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let title = input
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let event_time = input
            .get("event_time")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if title.is_empty() || event_time.is_empty() {
            anyhow::bail!("title and event_time required");
        }
        let description = input.get("description").and_then(|v| v.as_str());

        let node_id = Uuid::new_v4().to_string();
        let next_sort: (i64,) = sqlx::query_as(
            "SELECT COALESCE(MAX(sort_order), 0)::bigint + 1 FROM event_nodes WHERE case_id = $1 AND event_time = $2 AND status != 'deleted'",
        )
        .bind(&case_id)
        .bind(&event_time)
        .fetch_one(&ctx.state.pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO event_nodes (id, case_id, title, description, event_time, sort_order, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&node_id)
        .bind(&case_id)
        .bind(&title)
        .bind(description)
        .bind(&event_time)
        .bind(next_sort.0)
        .bind(ctx.user_id)
        .execute(&ctx.state.pool)
        .await?;

        Ok(json!({ "id": node_id, "title": title, "event_time": event_time }))
    }
}

#[async_trait]
impl Tool for MoveNodeTool {
    fn name(&self) -> &'static str {
        "move_node"
    }
    fn description(&self) -> &'static str {
        "移动时间轴节点到新日期"
    }
    fn danger_level(&self) -> DangerLevel {
        DangerLevel::Write
    }
    fn input_schema(&self) -> Value {
        json!({ "node_id": { "type": "string" }, "event_time": { "type": "string" } })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, input: Value) -> anyhow::Result<Value> {
        let node_id = input
            .get("node_id")
            .and_then(|v| v.as_str())
            .map(|s| normalize_uuid(s, "node_id"))
            .transpose()?
            .ok_or_else(|| anyhow::anyhow!("node_id required"))?;
        let event_time = input
            .get("event_time")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("event_time required"))?
            .to_string();

        let row: Option<(String,)> =
            sqlx::query_as("SELECT case_id FROM event_nodes WHERE id = $1 AND status != 'deleted'")
                .bind(&node_id)
                .fetch_optional(&ctx.state.pool)
                .await?;
        let Some((case_id,)) = row else {
            anyhow::bail!("node not found");
        };
        crate::access::ensure_case_write_access(&ctx.state.pool, uuid_from(ctx.user_id)?, &case_id)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        sqlx::query(
            "UPDATE event_nodes SET event_time = $1, updated_at = utc_text() WHERE id = $2 AND status != 'deleted'",
        )
        .bind(&event_time)
        .bind(&node_id)
        .execute(&ctx.state.pool)
        .await?;
        Ok(json!({ "id": node_id, "event_time": event_time }))
    }
}

#[async_trait]
impl Tool for DeleteNodeTool {
    fn name(&self) -> &'static str {
        "delete_node"
    }
    fn description(&self) -> &'static str {
        "删除时间轴节点（软删除）"
    }
    fn danger_level(&self) -> DangerLevel {
        DangerLevel::Destructive
    }
    fn input_schema(&self) -> Value {
        json!({ "node_id": { "type": "string" } })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, input: Value) -> anyhow::Result<Value> {
        let node_id = input
            .get("node_id")
            .and_then(|v| v.as_str())
            .map(|s| normalize_uuid(s, "node_id"))
            .transpose()?
            .ok_or_else(|| anyhow::anyhow!("node_id required"))?;
        let row: Option<(String,)> =
            sqlx::query_as("SELECT case_id FROM event_nodes WHERE id = $1 AND status != 'deleted'")
                .bind(&node_id)
                .fetch_optional(&ctx.state.pool)
                .await?;
        let Some((case_id,)) = row else {
            anyhow::bail!("node not found");
        };
        crate::access::ensure_case_write_access(&ctx.state.pool, uuid_from(ctx.user_id)?, &case_id)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        sqlx::query(
            "UPDATE event_nodes SET status = 'deleted', updated_at = utc_text() WHERE id = $1 AND status != 'deleted'",
        )
        .bind(&node_id)
        .execute(&ctx.state.pool)
        .await?;
        Ok(json!({ "id": node_id, "deleted": true }))
    }
}

#[async_trait]
impl Tool for ParseFileTool {
    fn name(&self) -> &'static str {
        "parse_file"
    }
    fn description(&self) -> &'static str {
        "触发证据文件解析"
    }
    fn danger_level(&self) -> DangerLevel {
        DangerLevel::Write
    }
    fn input_schema(&self) -> Value {
        json!({ "file_id": { "type": "string" } })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, input: Value) -> anyhow::Result<Value> {
        let file_id = input
            .get("file_id")
            .and_then(|v| v.as_str())
            .map(|s| normalize_uuid(s, "file_id"))
            .transpose()?
            .ok_or_else(|| anyhow::anyhow!("file_id required"))?;
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT case_id FROM evidence_files WHERE id = $1 AND status != 'deleted'",
        )
        .bind(&file_id)
        .fetch_optional(&ctx.state.pool)
        .await?;
        let Some((case_id,)) = row else {
            anyhow::bail!("file not found");
        };
        crate::access::ensure_case_write_access(&ctx.state.pool, uuid_from(ctx.user_id)?, &case_id)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        crate::parser::enqueue_parse(ctx.state.clone(), file_id.clone(), true).await?;
        Ok(json!({ "id": file_id, "parse_status": "processing" }))
    }
}

#[async_trait]
impl Tool for CreatePersonTool {
    fn name(&self) -> &'static str {
        "create_person"
    }
    fn description(&self) -> &'static str {
        "在当前案件创建人物"
    }
    fn danger_level(&self) -> DangerLevel {
        DangerLevel::Write
    }
    fn input_schema(&self) -> Value {
        json!({ "case_id": { "type": "string" }, "name": { "type": "string" }, "role_type": { "type": "string" } })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, input: Value) -> anyhow::Result<Value> {
        let case_id = require_case(ctx)?;
        ensure_case_access(ctx.state, ctx.user_id, &case_id).await?;
        crate::access::ensure_case_write_access(&ctx.state.pool, uuid_from(ctx.user_id)?, &case_id)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() {
            anyhow::bail!("name required");
        }
        let role_type = input
            .get("role_type")
            .and_then(|v| v.as_str())
            .unwrap_or("当事人")
            .to_string();

        let person_id = Uuid::new_v4().to_string();
        let link_id = Uuid::new_v4().to_string();
        let mut tx = ctx.state.pool.begin().await?;
        sqlx::query(
            "INSERT INTO persons (id, name, status, created_by) VALUES ($1, $2, 'active', $3)",
        )
        .bind(&person_id)
        .bind(&name)
        .bind(ctx.user_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO person_case_links (id, person_id, case_id, role_type) VALUES ($1, $2, $3, $4)",
        )
        .bind(&link_id)
        .bind(&person_id)
        .bind(&case_id)
        .bind(&role_type)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(json!({ "id": person_id, "name": name, "role_type": role_type }))
    }
}

#[async_trait]
impl Tool for MergePersonsTool {
    fn name(&self) -> &'static str {
        "merge_persons"
    }
    fn description(&self) -> &'static str {
        "在当前案件内合并两个人物（保留 target）"
    }
    fn danger_level(&self) -> DangerLevel {
        DangerLevel::Sensitive
    }
    fn input_schema(&self) -> Value {
        json!({ "case_id": { "type": "string" }, "source_person_id": { "type": "string" }, "target_person_id": { "type": "string" } })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, input: Value) -> anyhow::Result<Value> {
        let case_id = require_case(ctx)?;
        ensure_case_access(ctx.state, ctx.user_id, &case_id).await?;
        crate::access::ensure_case_write_access(&ctx.state.pool, uuid_from(ctx.user_id)?, &case_id)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let source = input
            .get("source_person_id")
            .and_then(|v| v.as_str())
            .map(|s| normalize_uuid(s, "source_person_id"))
            .transpose()?
            .ok_or_else(|| anyhow::anyhow!("source_person_id required"))?;
        let target = input
            .get("target_person_id")
            .and_then(|v| v.as_str())
            .map(|s| normalize_uuid(s, "target_person_id"))
            .transpose()?
            .ok_or_else(|| anyhow::anyhow!("target_person_id required"))?;

        let mut tx = ctx.state.pool.begin().await?;
        // Re-point case-local links (unique (person_id, case_id, role_type) guards duplicates).
        sqlx::query(
            "UPDATE person_case_links SET person_id = $1 WHERE case_id = $2 AND person_id = $3",
        )
        .bind(&target)
        .bind(&case_id)
        .bind(&source)
        .execute(&mut *tx)
        .await?;
        // Re-point relationships.
        sqlx::query(
            "UPDATE person_relationships SET from_person_id = $1 WHERE case_id = $2 AND from_person_id = $3",
        )
        .bind(&target)
        .bind(&case_id)
        .bind(&source)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE person_relationships SET to_person_id = $1 WHERE case_id = $2 AND to_person_id = $3",
        )
        .bind(&target)
        .bind(&case_id)
        .bind(&source)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "DELETE FROM person_relationships WHERE case_id = $1 AND (from_person_id = $2 OR to_person_id = $2)",
        )
        .bind(&case_id)
        .bind(&source)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE persons SET status = 'deleted' WHERE id = $1")
            .bind(&source)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(json!({ "source_person_id": source, "target_person_id": target }))
    }
}

#[async_trait]
impl Tool for ExportEvidenceListTool {
    fn name(&self) -> &'static str {
        "export_evidence_list"
    }
    fn description(&self) -> &'static str {
        "生成案件证据清单（xlsx）"
    }
    fn danger_level(&self) -> DangerLevel {
        DangerLevel::Write
    }
    fn input_schema(&self) -> Value {
        json!({ "case_id": { "type": "string" } })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, _input: Value) -> anyhow::Result<Value> {
        let case_id = require_case(ctx)?;
        ensure_case_access(ctx.state, ctx.user_id, &case_id).await?;
        crate::access::ensure_case_write_access(&ctx.state.pool, uuid_from(ctx.user_id)?, &case_id)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let exported = crate::routes::case_exports::agent_export_evidence_list(
            ctx.state,
            &case_id,
            ctx.user_id,
        )
        .await
        .map_err(|e| anyhow::anyhow!("export failed: {e}"))?;
        Ok(json!({ "export_id": exported.0, "file_name": exported.1 }))
    }
}

#[async_trait]
impl Tool for ExportTimelineReportTool {
    fn name(&self) -> &'static str {
        "export_timeline_report"
    }
    fn description(&self) -> &'static str {
        "生成时间轴报告（pdf/html）"
    }
    fn danger_level(&self) -> DangerLevel {
        DangerLevel::Write
    }
    fn input_schema(&self) -> Value {
        json!({ "case_id": { "type": "string" }, "format": { "type": "string" } })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, input: Value) -> anyhow::Result<Value> {
        let case_id = require_case(ctx)?;
        ensure_case_access(ctx.state, ctx.user_id, &case_id).await?;
        crate::access::ensure_case_write_access(&ctx.state.pool, uuid_from(ctx.user_id)?, &case_id)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let format = input
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("pdf")
            .to_string();
        anyhow::bail!(
            "时间轴报告生成暂不支持在对话中直接调用，请使用导出视图（export_timeline_report format={format}）"
        );
    }
}
