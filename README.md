# LegalGenie Agent

[中文](#中文) | [English](#english)

## 中文

LegalGenie Agent 是一个面向律师、法务与争议团队的开源法律工作台。项目包含 Rust 后端与基于 `Vite + Bun + Tailwind CSS + React` 的现代 Web 前端。

### 核心能力

- 案件管理：案件创建、成员协作、基础权限控制
- 证据处理：文件上传、异步解析、双语证据块翻译
- 时间轴工作台：案件事实链整理、节点管理、证据关联
- 人物与关系：案件内人物管理、去重建议、关系维护
- 审计与导出：操作日志、证据清单导出、时间轴导出
- 开源 Web 前端：可直接连接现有 API 的多页面工作台

### 技术栈

- 后端：Axum + Tokio + SQLx + SQLite
- 共享模型：Rust workspace
- Web 前端：Vite + React + Tailwind CSS + Bun

### 快速开始

1. 复制环境变量：

```bash
cp .env.example .env
```

2. 启动后端：

```bash
SERVER_PORT=8001 cargo run -p legalminds-server
```

3. 启动开源 Web 前端：

```bash
cd web
bun install
bun dev
```

默认情况下，`web/` 会连接 `http://127.0.0.1:8001/api/v1`。如果需要自定义地址，请设置：

```bash
VITE_API_BASE_URL=http://127.0.0.1:8001/api/v1
```

### 已接通的 Web API

- `GET /api/v1/health`
- `POST /api/v1/auth/login`
- `GET /api/v1/cases`

更多本地运行与双语翻译说明见 [docs/RUNBOOK_LOCAL.md](docs/RUNBOOK_LOCAL.md)。

## English

LegalGenie Agent is an open-source legal workspace for law firms, in-house legal teams, and dispute-resolution workflows. The repository includes a Rust backend and a modern Web frontend built with `Vite + Bun + Tailwind CSS + React`.

### Highlights

- Matter management with collaboration and access control
- Evidence upload, parsing, and bilingual chunk translation
- Timeline workspace for fact assembly and evidence linking
- Person and relationship management inside each case
- Audit logs and export workflows
- A standalone open-source Web frontend wired to the existing API

### Stack

- Backend: Axum + Tokio + SQLx + SQLite
- Shared models: Rust workspace crates
- Web frontend: Vite + React + Tailwind CSS + Bun

### Quick Start

1. Copy environment variables:

```bash
cp .env.example .env
```

2. Start the backend:

```bash
SERVER_PORT=8001 cargo run -p legalminds-server
```

3. Start the Web frontend:

```bash
cd web
bun install
bun dev
```

By default, `web/` connects to `http://127.0.0.1:8001/api/v1`. Override it with:

```bash
VITE_API_BASE_URL=http://127.0.0.1:8001/api/v1
```

### Connected Web API Routes

- `GET /api/v1/health`
- `POST /api/v1/auth/login`
- `GET /api/v1/cases`

For local runbooks and bilingual translation configuration, see [docs/RUNBOOK_LOCAL.md](docs/RUNBOOK_LOCAL.md).
