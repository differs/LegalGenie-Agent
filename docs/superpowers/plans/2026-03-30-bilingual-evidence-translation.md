# 双语证据解析与自动翻译 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为证据文件链路增加“解析阶段分片 + 解析后自动翻译 + 默认中文搜索 + 默认双语阅读”，同时保持现有文件主链可用且不强制重写现有 AI 抽取链。

**Architecture:** 以后端 `evidence_file_chunks` 作为内容真源，解析器先产出稳定 chunk，再由后台翻译 worker 消费 chunk 写入中文译文。文件级接口只暴露状态聚合与兼容字段；搜索和文件阅读都转向 chunk 数据，但对旧文件保留基于 `parsed_text` 的兼容回退。

**Tech Stack:** Rust, Axum, SQLx, SQLite FTS5, Tokio, Dioxus 0.7.3, serde, reqwest/gloo-net

---

## Planned File Structure

### Backend schema and config

- Create: `server/migrations/0014_evidence_file_translation.sql`
- 为 `evidence_files` 增加翻译状态字段
  - 新建 `evidence_file_chunks`
- Create: `server/migrations/0015_search_evidence_chunks.sql`
  - 在 Task 5 一次性新增 chunk 级 FTS 表和触发器
- Modify: `server/src/config.rs`
  - 新增翻译 provider 配置
- Modify: `server/src/state.rs`
  - 挂载可注入的 `TranslationProvider`
- Modify: `server/src/lib.rs`
  - 注册新模块

### Backend parsing and translation

- Create: `server/src/translation.rs`
  - `TranslationProvider` 抽象
  - chunk 翻译 worker
  - 重试/幂等规则
- Modify: `server/src/parser.rs`
  - 解析阶段产出 chunks
  - 持久化 chunks
  - 解析完成后触发翻译

### Backend routes and search

- Modify: `server/src/routes/files.rs`
  - 文件详情增强
  - 新增 chunks / translation / translate retry 接口
- Modify: `server/src/routes/case_files.rs`
  - 列表与上传响应带翻译聚合状态
- Modify: `server/src/routes/search.rs`
  - `language_mode`
  - chunk 搜索与旧索引回退

### Frontend files page

- Modify: `frontend/src/models.rs`
  - 增加 chunk / translation / search 返回结构
- Modify: `frontend/src/api.rs`
  - chunks / translation / retry / search language_mode API
- Modify: `frontend/src/pages/files.rs`
  - 默认双语阅读
  - 视图切换
  - 翻译状态与重试
- Modify: `frontend/src/pages/search.rs`
  - `language_mode`
  - bilingual evidence hit 展示与 fallback 标记

### Tests

- Create: `server/tests/translation_smoke.rs`
  - 覆盖 chunk、自动翻译、重试与搜索行为
  - 复用 `api_smoke.rs` / `permissions_smoke.rs` 现有 helper 风格：`build_test_app`、`request_json`、`request_raw`、`request_multipart_text`
- Modify: `server/tests/api_smoke.rs`
  - 保持文件上传/解析主路径回归
- Modify: `frontend/src/pages/files.rs`
  - 增加阅读状态和视图切换测试
- Modify: `frontend/src/api.rs`
  - 增加 query/path 组装单测

## Task 1: 建立数据库真源和翻译配置

**Files:**
- Create: `server/migrations/0014_evidence_file_translation.sql`
- Modify: `server/src/config.rs`
- Modify: `server/src/state.rs`
- Modify: `server/src/lib.rs`
- Test: `server/tests/translation_smoke.rs`

- [ ] **Step 1: 写失败测试，锁定新 schema 和配置会被使用**

```rust
#[tokio::test]
async fn translated_chunk_schema_is_available() {
    let (app, _tmp) = build_test_app().await;
    let cols = sqlx::query_scalar::<_, String>(
        "SELECT name FROM pragma_table_info('evidence_file_chunks') ORDER BY cid"
    )
    .fetch_all(test_pool(&app).await)
    .await
    .unwrap();

    assert!(cols.contains(&"translated_text".to_string()));
    assert!(cols.contains(&"source_text_hash".to_string()));
    assert!(cols.contains(&"display_label".to_string()));
}
```

