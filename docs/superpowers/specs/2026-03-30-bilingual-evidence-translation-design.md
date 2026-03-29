# 双语证据解析与自动翻译设计规格

## 状态

- 已确认
- 本规格覆盖跨国语言案件资料的自动翻译能力

## 背景

LegalMinds 当前已经具备证据文件上传、异步解析、解析结果下载、文件详情展示、全文搜索等能力，但整个链路默认只围绕单份 `parsed_text` 工作。

这在处理跨国案件时存在明显缺口：

- 大量证据并非中文，律师需要中文工作视图
- 当前没有“解析后自动翻译”的后台任务链
- 搜索、AI 抽取、阅读、引用都缺少稳定的页码/段落锚点
- 大文件如果只保留整份文本，后续翻译、重试、定位和导出都会变脆弱

本轮目标不是增加一个临时翻译按钮，而是把“解析、分片、翻译、搜索、阅读、AI 抽取”收成一条完整主链路。

## 目标

实现一条适用于跨国语言案件资料的双语处理链路，满足以下结果：

1. 文件上传并解析完成后，自动启动翻译任务
2. 原文与中文译文同时保留，不覆盖原始解析结果
3. 解析阶段就按页/段产出稳定分片，作为后续所有能力的真源
4. 前端文件阅读区默认展示双语对照
5. 搜索默认搜中文，但支持切到原文或双语
6. 后续 AI 抽取默认同时消费原文和中文
7. 翻译失败不影响文件的基础可用性，并支持重试
8. 云端与本地部署翻译模型共用同一后端抽象，按配置切换

## 非目标

- 不在这一轮引入独立微服务做翻译编排
- 不把全文翻译改成前端同步调用
- 不把文件处理主链路改成强依赖人工手动触发
- 不在这一轮重写所有旧文件历史数据
- 不把翻译能力扩展成摘要、改写或事实压缩功能

## 已确认决策

- 触发方式：上传后解析完成自动执行翻译
- 数据保留：原文与中文双存
- 模型接入：云端和本地都支持，按配置切换，本质是 `base_url` 和模型配置切换
- 触发范围：所有已解析文本都送翻译，由模型判断是否需要翻译或规范化
- 搜索行为：默认搜中文，可切原文或双语
- 失败语义：解析成功但翻译失败时，文件仍算可用，只标记“翻译失败，可重试”
- 阅读默认视图：双语对照
- AI 消费文本：原文 + 中文一起提供给模型

## 设计原则

### 1. 解析先于翻译

分页/分段必须在解析阶段完成，而不是在翻译阶段临时切块。解析阶段产出的稳定分片是搜索、阅读、引用、AI 抽取和翻译重试的共同锚点。

### 2. 分片是真源

`parsed_text` 继续保留做兼容字段，但真正的内容真源应当转向“按页/段存储的 chunks”。

### 3. 解析状态与翻译状态解耦

解析成功不等于翻译成功。文件可在 `parse=done` 时立即可用，翻译状态独立推进。

### 4. 中文优先但必须可回溯原文

系统主工作视图和默认搜索面向中文，但所有结果都必须能够回到原文片段与出处。

### 5. 大文件优先设计

设计必须首先适配大 PDF、长判决书、OCR 长文、转写长音频。不能假设整份文本可一次性稳定翻译。

## 总体架构

文件处理链路调整为：

1. 文件上传成功
2. 解析任务启动
3. 解析器按文件类型产出结构化 chunks
4. chunks 落库，文件级解析状态更新为 `done`
5. 自动触发翻译任务
6. 翻译器逐 chunk 产出中文译文
7. 文件级翻译状态按 chunk 聚合
8. 搜索索引基于 chunk 构建，默认中文
9. 前端文件页默认双语对照展示

关键边界：

- `evidence_files.parsed_text` 保留，但不再作为唯一真源
- `evidence_file_chunks` 成为后续阅读、搜索、翻译、引用、AI 抽取的核心表
- 解析与翻译分别有独立状态和错误字段

## 数据模型

### 文件级：增强 `evidence_files`

