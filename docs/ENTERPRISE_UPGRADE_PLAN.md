# LegalGenie Agent 企业级升级规划

Last updated: 2026-08-10
状态：**Phase 0 + P1 + P2 完成（2026-08-10）**：PostgreSQL 迁移、可靠任务执行、服务端 Agent 运行时均已交付；P3 企业存储与扩展 / P4 可观测合规待启动
范围：后端（`server/`）为主，前端仅在 Phase 2 涉及流式消费改造

---

## 1. 背景与目标

### 1.1 现状定位

LegalGenie Agent 当前成熟度为 **L4（Deliverable）**：单实例可交付、文档与实现对齐、权限边界已强制。
但面向"真正企业级可用"（多用户团队、合规审计、7x24 运行、水平扩展）仍有系统性缺口。

### 1.2 企业级定义（本规划采用的标准）

| 维度 | 企业级要求 |
|---|---|
| 可靠性 | 任务可恢复（进程崩溃/重启不丢工作）、幂等、超时与重试有边界 |
| 安全性 | 密钥强制管理、细粒度授权、审计链完整、防注入 |
| 可扩展性 | 多实例部署、无共享状态冲突、存储可水平扩展 |
| 可观测性 | 指标/追踪/结构化日志贯通、健康检查可依赖、告警可配置 |
| 合规性 | 审计日志不可篡改、敏感数据脱敏、数据保留策略 |
| 运维性 | 优雅停机、平滑升级、备份恢复演练 |

### 1.3 基准参照

以 opencode / Claude Code / Codex 的 agent 后端架构为参照系：

- **Agent 循环**：plan → act → observe → repeat，会话可持久化、可恢复
- **工具抽象**：统一 Tool trait + JSON Schema 描述，模型无关
- **权限/审批引擎**：分级权限 + allow/deny/ask 策略 + 审批记录持久化
- **流式事件**：事件流（event bus / SSE）贯通所有长任务
- **执行沙箱/超时**：任务有租约、超时、退避、死信

---

## 2. 差距清单（现状 → 目标）

基于代码审查（`server/src/`）确认的差距：

| # | 差距 | 证据（代码位置） | 影响 | 对应阶段 |
|---|---|---|---|---|
| G1 | **无服务端 Agent 运行时**：chat 意图解析在前端 `agent.ts`，后端只有 CRUD；审批队列在前端内存，刷新即失 | `web/src/lib/agent.ts`；后端无 `/sessions` | 智能能力不落后端，无法多端复用、审计回放、真实 LLM 接入 | P2 |
| G2 | **任务执行无持久化队列**：解析/翻译均为 `tokio::spawn` + DB 状态位 | `parser.rs::enqueue_parse`、`translation.rs` | 进程重启丢失执行中任务；多实例重复处理；无任务级超时/死信 | P1 |
| G3 | **外部命令无超时**：`pdftotext/tesseract/ffmpeg` 可无限挂起 | `parser.rs::run_cmd_capture_stdout` | 恶意/损坏文件可拖死 worker | P1 |
| G4 | **无优雅停机**：SIGTERM 直接退出，进行中任务无检查点 | `main.rs`（无 shutdown 处理） | 更新/发布丢任务 | P1 |
| G5 | **限流无 Retry-After**：429 响应无重试语义；限流器为进程内存 | `rate_limit.rs` | 客户端无法智能退避；多实例限流失效 | P0/P3 |
| G6 | **幂等缺失**：上传/解析/翻译重试可能双处理（`mark_processing` 的 `force` 分支与普通分支相同，为死代码） | `parser.rs::mark_processing` | 用户重试导致重复资源消耗 | P0 |
| G7 | **权限是角色门而非策略**：无操作级权限矩阵、无服务端审批流（前端审批是 UI 概念） | `access.rs` | 无法表达"member 可建节点但不可删"或"高风险操作需确认" | P2 |
| G8 | **并发一致性**：SQLite 单写者；`MAX(sort_order)+1` 无锁；access 检查与操作非事务（TOCTOU） | `case_timeline.rs`、`access.rs` | 并发创建节点冲突、权限变更竞态 | P1/P3 |
| G9 | **可观测性不足**：仅 tracing 日志，无指标、无 trace 贯通、health 无依赖检查 | `routes/health.rs` | 无法定位积压/故障 | P4 |
| G10 | **存储不可扩展**：SQLite + 本地 FS，无对象存储抽象 | `config.rs`、`case_files.rs` | 单机上限，无法异地容灾 | P3 |
| G11 | **密钥管理**：`TRANSLATION_API_KEY` 明文 env；JWT 校验已有（生产 32+ 非默认），但缺少轮换机制 | `config.rs:164-167` | 密钥泄露风险 | P0 |
| G12 | **搜索中文分词**：FTS5 默认分词器对 CJK 按整段 token，仅前缀命中 | `search.rs` | 中文检索体验差（已在前端观察到） | P3 |

