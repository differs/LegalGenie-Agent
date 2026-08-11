//! Agent engine (P2): intent parsing + unified tool execution pipeline.
//!
//! The rule engine translates natural-language intents into tool calls.
//! Read tools execute immediately; anything requiring approval is persisted
//! as a pending `agent_tool_calls` row until the user confirms via the
//! approval endpoints. A configured LLM can replace intent parsing later
//! without touching the tool pipeline.
use crate::agent::tools::{find_tool, ToolContext};
use crate::state::AppState;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum Intent {
    ListNodes,
    ListFiles,
    ListPersons,
    DedupePersons,
    Search {
        keyword: String,
    },
    CreateNode {
        title: Option<String>,
        event_time: Option<String>,
        description: Option<String>,
    },
    MoveNode,
    DeleteNode,
    ParseFile,
    CreatePerson,
    MergePersons,
    ExportEvidenceList,
    ExportTimelineReport {
        format: String,
    },
    Help,
    Unknown {
        text: String,
    },
}

pub fn parse_intent(text: &str) -> Intent {
    let t = text.trim().to_lowercase();
    let has = |re: &str| regex_like(&t, re);

    if t.starts_with('/') {
        let cmd = t
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches('/');
        let rest = t.splitn(2, ' ').nth(1).unwrap_or("").trim().to_string();
        return match cmd {
            "cases" | "nodes" | "timeline" => Intent::ListNodes,
            "files" | "evidence" => Intent::ListFiles,
            "persons" | "people" => Intent::ListPersons,
            "dedupe" => Intent::DedupePersons,
            "search" => Intent::Search { keyword: rest },
            "export" => {
                if rest.contains("pdf") || rest.contains("report") {
                    Intent::ExportTimelineReport {
                        format: "pdf".to_string(),
                    }
                } else {
                    Intent::ExportEvidenceList
                }
            }
            "help" => Intent::Help,
            _ => Intent::Unknown {
                text: text.to_string(),
            },
        };
    }

    if has(r"help|帮助|能做什么|怎么用") {
        return Intent::Help;
    }
    if has(r"去重|合并|重复|dedupe") {
        return Intent::DedupePersons;
    }
    if has(r"人物|人员|person|people") {
        return Intent::ListPersons;
    }
    if has(r"导出|export") {
        if has(r"证据清单|evidence-list|xlsx") {
            return Intent::ExportEvidenceList;
        }
        return Intent::ExportTimelineReport {
            format: if has(r"html") {
                "html".to_string()
            } else {
                "pdf".to_string()
            },
        };
    }
    if has(r"证据|文件|上传|材料|file|evidence") {
        return Intent::ListFiles;
    }
    if has(r"时间轴|节点|timeline|node") {
        // 新建节点意图（含标题/日期）
        let title = extract_title(text);
        let date = extract_date(text);
        if has(r"新建|新增|创建|添加|add|create|new") || title.is_some() || date.is_some() {
            return Intent::CreateNode {
                title,
                event_time: date,
                description: None,
            };
        }
        return Intent::ListNodes;
    }
    if has(r"搜索|查找|检索|search|find|搜一下|帮我找") {
        let keyword = t
            .replace("搜索", "")
            .replace("查找", "")
            .replace("检索", "")
            .replace("search", "")
            .replace("find", "")
            .replace("搜一下", "")
            .replace("帮我找", "")
            .trim()
            .to_string();
        return Intent::Search { keyword };
    }

    Intent::Unknown {
        text: text.to_string(),
    }
}

fn regex_like(text: &str, pattern: &str) -> bool {
    // Cheap substring/regex-lite matching: any of the | separated tokens.
    pattern.split('|').any(|token| text.contains(token.trim()))
}