在现有 `evidence_files` 基础上新增文件级翻译字段：

- `translation_status`
- `translation_error`
- `source_language`
- `target_language`
- `chunk_count`
- `translated_chunk_count`
- `translation_model`
- `translation_provider`

说明：

- `parse_status` 只描述解析
- `translation_status` 只描述翻译
- 文件列表页和详情页显示的是文件级聚合状态

### 分片级：新增 `evidence_file_chunks`

新增核心表 `evidence_file_chunks`，至少包含：

- `id`
- `evidence_id`
- `case_id`
- `chunk_index`
- `page_number`
- `segment_number`
- `chunk_kind`
- `source_text`
- `translated_text`
- `source_language`
- `translation_status`
- `translation_error`
- `char_count`
- `token_estimate`
- `source_text_hash`
- `retry_count`
- `last_attempt_at`
- `next_retry_at`
- `anchor_json`
- `created_at`
- `updated_at`

字段语义：

- `evidence_id` 明确外键指向 `evidence_files.id`，与现有 `/api/v1/files/:id` 中的 `:id` 是同一个文件实体 ID，不引入新的“证据主表”概念
- `page_number` 用于 PDF 真实页码
- `segment_number` 用于非 PDF 逻辑段分页
- `chunk_kind` 标识分片来源，如 `pdf_page`、`doc_block`、`sheet_block`、`transcript_segment`
- `source_text_hash` 用于翻译幂等与结果复用
- `retry_count / last_attempt_at / next_retry_at` 用于后台重试调度
- `anchor_json` 存页内偏移、标题层级、sheet 名、时间戳等文件类型相关锚点

建议约束与索引：

- 外键：`evidence_file_chunks.evidence_id -> evidence_files.id`
- 唯一约束：`(evidence_id, chunk_index)`
- 常用索引：
  - `(case_id, evidence_id, chunk_index)`
  - `(evidence_id, page_number)`
  - `(evidence_id, segment_number)`
  - `(evidence_id, translation_status)`

## 状态机

### 文件级解析状态

- `pending`
- `processing`
- `done`
- `failed`

### 文件级翻译状态

- `pending`
- `processing`
- `done`
- `partial`
- `failed`

### 状态推进

- 上传成功：`parse=pending, translation=pending`
- 解析进行中：`parse=processing`
- 解析完成并写入 chunks：`parse=done, translation=processing`
- 全部分片翻译完成：`translation=done`
- 部分分片翻译失败：`translation=partial`
- 全部分片翻译失败：`translation=failed`

约束：

- `parse=done + translation=failed/partial` 时，文件仍然可浏览、可搜索原文、可继续重试翻译
- 前端明确展示“已解析，翻译失败/部分完成，可重试”

## 解析分片规则

### PDF

- 优先按真实页码切分
- 每页一个基础 chunk
- 如果单页文本过长，可在页内按段落细切，但必须保留原始 `page_number`

### DOCX / TXT / Markdown / OCR 长文

- 按标题、段落、字符数做逻辑分段
- 生成 `segment_number`
- 不伪造真实页码

### XLSX

- 按 `sheet + block` 切分
- block 至少可映射回 sheet 名和行列范围

### 音频转写

- 按时间段切分
- 每段保留开始/结束时间锚点

### 切分原则

- 优先保持语义完整，不在句子中间硬切
- 单 chunk 原文目标长度控制在稳定翻译范围内
- 初版建议原文字符数目标 `1500 - 3000`
- 超长 chunk 可继续细切，超短相邻块可合并

### `anchor_json` 最小 schema

`anchor_json` 不是自由结构，至少要满足前端跳转、高亮和引用回溯的最小需求。

通用最小字段：

- `locator_type`
- `display_label`
- `start_offset`
- `end_offset`

按文件类型补充：

- `PDF`
  - `locator_type = "pdf_page"`
  - 必需：`page_number`
  - 可选：`bbox_list`、`line_range`
- `DOCX / TXT / OCR 长文`
  - `locator_type = "text_block"`
  - 必需：`segment_number`
  - 可选：`heading_path`、`paragraph_range`