---

## 3. 目标架构（TO-BE）

```
┌────────────────────────────────────────────────────────┐
│ Web 前端（React）                                      │
│  · SSE 事件流消费（agent 步进 / 任务进度 / 审批推送）    │
└──────────────────────┬─────────────────────────────────┘
                       │ HTTPS  /api/v1
┌──────────────────────▼─────────────────────────────────┐
│ API 层（Axum）                                          │
│  · 认证：JWT access+refresh（已具备）+ 密钥轮换         │
│  · 授权：RBAC 决策点（operation × role → allow/deny/ask）│
│  · 幂等中间件（Idempotency-Key → 响应缓存）             │
│  · 限流：Redis 或 DB-backed，429 带 Retry-After         │
│  · SSE 端点：/events（会话内订阅）                      │
├────────────────────────────────────────────────────────┤
│ 领域层                                                  │
│  · 案件 / 时间轴 / 证据 / 人物 / 导出（重构为服务）      │
│  · Agent 会话运行时：sessions + messages + tool_calls    │
│  · 审批流：approval_requests（挂起/确认/拒绝/过期）      │
├────────────────────────────────────────────────────────┤
│ 执行层（独立 worker 进程，可水平扩展）                   │
│  · 任务队列：jobs 表 + SKIP LOCKED 租约                 │
│  · 解析 worker / 翻译 worker / 导出 worker / 死信回收   │
│  · 统一重试策略：指数退避 + jitter + 上限               │
├────────────────────────────────────────────────────────┤
│ 存储层                                                  │
│  · PostgreSQL 16（主库；SQLite → PG 迁移）              │
│  · 对象存储抽象：本地 FS（默认）→ MinIO/S3（可选）       │
│  · Redis（可选部署）：限流计数 / 分布式锁 / 事件扇出     │
└────────────────────────────────────────────────────────┘
```

### 3.1 关键技术决策

| 决策点 | 选择 | 理由 |
|---|---|---|
| 数据库 | **PostgreSQL 16**（迁移） | 多实例并发写、SKIP LOCKED 队列、tsvector 中文分词（pg_jieba/pg_trgm）、JSONB、成熟备份工具 |
| 任务队列 | **Postgres jobs 表 + SKIP LOCKED**（自研，约 200 行） | 不引入新基础设施（自托管友好）；与业务事务一致；SQLx 原生支持；避免 Redis 成为单点 |
| 流式通道 | **SSE**（`text/event-stream`） | 服务端→客户端单向事件足够（agent 步进/任务状态/审批）；浏览器原生 EventSource；代理友好 |
| Agent 协议 | **自研轻量运行时**，工具用 JSON Schema；预留 MCP 作为工具扩展协议 | 借鉴 opencode 事件模型但不绑定其实现；MCP 生态可后续接入 |
| 对象存储 | **trait 抽象 + 本地实现**（默认），S3/MinIO 适配器后续 | 保持自托管默认体验，企业部署可切 |
| 迁移路径 | 双写迁移（SQLite→PG 同步工具脚本）→ 全量切换 | 降低切换风险 |

---

## 4. 分阶段路线图

### Phase 0 — 安全与可靠性基线（约 1 周）✅ 已完成

**目标**：消除可被轻易利用/导致数据损坏的低级风险。

- [x] P0-1 密钥管理
  - `TRANSLATION_API_KEY_FILE` 文件注入（容器/secret 友好）；翻译错误落库/日志前经 `redact_secret` 脱敏
  - `JWT_SECRET_OLD` 轮换窗口：旧密钥仅验签、新密钥签发；生产环境同样强制 32+ 非默认
- [x] P0-2 幂等
  - `Idempotency-Key` 中间件（`server/src/idempotency.rs` + migration 0016）：POST/PUT/PATCH/DELETE 有效，成功响应按 `sha256(method|path|key|auth)` 缓存 7 天，重放带 `x-idempotency-replayed`
  - 修复 `parser.rs::mark_processing` force 死代码：force=false 仅允许 pending/failed 重新解析，force=true 允许显式重解析
- [x] P0-3 限流语义
  - 429 响应携带 `Retry-After`（滑动窗口剩余秒数）；限流器返回 `(allowed, retry_after)`，含单元测试