- [ ] **Step 2: 运行失败测试，确认迁移前不存在新表/新列**

Run: `cargo test -p legalminds-server --test translation_smoke translated_chunk_schema_is_available -- --exact`

Expected: FAIL，提示 `no such table: evidence_file_chunks` 或缺少列。

- [ ] **Step 3: 写迁移和配置最小实现**

```sql
ALTER TABLE evidence_files ADD COLUMN translation_status TEXT NOT NULL DEFAULT 'pending';
ALTER TABLE evidence_files ADD COLUMN translation_error TEXT;
ALTER TABLE evidence_files ADD COLUMN source_language TEXT;
ALTER TABLE evidence_files ADD COLUMN target_language TEXT NOT NULL DEFAULT 'zh-CN';
ALTER TABLE evidence_files ADD COLUMN chunk_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE evidence_files ADD COLUMN translated_chunk_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE evidence_files ADD COLUMN failed_chunk_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE evidence_files ADD COLUMN translation_model TEXT;
ALTER TABLE evidence_files ADD COLUMN translation_provider TEXT;

CREATE TABLE evidence_file_chunks (
  id TEXT PRIMARY KEY,
  evidence_id TEXT NOT NULL,
  case_id TEXT NOT NULL,
  chunk_index INTEGER NOT NULL,
  page_number INTEGER,
  segment_number INTEGER,
  chunk_kind TEXT NOT NULL,
  source_text TEXT NOT NULL,
  translated_text TEXT,
  source_language TEXT,
  translation_status TEXT NOT NULL DEFAULT 'pending',
  translation_error TEXT,
  char_count INTEGER NOT NULL,
  token_estimate INTEGER,
  source_text_hash TEXT NOT NULL,
  retry_count INTEGER NOT NULL DEFAULT 0,
  last_attempt_at TEXT,
  next_retry_at TEXT,
  display_label TEXT NOT NULL,
  anchor_json TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE (evidence_id, chunk_index),
  FOREIGN KEY (evidence_id) REFERENCES evidence_files(id)
);
```

- [ ] **Step 4: 增加配置项并让测试环境可构建**

配置名固定为：

- `TRANSLATION_PROVIDER`
- `TRANSLATION_BASE_URL`
- `TRANSLATION_API_KEY`
- `TRANSLATION_MODEL`
- `TRANSLATION_TARGET_LANGUAGE`
- `TRANSLATION_MAX_CONCURRENCY`
- `TRANSLATION_CHUNK_SIZE_LIMIT`

Run: `cargo test -p legalminds-server --test translation_smoke translated_chunk_schema_is_available -- --exact`

Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add server/migrations/0014_evidence_file_translation.sql server/src/config.rs server/src/state.rs server/src/lib.rs server/tests/translation_smoke.rs
git commit -m "feat: add bilingual evidence schema"
```

## Task 2: 解析阶段产出并持久化 chunks

**Files:**
- Modify: `server/src/parser.rs`
- Test: `server/tests/translation_smoke.rs`
- Reference: `server/src/routes/files.rs`, `server/src/routes/case_files.rs`

- [ ] **Step 1: 写失败测试，锁定 PDF/TXT 解析后会生成 chunk**

```rust
#[tokio::test]
async fn parsing_creates_chunks_for_uploaded_file() {
    let (app, _tmp) = build_test_app().await;
    let register = register_user(&app, "chunkuser", "chunkuser@example.com").await;
    let token = register.access_token;
    let case_id = create_case(&app, &token, "Chunk Case").await;
    let file_id = upload_text_file(&app, &token, &case_id, "evidence.txt", "Alpha\n\nBeta").await;

    let detail = wait_for_file_parse_done(&app, &token, &file_id).await;
    assert_eq!(detail["data"]["parse_status"], "done");

    let chunks = get_file_chunks(&app, &token, &file_id).await;
    assert!(!chunks["data"]["items"].as_array().unwrap().is_empty());
}
```

- [ ] **Step 2: 运行失败测试，确认当前解析链只有 `parsed_text` 没有 chunks**

Run: `cargo test -p legalminds-server --test translation_smoke parsing_creates_chunks_for_uploaded_file -- --exact`

Expected: FAIL，`/files/:id/chunks` 未实现或 chunk 为空。

- [ ] **Step 3: 在解析器里新增 chunk 产出与持久化**

```rust
struct ParsedChunk {
    chunk_index: i64,
    page_number: Option<i64>,
    segment_number: Option<i64>,
    chunk_kind: &'static str,
    display_label: String,
    source_text: String,
    source_text_hash: String,
    char_count: i64,
    token_estimate: Option<i64>,
    anchor_json: serde_json::Value,
}