- `XLSX`
  - `locator_type = "sheet_block"`
  - 必需：`sheet_name`、`cell_range`
- `音频转写`
  - `locator_type = "transcript_segment"`
  - 必需：`start_ms`、`end_ms`

前端搜索命中跳转和阅读高亮，最低依赖以下字段：

- `chunk_index`
- `page_number` 或 `segment_number`
- `display_label`
- `start_offset`
- `end_offset`

偏移语义约定：

- `start_offset / end_offset` 均表示相对 `source_text` 的字符偏移
- 前端高亮以 `source_text` 为准
- 中文阅读视图的高亮以 chunk 级命中联动为主，V1 不要求原文和译文逐字符对齐

## 翻译执行策略

### 执行方式

- 文件解析完成后，自动批量创建 chunk 翻译任务
- 翻译在后台异步执行，不阻塞文件可见性
- 每个 chunk 独立记录翻译结果、错误和重试次数
- 文件级翻译状态由 chunk 状态聚合

### 提供方抽象

后端引入统一的 `TranslationProvider` 抽象，通过配置切换不同提供方。

配置最小集合：

- `provider`
- `base_url`
- `api_key`
- `model`
- `target_language`
- `max_concurrency`
- `chunk_size_limit`

测试环境优先走云端模型；私有化交付时可将 `base_url` 切换到本地部署模型服务。

V1 约束：

- `target_language` 在 V1 固定为 `zh-CN`
- 字段保留为后续多目标语言扩展能力，但本轮不做多目标语言翻译

### 提示词语义

翻译不是摘要。模型任务定义为：

- 如原文已是中文，则输出规范中文，不做无端删改
- 如原文不是中文，则翻译为准确法律中文
- 尽量保留专有名词、人名、地名、机构名、法条名的可追溯性
- 不允许擅自总结、删减、合并或改变证据事实
- 原文已是中文时，允许输出规范中文；V1 不提供关闭该行为的前端开关

### 失败与重试

- 单 chunk 翻译失败不影响整份文件基础可用
- chunk 支持自动重试与手动重试
- 文件可进入 `translation=partial`
- 双语阅读区对失败 chunk 保留原文，并显示“翻译失败，可重试”

重试与幂等规则：

- 自动重试上限：`3` 次
- 退避策略：指数退避，建议 `1m / 5m / 30m`
- 超过自动重试上限后，chunk 进入终态 `failed`
- 手动重试会重置该 chunk 的自动重试计数
- 幂等键：`(evidence_id, chunk_index, translation_provider, translation_model, source_text_hash)`
- 如同一幂等键已成功写入，不得重复落库覆盖

文件级 `translation_status` 聚合规则：

- 只要存在 `pending` 或 `processing` chunk，文件级状态为 `processing`
- 所有 chunk 为 `done`，文件级状态为 `done`
- 所有 chunk 都进入终态且 `done = 0`，文件级状态为 `failed`
- 所有 chunk 都进入终态且 `done > 0` 且存在失败 chunk，文件级状态为 `partial`

`source_language` 来源规则：

- 以 chunk 为粒度记录
- 初版由翻译模型返回或推断
- 文件级 `source_language` 取主要语言汇总值，不要求覆盖所有混合语言片段

### 分片上限单位与优先级

- V1 的 `chunk_size_limit` 单位定义为“字符数”
- 解析分片规则中的 `1500 - 3000` 字符目标，必须服从 `chunk_size_limit`
- `token_estimate` 仅用于观测与后续优化，不作为 V1 的硬切分单位
- 如 provider 运行时返回明确的上下文超限错误，翻译 worker 可对超限 chunk 做一次更细粒度二次切分，但不改变解析阶段的主分片规则

## 搜索设计

搜索不再只依赖 `evidence_files.parsed_text`，而应基于 chunk 建索引，并支持三种语言模式：

- `zh`
- `source`
- `bilingual`

默认模式是 `zh`。

搜索结果必须带回：

- 文件名
- 页码或段号
- 命中文本摘要
- 命中来源语言
- chunk 锚点