- [x] P0-4 错误码矩阵审计
  - 产出 `docs/ERROR_CODE_MATRIX.md`：18 个在用码 + 有意保持的 4 项（防用户枚举/信息泄露）+ 4 个预留码（search/export/logs/members，随 P1/P2 落地）
- [x] P0-5 上传边界
  - 确认流式中断已实现（`case_files.rs` total > max_file_size 即中断并清理）；新增 11MB 超限测试（无残留文件行）

**验证**：`p0_security_smoke.rs`（5 用例：幂等重放/键隔离/失败不缓存/Retry-After/超限中断）+ 密钥轮换单元测试 + 脱敏单元测试；全量 60 tests 通过。

### Phase 1 — 可靠任务执行（约 2 周）

**目标**：解析/翻译/导出全部成为可恢复、可观测、可水平扩展的作业。

- [ ] P1-1 统一任务模型
  - `jobs` 表：`id, kind(parse|translate|export), payload(jsonb), status(pending|claimed|processing|succeeded|failed|cancelled|dead), lease_until, attempts, max_attempts, next_run_at, last_error, created_at, updated_at`
  - 状态机 + 迁移：现有 `parse_status`/`translation_status` 与 jobs 联动（保持对外 API 兼容）
- [ ] P1-2 Worker 执行器
  - 通用 worker 循环：`SELECT ... FOR UPDATE SKIP LOCKED` 领租约 → 执行 → 提交结果
  - 租约续期心跳；崩溃恢复：过期租约回 `pending` 并 `attempts+1`
  - 超时：外部命令统一 `Command::timeout`（解析/翻译/ffmpeg），默认 300s 可配置
  - 重试：指数退避 + jitter，`max_attempts` 后进死信（`dead`）并可人工重投
- [ ] P1-3 优雅停机
  - `axum::serve` graceful shutdown（SIGTERM/SIGINT）：停止领取 → drain 进行中任务（续期放宽）→ 退出码 0
- [ ] P1-4 事件 outbox
  - `job_events` 追加表（or 现有 oplog 扩展）：解析完成/失败/翻译进度 → 前端经轮询或 SSE 消费

**验证**：`kill -9` 恢复测试；双实例并行领取无重复；1000 任务吞吐基线；恶意文件超时测试。

### Phase 2 — Agent 运行时（约 3-4 周）

**目标**：把"对话驱动工作台"从前端规则升级为服务端 Agent 运行时，审批持久化可审计。

- [x] P2-1 会话模型（migration 0018）
  - `agent_sessions` + `agent_messages` + `agent_tool_calls`（含 requires_approval / danger_level / decided_by / decided_at）
- [x] P2-2 工具注册表（`server/src/agent/tools.rs`）
  - `Tool` trait：name / description / input_schema / danger_level / execute；`ToolContext` 注入 state + user + case
  - 首批 13 个工具：list_nodes / list_files / list_persons / dedupe_persons / search（读）；create_node / move_node / parse_file / create_person / export_evidence_list（写）；merge_persons（敏感）；delete_node（破坏）
  - 工具执行内强制 `ensure_case_write_access`（viewer 写操作服务端拒绝）
- [x] P2-3 审批持久化（服务端强制）
  - 写工具经统一管道创建 `pending` 工具调用（不执行）；`GET /agent/approvals/pending?case_id=` 拉取；`POST /agent/approvals/:id/confirm|reject` 决策
  - 审批按 requested_by 归属 + 案件访问控制双重校验；确认后才执行并写回 output，决策全程落库可审计
- [x] P2-4 执行引擎（`server/src/agent/engine.rs`）
  - 规则引擎默认：中文/英文/斜杠命令意图解析 → 工具管道；读立即执行、写进审批
  - LLM 意图识别为后续可插拔项（管道与工具执行解耦）
- [x] P2-5 前端适配
  - ChatView 改消费服务端会话（会话自动创建/复用、消息服务端持久化）；ContextPanel 审批队列改为服务端轮询 + 确认/拒绝直调 API
- 端点：`POST/GET /agent/sessions`、`GET/DELETE /agent/sessions/:id`、`POST/GET /agent/sessions/:id/messages`、`GET /agent/approvals/pending`、`POST /agent/approvals/:id/confirm|reject`

**验证**：端到端用例：自然语言 → 工具调用 → 审批 → 执行 → 审计回放（oplog 记录 approve/reject/execute 全链）。

### Phase 3 — 企业存储与扩展（约 3-4 周）