async fn persist_chunks(pool: &SqlitePool, row: &EvidenceFileToParse, chunks: &[ParsedChunk]) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM evidence_file_chunks WHERE evidence_id = ?1")
        .bind(&row.id)
        .execute(pool)
        .await?;
    // insert chunks in chunk_index order and update chunk_count
    Ok(())
}

fn chunk_anchor_pdf(page_number: i64, part: Option<(i64, i64)>) -> serde_json::Value {
    json!({
        "locator_type": "pdf_page",
        "display_label": match part {
            Some((idx, total)) => format!("第 {page_number} 页({idx}/{total})"),
            None => format!("第 {page_number} 页"),
        },
        "page_number": page_number
    })
}
```

- [ ] **Step 4: 把各文件类型的 chunk 规则写死到实现与测试**

至少覆盖：

- PDF：
  - 默认每页一个 chunk
  - 如单页字符数超过 `TRANSLATION_CHUNK_SIZE_LIMIT`，页内按段落再细切
  - `display_label` 使用 `第 N 页` 或 `第 N 页(i/total)`
- DOCX / TXT / Markdown / OCR 长文：
  - 按标题、空行段落和字符数切分
  - 使用 `第 N 段`
  - 必须服从 `TRANSLATION_CHUNK_SIZE_LIMIT`
- XLSX：
  - 按 `sheet + block` 切分，`display_label` 形如 `Sheet A / A1:D20`
  - 必须服从 `TRANSLATION_CHUNK_SIZE_LIMIT`
- 音频转写：
  - 按时间段切分，`display_label` 形如 `00:03:20 - 00:04:10`
  - 必须服从 `TRANSLATION_CHUNK_SIZE_LIMIT`

如当前解析器拿不到精确 anchor：

- XLSX：在 `parse_excel_sync` 中按固定行窗口累计单元格，直接生成 `sheet_name + cell_range`
- 音频：优先从转写库获取真实 segment；如果 V1 只能拿到整段 transcript，则按总时长和字符比例生成近似 `start_ms/end_ms`，并在 `anchor_json` 中标记 `synthetic=true`

- [ ] **Step 5: 解析完成后重置并回写文件级翻译聚合字段**

每次重解析并重建 chunks 时，必须同步重置：

- `translation_status = 'pending'`
- `translation_error = NULL`
- `translated_chunk_count = 0`
- `failed_chunk_count = 0`
- `source_language = NULL`
- `translation_model = NULL`
- `translation_provider = NULL`

然后再回写新的 `chunk_count`，避免旧翻译状态污染新一轮 chunks。

Run: `cargo test -p legalminds-server --test translation_smoke parsing_creates_chunks_for_uploaded_file -- --exact`

Expected: PASS

- [ ] **Step 6: 提交**

```bash
git add server/src/parser.rs server/tests/translation_smoke.rs
git commit -m "feat: persist parsed evidence chunks"
```

## Task 3: 接入自动翻译 worker 和文件级聚合状态

**Files:**
- Create: `server/src/translation.rs`
- Modify: `server/src/parser.rs`
- Modify: `server/src/config.rs`
- Modify: `server/src/state.rs`
- Test: `server/tests/translation_smoke.rs`

- [ ] **Step 1: 写失败测试，锁定解析完成后会自动推进翻译状态**

```rust
#[tokio::test]
async fn parsing_auto_starts_translation() {
    let (app, _tmp) = build_test_app_with_fake_translation().await;
    let register = register_user(&app, "translator", "translator@example.com").await;
    let token = register.access_token;
    let case_id = create_case(&app, &token, "Translate Case").await;
    let file_id = upload_text_file(&app, &token, &case_id, "en.txt", "Payment due tomorrow.").await;

    wait_for_file_parse_done(&app, &token, &file_id).await;
    let translation = wait_for_translation_finished(&app, &token, &file_id).await;

    assert_eq!(translation["data"]["translation_status"], "done");
    assert_eq!(translation["data"]["translated_chunk_count"], 1);
}
```

- [ ] **Step 2: 运行失败测试，确认当前没有翻译 worker**

Run: `cargo test -p legalminds-server --test translation_smoke parsing_auto_starts_translation -- --exact`

Expected: FAIL，`translation_status` 不推进或接口不存在。

- [ ] **Step 3: 写最小翻译 provider 抽象和 fake provider**

```rust
pub trait TranslationProvider: Send + Sync {
    async fn translate(&self, source_text: &str, target_language: &str) -> anyhow::Result<TranslationResult>;
}