这样前端可以直接跳转到对应 chunk 并高亮。

### 索引与迁移切换策略

- 新解析文件：一律以 chunk 为真源建立搜索索引
- 建议新增两套 chunk 级 FTS：
  - `search_evidence_chunks_source`
  - `search_evidence_chunks_translated`
- 现有基于 `evidence_files.parsed_text` 的搜索索引短期保留，只用于旧文件兼容回退

V1 搜索实现前提：

- 继续沿用 SQLite FTS
- chunk 入库或 chunk 翻译完成时增量更新对应 FTS 索引
- 旧索引并存，直到文件完成重解析并具备 chunk 索引后再切换查询优先级

### 旧文件回退策略

老文件如尚未重解析、没有 chunk 或没有译文，默认 `zh` 搜索行为为：

- 优先命中 chunk 中文索引
- 若文件没有 chunk 中文索引，则退回旧 `parsed_text` 原文索引
- 回退结果必须打上 `原文回退` 标记，避免用户误以为这是中文结果

这意味着：

- 新文件走 chunk 搜索真源
- 老文件在完成重解析前允许临时回退
- 一旦文件生成 chunk 数据，搜索优先级切换到 chunk 索引，不再优先依赖 `parsed_text`

## AI 抽取设计

时间线抽取、人物抽取、证据整理、摘要生成等后续 AI 能力，长期目标是不再直接吃整份全文，而是基于 chunk 聚合执行。

本轮范围边界：

- 本轮必须把 `source_text + translated_text` 的数据与接口准备好
- 本轮不要求同步重写所有现有 AI 抽取链路
- 现有 AI 抽取功能可以继续沿用旧链路，但新数据模型和接口必须为后续切换做好准备

默认策略：

1. 针对 chunk 做局部抽取
2. 将局部抽取汇总为文件级或案件级结论
3. 给模型提供 `source_text + translated_text`

这样既提高定位能力，也降低整份长文直接进入模型的成本和失败率。

## 前端文件阅读设计

### 默认视图

文件阅读区默认展示 `双语对照`，并允许切换：

- 双语
- 中文
- 原文

### 页面结构

顶部工具条包含：

- 视图切换：双语 / 中文 / 原文
- 搜索模式：中文 / 原文 / 双语
- 状态标签：解析中 / 已解析 / 翻译中 / 部分完成 / 翻译失败
- 操作入口：重试翻译、下载原文件、下载解析结果

阅读区包含：

- 左侧分页/段落导航
- 主阅读区
- 命中高亮

规则：

- PDF 使用真实页码导航
- 非 PDF 使用逻辑段分页导航
- 双语模式下左右对照
- 中文/原文模式下单列阅读

### 失败表现

如文件已解析但翻译部分失败：

- 阅读区仍然可进入
- 成功 chunk 正常显示双语
- 失败 chunk 保留原文
- 中文区域显示“翻译失败，可重试”

## API 设计

### 兼容性原则

现有文件列表和文件详情接口保留，不做破坏性删除；在此基础上扩展新字段。

### 文件详情增强

文件详情新增：

- `translation_status`
- `translation_error`
- `source_language`
- `target_language`
- `chunk_count`
- `translated_chunk_count`

### 新增接口

- `GET /api/v1/files/:id/chunks`
  - 读取文件 chunks
  - 查询参数：
    - `page`
    - `page_size`
    - `view_mode = bilingual | zh | source`
  - 排序：固定按 `chunk_index ASC`
  - 响应项最少包含：
    - `id`
    - `chunk_index`
    - `page_number`
    - `segment_number`
    - `chunk_kind`
    - `display_label`
    - `source_text`
    - `translated_text`
    - `translation_status`
    - `translation_error`
    - `anchor_json`
- `POST /api/v1/files/:id/translate/retry`
  - 请求体：
    - `scope = failed | all | selected`
    - `chunk_ids`，仅当 `scope = selected` 时必填
  - 行为：
    - `failed`：只重试失败 chunk
    - `all`：重试整份文件全部 chunk
    - `selected`：只重试给定 chunk