**目标**：多实例部署、存储可扩展、中文检索升级。

- [x] P3-1 PostgreSQL 迁移（提前完成：2026-08-10，作为 P1a）
  - 后端完全切换 PostgreSQL 16（sqlx postgres 驱动）；17 个 migration 位于 `server/migrations_pg/`
  - **存储层约定**：时间列 TEXT（UTC 'YYYY-MM-DD HH24:MI:SS'，`utc_text()`），保持 chrono-free 字符串语义；整型列 BIGINT；参数占位符统一 `$N`
  - FTS 迁移：FTS5 → **pg_trgm 镜像表**（业务表触发器同步 `searchable` 合并列 + 分语言列），CJK trigram 解决 G12 中文分词（实测「补充协议」可命中）
  - 语言模式列级过滤：zh/source 分别查 `searchable_translated`/`searchable_source`，bilingual 查合并列
  - 数据迁移工具 `scripts/migrate_sqlite_to_pg.py`；docker-compose 增加 postgres service（healthcheck + volume）
- [ ] P3-2 对象存储抽象
  - `ObjectStore` trait：`put/get/delete/presign`；本地 FS 实现（现有路径逻辑收拢）+ MinIO/S3 适配器
- [ ] P3-3 多实例
  - 限流/锁迁移至共享后端（PG 或 Redis）；worker 独立进程部署（compose profile / k8s manifest）
  - 并发一致性：`sort_order` 改为事务内 `SELECT MAX ... FOR UPDATE` 或宽松排序（双精度小数位）；access 检查与写操作合并为事务（或引入 `case_version` 乐观锁）
- [ ] P3-4 RBAC 矩阵
  - `permissions(role, operation)` 策略表：owner/member/viewer × 全部操作（含导出、解析、成员管理）
  - 审批策略可配置：`APPROVAL_POLICY=auto|ask|double`（双人审批企业选项）

**验证**：双实例并发压测；迁移演练（生产数据往返校验）；权限矩阵负向测试全量。

### Phase 4 — 可观测性与合规（约 2 周）

- [ ] P4-1 Metrics：Prometheus（`/metrics`）：HTTP 延迟直方图、队列深度、worker 状态、重试/死信计数
- [ ] P4-2 Tracing：OpenTelemetry + `request_id`/`job_id`/`session_id` 全链贯通（现有 oplog request_id 扩展）
- [ ] P4-3 日志脱敏：敏感字段（电话/邮箱/密钥）自动遮罩；`log_sensitive=true` 时仅 hash
- [ ] P4-4 审计增强：`operation_logs` append-only（禁止 UPDATE/DELETE 权限）+ 保留策略（`LOG_RETENTION_DAYS`）；审批链回放端点
- [ ] P4-5 健康检查：`/health/live`（进程存活）、`/health/ready`（DB/存储/队列依赖检查）
- [ ] P4-6 告警规则模板（Prometheus AlertManager：错误率、队列积压、租约过期）

**验证**：故障演练（杀 worker、断 DB、注入慢查询）→ 指标/告警可发现；合规自检清单。

---

## 5. 依赖与风险

| 风险 | 缓解 |
|---|---|
| SQLite → PG 迁移破坏现有数据 | 双写迁移 + 校验脚本 + 全量备份演练；Phase 3 前 API 保持兼容 |
| Agent 运行时范围膨胀 | 工具集保持首批 10 个以内；Provider 可回退本地规则引擎 |
| 多实例引入分布式复杂度 | 队列选 PG 内实现避免新单点；Redis 为可选部署 |
| 前端改造工作量 | Phase 2 前端仅改 ChatView + 审批拉取；其余视图不动 |
| 翻译/解析行为变化 | jobs 迁移保留原 `translation_status` 等对外字段，逐步对齐 |

## 6. 里程碑建议

- M1（P0+P1）：**可恢复执行基线** —— 崩溃不丢任务、重试有界、停机优雅
- M2（P2）：**服务端 Agent + 审批链** —— 交互升级为后端驱动，可审计回放
- M3（P3）：**企业部署形态** —— PG + 对象存储 + 多实例 + RBAC 矩阵
- M4（P4）：**可观测与合规** —— 指标/追踪/审计/告警就绪

每阶段结束运行一次 `scripts/acceptance_api_smoke.sh` + 新增阶段用例，作为回归门槛。

## 7. 不在本次范围

- 实时多人协作画布（WebSocket 白板）
- 计费/租户隔离（多租户需 Tenant ID 贯穿，另行立项）
- 移动端