#[derive(Clone)]
pub struct AppState {
    pub translation_provider: Arc<dyn TranslationProvider>,
}

pub async fn enqueue_translation(
    state: AppState,
    evidence_id: String,
) -> anyhow::Result<()> {
    tokio::spawn(async move {
        let _ = translate_file_chunks(state, evidence_id).await;
    });
    Ok(())
}
```

测试 wiring 也必须明确：

- `build_test_app_with_fake_translation()` 基于现有 `build_test_app()` 复制一份 builder
- 在 `AppState` 中注入 `Arc<FakeTranslationProvider>`
- fake provider 返回稳定中译文，避免测试依赖外网模型

真实 provider 协议固定为 OpenAI-compatible HTTP：

- URL：`{TRANSLATION_BASE_URL}/chat/completions`
- Header：`Authorization: Bearer {TRANSLATION_API_KEY}`
- JSON：
  - `model = TRANSLATION_MODEL`
  - `messages = [{role: \"system\"}, {role: \"user\"}]`
- 响应从首个 choice 提取译文文本

提示词固定为：

- `system`：
  - 你是法律证据翻译引擎
  - 任务是翻译或规范化，不是摘要
  - 不得删减、压缩、改写证据事实
  - 保留人名、地名、机构名、法条名和专有名词可追溯性
  - 如果原文已是中文，则输出规范中文
- `user`：
  - 提供 `source_text`
  - 指明目标语言 `zh-CN`
  - 要求只返回译文正文

- [ ] **Step 4: 实现幂等、重试、并发限制、审计和文件级状态聚合**

必须明确落地：

- 自动重试上限 `3`
- 退避 `1m / 5m / 30m`
- 幂等键：`(evidence_id, chunk_index, translation_provider, translation_model, source_text_hash)`
- `chunk_size_limit` 以“字符数”为单位执行
- 已成功且幂等键不变的 chunk 不重写
- provider 调用日志只记录元数据，不记录 chunk 全文
- 文件级聚合同步回写 `translated_chunk_count` 和 `failed_chunk_count`
- 每次 chunk 翻译完成时记录 `evidence_file_chunks.source_language`
- 文件级 `source_language` 取该文件所有 chunk 的主语言汇总值
- app 启动时创建一个 translation retry poller：
  - `tokio::spawn`
  - 每 `30s` 扫描一次 `next_retry_at <= now()` 且 `translation_status = 'pending' | 'failed'`
  - 重新入队 chunk 翻译任务

Run: `cargo test -p legalminds-server --test translation_smoke`

Expected: PASS，至少覆盖自动翻译、`partial/failed` 聚合、手动重试不会覆盖已完成 chunk。

- [ ] **Step 5: 提交**

```bash
git add server/src/translation.rs server/src/parser.rs server/src/config.rs server/tests/translation_smoke.rs
git commit -m "feat: add automatic evidence translation worker"
```

## Task 4: 打通文件详情、chunks、translation 和 retry API

**Files:**
- Modify: `server/src/routes/files.rs`
- Modify: `server/src/routes/case_files.rs`
- Test: `server/tests/translation_smoke.rs`
- Reference: `server/src/routes/auth.rs`, `server/src/routes/logs.rs`

- [ ] **Step 1: 写失败测试，锁定文件详情和新接口契约**

```rust
#[tokio::test]
async fn file_detail_and_chunk_endpoints_expose_translation_state() {
    let (app, _tmp) = build_test_app_with_fake_translation().await;
    let register = register_user(&app, "filedetail", "filedetail@example.com").await;
    let token = register.access_token;
    let case_id = create_case(&app, &token, "File Detail Case").await;
    let file_id = upload_text_file(&app, &token, &case_id, "en.txt", "Clause A").await;

    wait_for_translation_finished(&app, &token, &file_id).await;

    let detail = request_json(&app, &token, &format!("/api/v1/files/{file_id}")).await;
    assert_eq!(detail["data"]["translation_status"], "done");

    let chunks = request_json(&app, &token, &format!("/api/v1/files/{file_id}/chunks?page=1&page_size=20&view_mode=bilingual")).await;
    assert_eq!(chunks["data"]["items"][0]["translation_status"], "done");
}
```

- [ ] **Step 2: 运行失败测试，确认接口尚未扩展**

Run: `cargo test -p legalminds-server --test translation_smoke file_detail_and_chunk_endpoints_expose_translation_state -- --exact`

Expected: FAIL

- [ ] **Step 3: 扩展文件详情/列表和新增 3 个文件接口**

```rust
Router::new()
    .route("/:id", get(get_file).delete(delete_file))
    .route("/:id/chunks", get(get_file_chunks))
    .route("/:id/translation", get(get_translation_status))
    .route("/:id/translate/retry", post(retry_translation));
```

`GET /api/v1/files/:id/translation` 最小返回契约必须写死为：

- `translation_status`
- `translation_error`
- `chunk_count`
- `translated_chunk_count`
- `failed_chunk_count`
- `source_language`
- `target_language`
- `translation_provider`
- `translation_model`
- `translation_incomplete`

- [ ] **Step 4: 增加权限校验、分页、`view_mode`、错误语义和审计最小元数据**

必须覆盖：

- `409`：解析未完成，chunks 不可读
- `422`：`scope=selected` 但缺少 `chunk_ids`
- `scope=all` 只重试未完成 chunk，已成功 chunk 返回 `skipped`

Run: `cargo test -p legalminds-server --test translation_smoke`

Expected: PASS，接口权限、分页、重试、已成功 chunk 的 skip 语义都通过。

- [ ] **Step 5: 提交**

```bash
git add server/src/routes/files.rs server/src/routes/case_files.rs server/tests/translation_smoke.rs
git commit -m "feat: expose bilingual evidence file APIs"
```

## Task 5: 将搜索真源切到 chunk，并保留旧文件回退

**Files:**
- Modify: `server/src/routes/search.rs`
- Create: `server/migrations/0015_search_evidence_chunks.sql`
- Test: `server/tests/translation_smoke.rs`
- Reference: `server/migrations/0004_search.sql`

- [ ] **Step 1: 写失败测试，锁定 `language_mode` 与中文/原文/双语搜索行为**

```rust
#[tokio::test]
async fn evidence_search_supports_zh_source_and_bilingual_modes() {
    let app = spawn_test_app_with_fake_translation().await;
    let (token, case_id) = create_case_with_owner(&app).await;
    let file_id = upload_text_file(&app, &token, &case_id, "en.txt", "Payment due tomorrow.").await;

    wait_for_translation_finished(&app, &token, &file_id).await;

    let zh = search_evidence(&app, &token, "付款", "zh").await;
    let source = search_evidence(&app, &token, "Payment", "source").await;
    let bilingual = search_evidence(&app, &token, "Payment", "bilingual").await;

    assert_eq!(zh["data"]["items"][0]["file_id"], file_id);
    assert_eq!(zh["data"]["items"][0]["language_mode"], "zh");
    assert!(zh["data"]["items"][0]["file_name"].as_str().unwrap().contains("en.txt"));
    assert!(source["data"]["items"][0]["anchor_json"].is_object());
    assert!(bilingual["data"]["items"][0]["snippet_source"].is_string());
}

#[tokio::test]
async fn global_search_accepts_language_mode_for_evidence_hits() {
    let (app, _tmp) = build_test_app_with_fake_translation().await;
    let register = register_user(&app, "globalsearch", "globalsearch@example.com").await;
    let token = register.access_token;
    let case_id = create_case(&app, &token, "Global Search Case").await;
    let _file_id = upload_text_file(&app, &token, &case_id, "en.txt", "Payment due tomorrow.").await;

    let resp = request_json(
        &app,
        Method::GET,
        "/api/v1/search?q=Payment&language_mode=bilingual",
        Some(&token),
        json!({})
    )
    .await;

    assert_eq!(resp.0, StatusCode::OK);
}
```

- [ ] **Step 2: 运行失败测试，确认当前搜索只有 `parsed_text` 真源**

Run: `cargo test -p legalminds-server --test translation_smoke evidence_search_supports_zh_source_and_bilingual_modes -- --exact`

Expected: FAIL

- [ ] **Step 3: 建 chunk FTS，扩展 `/api/v1/search` 和 `/api/v1/search/evidence`**

```sql
CREATE VIRTUAL TABLE search_evidence_chunks_source USING fts5(source_text, content='evidence_file_chunks', content_rowid='rowid');
CREATE VIRTUAL TABLE search_evidence_chunks_translated USING fts5(translated_text, content='evidence_file_chunks', content_rowid='rowid');

CREATE TRIGGER evidence_chunks_ai AFTER INSERT ON evidence_file_chunks BEGIN
  INSERT INTO search_evidence_chunks_source(rowid, source_text) VALUES (new.rowid, new.source_text);
  INSERT INTO search_evidence_chunks_translated(rowid, translated_text) VALUES (new.rowid, coalesce(new.translated_text, ''));
END;

CREATE TRIGGER evidence_chunks_au AFTER UPDATE ON evidence_file_chunks BEGIN
  INSERT INTO search_evidence_chunks_source(search_evidence_chunks_source, rowid, source_text)
  VALUES('delete', old.rowid, old.source_text);
  INSERT INTO search_evidence_chunks_source(rowid, source_text) VALUES (new.rowid, new.source_text);
  INSERT INTO search_evidence_chunks_translated(search_evidence_chunks_translated, rowid, translated_text)
  VALUES('delete', old.rowid, old.translated_text);
  INSERT INTO search_evidence_chunks_translated(rowid, translated_text)
  VALUES (new.rowid, coalesce(new.translated_text, ''));
END;

CREATE TRIGGER evidence_chunks_ad AFTER DELETE ON evidence_file_chunks BEGIN
  INSERT INTO search_evidence_chunks_source(search_evidence_chunks_source, rowid, source_text)
  VALUES('delete', old.rowid, old.source_text);
  INSERT INTO search_evidence_chunks_translated(search_evidence_chunks_translated, rowid, translated_text)
  VALUES('delete', old.rowid, old.translated_text);
END;
```

- [ ] **Step 4: 扩展 evidence 搜索响应契约，并实现 `zh` 未翻译完成语义**

evidence 搜索返回至少新增：

- `language_mode`
- `file_id`
- `file_name`
- `chunk_id`
- `chunk_index`
- `display_label`
- `anchor_json`
- `matched_language`
- `snippet_source`
- `snippet_translated`
- `match_start_offset`
- `match_end_offset`
- `translation_incomplete`
- `source_fallback`

Run: `cargo test -p legalminds-server --test translation_smoke`

Expected: PASS，包含旧文件回退和 `translation_incomplete` 分支。

必须写成测试断言的规则：

- 新文件 `language_mode=zh` 且翻译未覆盖全部 chunk 时，只查中文 FTS，不查原文 FTS
- 此时返回 `translation_incomplete=true`
- 同时返回 `source_fallback=false`

- [ ] **Step 5: 提交**

```bash
git add server/migrations/0015_search_evidence_chunks.sql server/src/routes/search.rs server/tests/translation_smoke.rs
git commit -m "feat: add chunk-based bilingual evidence search"
```

## Task 6: 前端接 chunks/translation API，并改造文件页为默认双语阅读

**Files:**
- Modify: `frontend/src/models.rs`
- Modify: `frontend/src/api.rs`
- Modify: `frontend/src/pages/files.rs`
- Modify: `frontend/src/pages/search.rs`
- Test: `frontend/src/api.rs`
- Test: `frontend/src/pages/files.rs`

- [ ] **Step 1: 写失败测试，锁定前端文件页默认双语视图和状态文案**

```rust
#[test]
fn file_reader_defaults_to_bilingual_view() {
    let state = FileReaderState::new();
    assert_eq!(state.view_mode, FileViewMode::Bilingual);
}
```

- [ ] **Step 2: 运行失败测试，确认前端模型/API 还不认识 chunks 和 translation**

Run: `cargo test -p legalminds-frontend file_reader_defaults_to_bilingual_view -- --exact`

Expected: FAIL

- [ ] **Step 3: 增加模型和 API 封装**

```rust
pub struct EvidenceFileChunkItem {
    pub id: String,
    pub chunk_index: i64,
    pub display_label: String,
    pub source_text: String,
    pub translated_text: Option<String>,
    pub translation_status: String,
    pub anchor_json: serde_json::Value,
}

pub async fn get_file_chunks(...) -> Result<FileChunkListData, String> { ... }
pub async fn get_file_translation(...) -> Result<FileTranslationData, String> { ... }
pub async fn post_retry_translation(...) -> Result<serde_json::Value, String> { ... }
```

- [ ] **Step 4: 改文件页为“状态条 + 视图切换 + 分页导航 + 双语正文 + 重试按钮”**

同时补齐这两个前端分支：

- 老文件无 chunk 时，显示降级提示并退回整份原文阅读
- `translation_incomplete=true` 时，显示“中文索引构建中，可切到原文或双语搜索”
- 搜索页新增 `language_mode = zh | source | bilingual` 切换，并识别 `source_fallback` 标记

- [ ] **Step 5: 明确 search 页面语言切换测试**

Run: `cargo test -p legalminds-frontend search_language_mode_toggle_updates_request -- --exact`

Expected: PASS

Run: `cargo test -p legalminds-frontend`

Expected: PASS

- [ ] **Step 6: 提交**

```bash
git add frontend/src/models.rs frontend/src/api.rs frontend/src/pages/files.rs frontend/src/pages/search.rs
git commit -m "feat: add bilingual evidence reader UI"
```

## Task 7: 回归、兼容和文档收尾

**Files:**
- Modify: `server/tests/api_smoke.rs`
- Modify: `docs/RUNBOOK_LOCAL.md`
- Modify: `README.md`

- [ ] **Step 1: 补兼容测试，锁定旧文件仍能退回原文阅读/搜索**

```rust
#[tokio::test]
async fn legacy_file_without_chunks_still_remains_usable() {
    let (app, _tmp, pool) = build_test_app_with_pool().await;
    let token = register_user(&app, "legacy_user", "legacy_user@example.com").await.1;
    let case_id = create_case(&app, &token, "Legacy Case").await;
    let file_id = insert_legacy_file_row_with_parsed_text_only(&pool, &case_id, "legacy.txt", "Legacy source text").await;

    let detail = request_json(&app, Method::GET, &format!("/api/v1/files/{file_id}"), Some(&token), json!({})).await;
    assert_eq!(detail.0, StatusCode::OK);

    let search = request_json(&app, Method::GET, "/api/v1/search/evidence?q=Legacy&language_mode=zh", Some(&token), json!({})).await;
    assert_eq!(search.1["data"]["items"][0]["source_fallback"], true);
}
```

- [ ] **Step 2: 运行后端和前端完整回归**

Run:

```bash
cargo test -p legalminds-server --test api_smoke
cargo test -p legalminds-server --test translation_smoke
cargo test -p legalminds-frontend
```

Expected: 全绿

- [ ] **Step 3: 验证 web 和 desktop 构建，并确认 files/search 页面会自动刷新翻译状态**

要求：

- Files 页面轮询不只盯 `parse_status=processing`，也要盯 `translation_status=processing`
- Search 页面默认 `zh`，但可切 `source/bilingual`

Run:

```bash
cargo build -p legalminds-frontend --target wasm32-unknown-unknown
cargo build -p legalminds-frontend --no-default-features --features desktop
```

Expected: 两条构建都通过

- [ ] **Step 4: 更新运行说明**

```md
- 新增翻译相关环境变量
- 说明 fake provider / 云端 provider / 本地 provider 切换方式
- 说明旧文件需重新解析才会获得双语分片
```

- [ ] **Step 5: 提交**

```bash
git add server/tests/api_smoke.rs docs/RUNBOOK_LOCAL.md README.md
git commit -m "docs: document bilingual evidence translation flow"
```