fn extract_title(text: &str) -> Option<String> {
    for (open, close) in [("「", "」"), ("“", "”"), ("\"", "\"")] {
        if let Some(start) = text.find(open) {
            let rest = &text[start + open.len()..];
            if let Some(end) = rest.find(close) {
                let t = rest[..end].trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

fn extract_date(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 8 {
        let y = &digits[0..4];
        let m = &digits[4..6];
        let d = &digits[6..8];
        return Some(format!("{y}-{m}-{d}"));
    }
    if digits.len() >= 4 {
        let m = &digits[0..2];
        let d = &digits[2..4];
        return Some(format!("{}-{m}-{d}", current_year()));
    }
    let _ = chars;
    None
}

fn current_year() -> String {
    chrono::Utc::now().format("%Y").to_string()
}

// ---------- Execution ----------

pub struct AgentRun {
    pub assistant_text: String,
    pub tool_calls: Vec<Value>,
    pub pending_approvals: Vec<Value>,
}

pub async fn process_user_message(
    state: &AppState,
    user_id: &str,
    username: &str,
    case_id: Option<&str>,
    session_id: &str,
    text: &str,
) -> anyhow::Result<AgentRun> {
    // Persist the user message.
    let msg_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO agent_messages (id, session_id, role, content) VALUES ($1, $2, 'user', $3)",
    )
    .bind(&msg_id)
    .bind(session_id)
    .bind(text)
    .execute(&state.pool)
    .await?;

    let intent = parse_intent(text);
    let ctx = ToolContext {
        state,
        user_id,
        username,
        case_id,
    };

    match intent {
        Intent::Help => {
            let content = help_text();
            save_assistant(state, session_id, &content).await?;
            Ok(AgentRun {
                assistant_text: content,
                tool_calls: vec![],
                pending_approvals: vec![],
            })
        }
        Intent::Unknown { text } => {
            let content = format!("我暂时无法理解这条指令（{text}）。试试：列出节点 / 列出证据 / 人物去重 / 搜索 xxx / 导出时间轴报告。");
            save_assistant(state, session_id, &content).await?;
            Ok(AgentRun {
                assistant_text: content,
                tool_calls: vec![],
                pending_approvals: vec![],
            })
        }
        Intent::Search { keyword } => {
            run_tool(&ctx, session_id, "search", json!({ "keyword": keyword })).await
        }
        Intent::ListNodes => run_tool(&ctx, session_id, "list_nodes", json!({})).await,
        Intent::ListFiles => run_tool(&ctx, session_id, "list_files", json!({})).await,
        Intent::ListPersons => run_tool(&ctx, session_id, "list_persons", json!({})).await,
        Intent::DedupePersons => run_tool(&ctx, session_id, "dedupe_persons", json!({})).await,
        Intent::CreateNode {
            title,
            event_time,
            description,
        } => {
            if title.is_none() || event_time.is_none() {
                let content =
                    "创建节点缺少标题或日期。请按此格式补充：新建节点「标题」2024-01-12。";
                save_assistant(state, session_id, content).await?;
                return Ok(AgentRun {
                    assistant_text: content.to_string(),
                    tool_calls: vec![],
                    pending_approvals: vec![],
                });
            }
            run_tool(
                &ctx,
                session_id,
                "create_node",
                json!({ "title": title, "event_time": event_time, "description": description }),
            )
            .await
        }
        Intent::MoveNode => {
            let content = "移动节点需要 node_id 与新日期。在时间轴中选择节点后告诉我，或使用 /timeline 查看。";
            save_assistant(state, session_id, content).await?;
            Ok(AgentRun {
                assistant_text: content.to_string(),
                tool_calls: vec![],
                pending_approvals: vec![],
            })
        }
        Intent::DeleteNode => {
            let content = "删除节点需要 node_id。在时间轴中选择节点后告诉我。";
            save_assistant(state, session_id, content).await?;
            Ok(AgentRun {
                assistant_text: content.to_string(),
                tool_calls: vec![],
                pending_approvals: vec![],
            })
        }
        Intent::ParseFile => {
            let content = "解析证据需要 file_id。先在证据视图选择文件，或让我列出证据文件。";
            save_assistant(state, session_id, content).await?;
            Ok(AgentRun {
                assistant_text: content.to_string(),
                tool_calls: vec![],
                pending_approvals: vec![],
            })
        }
        Intent::CreatePerson => {
            let content = "创建人物需要姓名与角色，例如：新建人物 张三 当事人。";
            save_assistant(state, session_id, content).await?;
            Ok(AgentRun {
                assistant_text: content.to_string(),
                tool_calls: vec![],
                pending_approvals: vec![],
            })
        }
        Intent::MergePersons => {
            let content = "人物合并需要 source 与 target 人物。使用「人物去重」查看建议后告诉我。";
            save_assistant(state, session_id, content).await?;
            Ok(AgentRun {
                assistant_text: content.to_string(),
                tool_calls: vec![],
                pending_approvals: vec![],
            })
        }
        Intent::ExportEvidenceList => {
            run_tool(&ctx, session_id, "export_evidence_list", json!({})).await
        }
        Intent::ExportTimelineReport { format } => {
            run_tool(
                &ctx,
                session_id,
                "export_timeline_report",
                json!({ "format": format }),
            )
            .await
        }
    }
}

/// Executes a tool through the unified pipeline.
async fn run_tool(
    ctx: &ToolContext<'_>,
    session_id: &str,
    tool_name: &str,
    input: Value,
) -> anyhow::Result<AgentRun> {
    let Some(tool) = find_tool(tool_name) else {
        anyhow::bail!("unknown tool: {tool_name}");
    };

    let tool_call_id = Uuid::new_v4().to_string();
    let danger = tool.danger_level();

    if !danger.requires_approval() {
        // Read tool: execute immediately.
        match tool.execute(ctx, input.clone()).await {
            Ok(output) => {
                sqlx::query(
                    r#"
                    INSERT INTO agent_tool_calls
                        (id, session_id, tool_name, input, output, status, danger_level, requested_by)
                    VALUES ($1, $2, $3, $4, $5, 'executed', $6, $7)
                    "#,
                )
                .bind(&tool_call_id)
                .bind(session_id)
                .bind(tool_name)
                .bind(input.to_string())
                .bind(output.to_string())
                .bind(danger.as_str())
                .bind(ctx.user_id)
                .execute(&ctx.state.pool)
                .await?;

                let summary = summarize_tool_output(tool_name, &output);
                save_assistant(ctx.state, session_id, &summary).await?;
                Ok(AgentRun {
                    assistant_text: summary,
                    tool_calls: vec![json!({
                        "id": tool_call_id, "tool": tool_name, "status": "executed", "output": output,
                    })],
                    pending_approvals: vec![],
                })
            }
            Err(err) => {
                sqlx::query(
                    r#"
                    INSERT INTO agent_tool_calls
                        (id, session_id, tool_name, input, output, status, error, danger_level, requested_by)
                    VALUES ($1, $2, $3, $4, NULL, 'failed', $5, $6, $7)
                    "#,
                )
                .bind(&tool_call_id)
                .bind(session_id)
                .bind(tool_name)
                .bind(input.to_string())
                .bind(err.to_string())
                .bind(danger.as_str())
                .bind(ctx.user_id)
                .execute(&ctx.state.pool)
                .await?;
                let content = format!("工具执行失败：{err}");
                save_assistant(ctx.state, session_id, &content).await?;
                Ok(AgentRun {
                    assistant_text: content,
                    tool_calls: vec![],
                    pending_approvals: vec![],
                })
            }
        }
    } else {
        // Write tool: create pending approval, do NOT execute.
        sqlx::query(
            r#"
            INSERT INTO agent_tool_calls
                (id, session_id, tool_name, input, status, requires_approval, danger_level, requested_by)
            VALUES ($1, $2, $3, $4, 'pending', TRUE, $5, $6)
            "#,
        )
        .bind(&tool_call_id)
        .bind(session_id)
        .bind(tool_name)
        .bind(input.to_string())
        .bind(danger.as_str())
        .bind(ctx.user_id)
        .execute(&ctx.state.pool)
        .await?;

        let content = format!(
            "已生成待确认动作：{}（{} 级）。确认后才会真正写入。",
            tool.description(),
            danger.as_str()
        );
        save_assistant(ctx.state, session_id, &content).await?;
        Ok(AgentRun {
            assistant_text: content,
            tool_calls: vec![json!({
                "id": tool_call_id, "tool": tool_name, "status": "pending",
                "danger_level": danger.as_str(), "input": input,
            })],
            pending_approvals: vec![json!({
                "id": tool_call_id, "tool": tool_name, "danger_level": danger.as_str(),
                "input": input, "description": tool.description(),
            })],
        })
    }
}

pub async fn save_assistant(
    state: &AppState,
    session_id: &str,
    content: &str,
) -> anyhow::Result<()> {
    let msg_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO agent_messages (id, session_id, role, content) VALUES ($1, $2, 'assistant', $3)",
    )
    .bind(&msg_id)
    .bind(session_id)
    .bind(content)
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub fn summarize_tool_output(tool_name: &str, output: &Value) -> String {
    match tool_name {
        "list_nodes" => {
            let total = output["total"].as_i64().unwrap_or(0);
            if total == 0 {
                "当前案件没有时间轴节点。".to_string()
            } else {
                let titles: Vec<String> = output["nodes"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .take(5)
                            .filter_map(|n| n["title"].as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                format!("共 {total} 个节点：{}", titles.join("；"))
            }
        }
        "list_files" => {
            let total = output["total"].as_i64().unwrap_or(0);
            if total == 0 {
                "当前案件没有证据文件。".to_string()
            } else {
                let names: Vec<String> = output["files"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .take(5)
                            .filter_map(|f| f["original_name"].as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                format!("共 {total} 份证据文件：{}", names.join("；"))
            }
        }
        "list_persons" => {
            let total = output["total"].as_i64().unwrap_or(0);
            if total == 0 {
                "当前案件没有人物。".to_string()
            } else {
                let names: Vec<String> = output["persons"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .take(5)
                            .filter_map(|p| p["name"].as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                format!("共 {total} 个人物：{}", names.join("；"))
            }
        }
        "dedupe_persons" => {
            let groups = output["groups"].as_array().map(|a| a.len()).unwrap_or(0);
            if groups == 0 {
                "没有发现疑似重复的人物。".to_string()
            } else {
                format!("发现 {groups} 组疑似重复人物。")
            }
        }
        "search" => {
            let total = output["total"].as_i64().unwrap_or(0);
            if total == 0 {
                "没有找到匹配结果。".to_string()
            } else {
                let titles: Vec<String> = output["results"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .take(5)
                            .filter_map(|r| r["title"].as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                format!("找到 {total} 条结果：{}", titles.join("；"))
            }
        }
        "create_node" => {
            format!(
                "已创建节点「{}」（{}）。",
                output["title"].as_str().unwrap_or(""),
                output["event_time"].as_str().unwrap_or("")
            )
        }
        "export_evidence_list" => {
            format!(
                "证据清单已生成：{}",
                output["file_name"].as_str().unwrap_or("")
            )
        }
        "export_timeline_report" => {
            format!(
                "时间轴报告已生成：{}",
                output["file_name"].as_str().unwrap_or("")
            )
        }
        _ => format!("{tool_name} 完成。"),
    }
}

fn help_text() -> String {
    [
        "我可以帮你处理案件工作台里的常规操作：",
        "· 列出时间轴节点 / 证据文件 / 人物",
        "· 搜索案件材料（关键词）",
        "· 人物去重建议",
        "· 新建节点（标题 + 日期）、导出证据清单 / 时间轴报告",
        "",
        "直接输入意图即可，例如：「列出节点」「搜索 补充协议」「新建节点「签约」2024-01-12」",
        "写操作（新建、导出、合并、删除）会先生成待确认动作，确认后才执行。",
    ]
    .join("\n")
}