- `GET /api/v1/files/:id/translation`
  - 返回：
    - `translation_status`
    - `translation_error`
    - `chunk_count`
    - `translated_chunk_count`
    - `failed_chunk_count`
    - `source_language`
    - `target_language`
    - `translation_provider`
    - `translation_model`

### 搜索扩展

搜索接口增加 `language_mode` 参数：

- `zh`
- `source`
- `bilingual`

默认值：`zh`

前后端枚举对齐规则：

- 阅读视图使用 `view_mode = bilingual | zh | source`
- 搜索模式使用 `language_mode = bilingual | zh | source`
- 两者枚举值保持一致，避免前端做多套映射

建议错误码范围：

- `404`：文件不存在
- `403`：无权限访问
- `409`：文件尚未完成解析，chunks 不可读
- `422`：请求参数非法，例如 `scope=selected` 但未提供 `chunk_ids`

## 数据迁移与兼容策略

迁移分两步：

1. 给 `evidence_files` 增加文件级翻译字段
2. 新建 `evidence_file_chunks` 表及必要索引

兼容要求：

- 老文件即使没有 chunk 数据，也不应导致系统不可用
- 重新解析文件时，应自动生成新的 chunk 真源
- `parsed_text` 和原解析 artifact 继续保留，用于旧接口兼容与全文导出
- 老文件无 chunk 时，文件阅读区应退回“整份原文阅读”降级态，并明确提示“该文件尚未完成双语分片，请重新解析以启用双语对照与精准定位”

本轮不要求强制对全部历史文件做一次全量重建。

## 测试范围

### 1. 解析分片

- PDF 生成真实页码 chunk
- TXT / DOCX 生成逻辑段 chunk
- chunk 顺序、计数、锚点正确

### 2. 自动翻译

- 解析完成后自动触发翻译
- chunk 级翻译结果正确落库
- 文件级 `translation_status` 正确聚合

### 3. 失败语义

- 解析成功、翻译失败时文件仍可用
- 失败 chunk 可重试
- 文件可进入 `partial`

### 4. 搜索

- 默认搜中文
- 可切原文与双语
- 命中结果可定位到页/段

### 5. 前端文件页

- 默认双语对照
- 可切中文/原文
- 失败 chunk 有明确占位与重试入口

## 风险与约束

### 风险 1：大文件导致翻译成本和时延放大

必须通过解析阶段分片和并发控制来收住，不能走整份全文一次性翻译。

### 风险 2：旧搜索链仍依赖 `parsed_text`

实现时需要明确把搜索真源迁到 chunk 索引，否则会出现中文搜索与阅读定位不一致的问题。

### 风险 3：前端文件页需要从“整份文本”思维切到“按 chunk 阅读”

这会影响文件页状态、跳转、高亮和下载入口，但这是必要的结构升级。

### 风险 4：模型输出可能混入摘要式改写

翻译 prompt 和结果校验需要明确约束“翻译/规范化，不得擅自总结证据事实”。

### 风险 5：翻译调用的日志与审计可能泄露敏感文本

实现时必须避免把 chunk 原文、译文和完整模型请求体直接打进常规日志。

最小约束：

- 操作日志只记录文件级与任务级元数据，不记录 chunk 全文
- provider 调用审计只记录：
  - `evidence_id`
  - `chunk_index`
  - `provider`
  - `model`
  - `status`
  - `latency_ms`
  - `request_id`
- 失败原因按类别记录，例如：
  - `timeout`
  - `rate_limited`
  - `provider_error`
  - `validation_error`
  - `permanent_failure`

## 验收标准

功能完成后，至少满足以下条件：

1. 上传外文或混合语言大文件后，解析完成会自动启动翻译
2. 文件详情可同时看到解析状态与翻译状态
3. 文件阅读区默认双语对照，可切中文和原文
4. 搜索默认中文，可切原文和双语
5. AI 抽取链默认可获得原文与中文
6. 翻译失败不会让文件不可用
7. chunk 支持定位、引用、命中跳转和重试
