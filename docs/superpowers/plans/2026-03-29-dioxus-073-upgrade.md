# Dioxus 0.7.3 双端升级 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 LegalMinds 前端从 Dioxus 0.5.x 升级到 Dioxus 0.7.3，并恢复 `dx 0.7.3` 的 web 开发链路，同时保住 desktop 运行能力和现有聊天工作台交互。

**Architecture:** 先把前端 crate 切到 Dioxus 0.7 的特性模型与 CLI 配置，再迁移启动入口，随后按“共享壳层 -> 页面模块 -> 运行链”的顺序收编译与行为回归。验证同时覆盖现有前端测试、wasm 构建、desktop 构建、`dx serve`、以及 web/desktop 的最小手工 smoke。

**Tech Stack:** Rust workspace, Cargo, Dioxus 0.7.3, dx 0.7.3, wasm32-unknown-unknown, native desktop target, existing frontend tests in `frontend/src/shell.rs` and `frontend/src/workspace/view_model.rs`.

---

## File Map

- `Cargo.toml`
  统一 workspace 中 Dioxus 相关版本，移除 0.5.x 时代的 renderer 版本锁定。
- `frontend/Cargo.toml`
  把前端 crate 切换到 Dioxus 0.7 的 feature 驱动平台模型，定义 `web` / `desktop` feature。
- `Dioxus.toml`
  新增 Dioxus CLI 配置，让 `dx serve` 在 workspace 根目录下能直接找到 `legalminds-frontend`。
- `frontend/src/main.rs`
  迁移 web/desktop 启动入口到 0.7 API，并保留现有 desktop 窗口配置。
- `frontend/src/shell.rs`
  修复 Dioxus 0.7 下的共享状态与测试编译面；必要时补迁移回归测试。
- `frontend/src/workspace/frame.rs`
  修复壳层组件签名、事件与渲染兼容性。
- `frontend/src/workspace/conversation.rs`
  修复主对话组件在 0.7 下的事件和渲染兼容性。
- `frontend/src/workspace/canvas.rs`
  修复 Canvas 工作层的事件/渲染兼容性。
- `frontend/src/workspace/context_block.rs`
  修复上下文块组件兼容性。
- `frontend/src/workspace/view_model.rs`
  修复 view-model 编译面，并在必要时补空态/引用面板回归测试。
- `frontend/src/workspace/left_rail.rs`
  修复左栏组件签名与导入。
- `frontend/src/workspace/right_panel.rs`
  修复右侧面板组件签名与导入。
- `frontend/src/workspace/utilities.rs`
  修复工具面组件签名与导入。
- `frontend/src/workspace/mod.rs`
  保持 workspace 模块导出与 0.7 兼容。
- `frontend/src/pages/mod.rs`
  调整页面模块导出，消除与新平台模型冲突的旧引用。
- `frontend/src/pages/cases.rs`
  修复 cases 页面在 0.7 下的资源与空态渲染。
- `frontend/src/pages/files.rs`
  修复 files 页面在 0.7 下的资源与空态渲染。
- `frontend/src/pages/exports.rs`
  修复 exports 页面在 0.7 下的资源与空态渲染。
- `frontend/src/pages/logs.rs`
  修复 logs 页面在 0.7 下的资源与空态渲染。
- `frontend/src/pages/persons.rs`
  修复 persons 页面在 0.7 下的资源与空态渲染。
- `frontend/src/pages/search.rs`
  修复 search 页面在 0.7 下的资源与空态渲染。
- `frontend/src/pages/timeline.rs`
  修复 timeline 大页在 0.7 下的导入、事件和条件渲染兼容性。
- `.github/workflows/ci.yml`
  把 desktop 编译纳入 CI，避免再次只保住 web/wasm。
- `README.md`
  更新 Dioxus 0.7.3 对应的 web/desktop 启动命令。
- `docs/RUNBOOK_LOCAL.md`
  同步本地运行说明，反映 `dx 0.7.3` 和 desktop feature 命令。

## Task 1: 切到 Dioxus 0.7.3 的依赖与 CLI 配置模型

**Files:**
- Modify: `Cargo.toml`
- Modify: `frontend/Cargo.toml`
- Create: `Dioxus.toml`
- Modify: `Cargo.lock`

- [ ] **Step 1: 把 workspace Dioxus 版本升级到 0.7.3**

在 `Cargo.toml` 中把：

```toml
dioxus = { version = "0.5", features = ["macro"] }
dioxus-web = "0.5"
dioxus-desktop = "0.5"
```

改成 0.7.3 时代的统一版本基线。优先使用 `dioxus = "0.7.3"` 作为主依赖来源，不再维持 0.5.x 风格的 renderer 版本锁定；如果最终采用 `dioxus/web` 与 `dioxus/desktop` feature 方案，这里就直接删除 `dioxus-web` / `dioxus-desktop` 的 workspace 依赖定义。

- [ ] **Step 2: 在 `frontend/Cargo.toml` 引入平台 feature 布局**

把前端 crate 切成：

```toml
[features]
default = ["web"]
web = ["dioxus/web"]
desktop = ["dioxus/desktop"]
```

同时把 `frontend/Cargo.toml` 中旧的：

```toml
dioxus-web.workspace = true
dioxus-desktop.workspace = true
```

renderer 直接依赖移除，统一通过 `dioxus/web` 与 `dioxus/desktop` feature 打开平台能力。保留现有 wasm32 / 非 wasm32 的 target-specific 依赖分组，只有在 0.7 编译错误证明必须调整时，才改 `gloo-net` / `reqwest` / `tokio` 归属。

- [ ] **Step 3: 新增 `Dioxus.toml`**

创建根目录 `Dioxus.toml`，最小配置至少包含：

```toml
[application]
name = "legalminds"
sub_package = "legalminds-frontend"

[web.app]
title = "LegalMinds"
```

如果 `dx serve` 在 workspace 根目录下仍找不到包，再在同一文件中补必要的最小配置，不要一次性引入 bundle/asset 等无关项。

- [ ] **Step 4: 刷新 lockfile**

Run:

```bash
cargo update
```

Expected:

- `Cargo.lock` 更新到 Dioxus 0.7.3 依赖树
- 允许后续编译失败，但 lockfile 不应停留在 0.5.x renderer 栈

- [ ] **Step 5: 运行前端测试，确认进入“红灯”状态**

Run:

```bash
cargo test -p legalminds-frontend
```

Expected:

- 失败
- 失败原因应聚焦在 Dioxus 0.7 API/类型兼容，而不是 TOML 语法错误或 feature 名错误

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml frontend/Cargo.toml Dioxus.toml Cargo.lock
git commit -m "build: move frontend to dioxus 0.7.3 dependency model"
```

## Task 2: 迁移 web/desktop 启动入口到 0.7 API

**Files:**
- Modify: `frontend/src/main.rs`

- [ ] **Step 1: 先运行编译，定位启动入口错误**

Run:

```bash
cargo build -p legalminds-frontend --target wasm32-unknown-unknown
```

Expected:

- 失败
- 错误集中在 `dioxus_web::launch::launch_cfg`、`dioxus_desktop::launch::launch` 或相关 Config API

- [ ] **Step 2: 把 `main.rs` 迁到 0.7 的 LaunchBuilder / launch 模型**

按官方 0.6/0.7 迁移文档替换现有入口，目标结构是：

```rust
#[cfg(feature = "web")]
fn main() {
    dioxus::LaunchBuilder::web().launch(App);
}

#[cfg(feature = "desktop")]
fn main() {
    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_window(
                    dioxus::desktop::WindowBuilder::new()
                        .with_title("LegalMinds")
                        .with_inner_size(dioxus::desktop::LogicalSize::new(1280.0, 820.0)),
                ),
        )
        .launch(App);
}
```

如果 0.7.3 的最终 API 名与上面略有差异，以官方文档和编译器反馈为准；不要再保留旧的 `dioxus_web::launch::launch_cfg` / `dioxus_desktop::launch::launch` 路径。

- [ ] **Step 3: 验证 wasm 构建错误已从“启动入口”推进到更深层**

Run:

```bash
cargo build -p legalminds-frontend --target wasm32-unknown-unknown
```

Expected:

- 若仍失败，失败点不再是旧 launch API
- 错误进入组件/Element/事件层，说明入口已迁完

- [ ] **Step 4: 验证 desktop 编译链已接通到同一层**

Run:

```bash
cargo build -p legalminds-frontend --no-default-features --features desktop
```

Expected:

- 若失败，失败点进入共享组件层，而不是 platform feature 未定义或 desktop launch API 未迁完

- [ ] **Step 5: Commit**

```bash
git add frontend/src/main.rs
git commit -m "refactor: migrate frontend launchers to dioxus 0.7"
```

## Task 3: 修共享壳层与测试面，恢复 shell/workspace 编译

**Files:**
- Modify: `frontend/src/shell.rs`
- Modify: `frontend/src/workspace/frame.rs`
- Modify: `frontend/src/workspace/conversation.rs`
- Modify: `frontend/src/workspace/canvas.rs`
- Modify: `frontend/src/workspace/context_block.rs`
- Modify: `frontend/src/workspace/left_rail.rs`
- Modify: `frontend/src/workspace/right_panel.rs`
- Modify: `frontend/src/workspace/utilities.rs`
- Modify: `frontend/src/workspace/view_model.rs`
- Modify: `frontend/src/workspace/mod.rs`
- Modify: `frontend/src/pages/mod.rs`

- [ ] **Step 1: 运行共享测试面，确认当前失败**

Run:

```bash
cargo test -p legalminds-frontend shell::tests
cargo test -p legalminds-frontend workspace::view_model::tests
```

Expected:

- 失败
- 失败原因应集中在 `Element`、导入、事件签名、组件返回类型或 props 推导兼容性

- [ ] **Step 2: 修 0.7 下的 `Element` / 空渲染 / prelude 兼容性**

逐个处理 workspace 和 shell 相关文件中的破坏性变更：

- 不能直接返回 `None` 的组件语义
- prelude 不再自动暴露的类型或模块
- props / component 宏在 0.7 下的导入需求
- 事件与回调的签名收紧

要求：

- 不改变现有聊天工作台交互语义
- 不借迁移顺手重排组件结构

- [ ] **Step 3: 如果迁移需要低成本回归测试，就补在现有测试文件里**

允许补充的测试仅限于迁移回归，例如：

- `References` 面板在没有 inserted contexts 时仍给出空态文案
- workspace 在 `AuthGate::Ready` 前仍不会误进工作台

测试位置只放在已有测试载体中：

- `frontend/src/shell.rs`
- `frontend/src/workspace/view_model.rs`

- [ ] **Step 4: 重新运行共享测试面**

Run:

```bash
cargo test -p legalminds-frontend shell::tests
cargo test -p legalminds-frontend workspace::view_model::tests
```

Expected:

- 通过
- 共享壳层和 view-model 层已不再被 0.7 编译问题阻塞

- [ ] **Step 5: Commit**

```bash
git add frontend/src/shell.rs frontend/src/workspace frontend/src/pages/mod.rs
git commit -m "refactor: restore workspace shell under dioxus 0.7"
```

## Task 4: 收页面模块与空态/条件分支兼容

**Files:**
- Modify: `frontend/src/pages/cases.rs`
- Modify: `frontend/src/pages/files.rs`
- Modify: `frontend/src/pages/exports.rs`
- Modify: `frontend/src/pages/logs.rs`
- Modify: `frontend/src/pages/persons.rs`
- Modify: `frontend/src/pages/search.rs`
- Modify: `frontend/src/pages/timeline.rs`
- Modify: `frontend/src/api.rs`
- Modify: `frontend/src/models.rs`

- [ ] **Step 1: 运行完整前端测试，确认页面层仍是红灯**

Run:

```bash
cargo test -p legalminds-frontend
```

Expected:

- 若仍失败，问题应集中在页面模块、资源分支或 target-specific 编译面

- [ ] **Step 2: 逐页修 0.7 兼容问题**

按以下顺序处理，避免一开始就在 `timeline.rs` 里迷路：

1. `cases.rs`
2. `files.rs`
3. `exports.rs`
4. `logs.rs`
5. `persons.rs`
6. `search.rs`
7. `timeline.rs`

重点收口：

- `Ok(None)` / 空态资源分支
- 事件类型与闭包签名
- 需要显式 `rsx!` 包裹的条件渲染
- target-specific API 变更带来的 web/native 编译差异

- [ ] **Step 3: 把空渲染 smoke 需求映射到可验证对象**

至少保证下面这些分支在实现里被明确检查过：

- 未登录或缺少 `case_id` 时：
  - `Search`
  - `Logs`
  - `Files`
  - `Persons`
- 没有 inserted contexts 时：
  - `References`

如果无法低成本写自动测试，就在代码里加最小注释并把它们列入最终手工 smoke 清单，不要装作“自动化已经覆盖”。

- [ ] **Step 4: 重新运行完整前端测试**

Run:

```bash
cargo test -p legalminds-frontend
```

Expected:

- 通过
- 说明单元测试与页面编译面已恢复

- [ ] **Step 5: Commit**

```bash
git add frontend/src/pages frontend/src/api.rs frontend/src/models.rs
git commit -m "fix: adapt frontend pages to dioxus 0.7 rendering rules"
```

## Task 5: 恢复双端运行链，并同步文档与 CI

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`
- Modify: `docs/RUNBOOK_LOCAL.md`
- Modify: `docs/RUNBOOK_DOCKER.md` (only if command text or runtime assumptions are now stale)

- [ ] **Step 1: 补 desktop 编译进 CI**

在 `.github/workflows/ci.yml` 里保留现有 wasm 构建，并先补 Ubuntu/Linux desktop 所需系统依赖。按 Dioxus 0.7 官方 Getting Started 的 Ubuntu 依赖清单，新增一个安装步骤：

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  xdotool \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  lld
```

然后新增 desktop 编译步骤：

```bash
cargo build -p legalminds-frontend --no-default-features --features desktop
```

不要把 desktop 运行塞进 CI，只做编译覆盖。

- [ ] **Step 2: 更新 README 的启动命令**

把 desktop 命令改成 feature 模式，例如：

```bash
cargo run -p legalminds-frontend --no-default-features --features desktop
```

web 命令改成在 repo 根目录执行：

```bash
dx serve
```

前提是 `Dioxus.toml` 已正确指向 `legalminds-frontend`。

- [ ] **Step 3: 更新 `docs/RUNBOOK_LOCAL.md`**

同步 desktop/web 运行命令，并注明当前机器要求：

- `dx 0.7.3`
- `wasm32-unknown-unknown`

- [ ] **Step 4: 如果 Docker 运行说明受影响，再最小更新 `docs/RUNBOOK_DOCKER.md`**

仅当该文档提到了旧前端启动命令，才改它。不要做无关润色。

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml README.md docs/RUNBOOK_LOCAL.md docs/RUNBOOK_DOCKER.md
git commit -m "docs: update frontend runbooks for dioxus 0.7.3"
```

## Task 6: 全量验证与 web/desktop 手工 smoke

**Files:**
- No new files by default
- Modify only if verification暴露真实问题

- [ ] **Step 1: 准备 smoke 前置环境**

先把 backend 与 smoke 数据准备清楚，避免把环境问题误判成升级回归。

后端统一按前端默认 API base 跑在 `8001`：

```bash
SERVER_PORT=8001 cargo run -p legalminds-server
```

然后用一个独立终端准备最小 smoke 数据。使用标准库 `python3` 直接创建账号和案件，避免依赖 `jq`：

```bash
python3 - <<'PY'
import json
import time
import urllib.request

base = "http://127.0.0.1:8001"
username = f"smoke_{int(time.time())}"
password = "SmokePass123"
email = f"{username}@example.com"

def post(path, payload, token=None):
    body = json.dumps(payload).encode()
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(f"{base}{path}", data=body, headers=headers, method="POST")
    with urllib.request.urlopen(req) as resp:
        return json.load(resp)

reg = post("/api/v1/auth/register", {
    "username": username,
    "email": email,
    "password": password,
    "real_name": "Smoke User",
})
token = reg["data"]["access_token"]
case = post("/api/v1/cases", {
    "name": "Dioxus 0.7 Smoke Case",
    "description": "upgrade smoke",
}, token=token)
print("USERNAME=", username)
print("PASSWORD=", password)
print("CASE_ID=", case["data"]["id"])
PY
```

Expected:

- backend 运行在 `http://127.0.0.1:8001`
- 拿到一组可直接用于 web/desktop smoke 的 `USERNAME` / `PASSWORD` / `CASE_ID`

- [ ] **Step 2: 运行完整自动验证**

Run:

```bash
cargo fmt --all
cargo test -p legalminds-frontend
cargo build -p legalminds-frontend --target wasm32-unknown-unknown
cargo build -p legalminds-frontend --no-default-features --features desktop
```

Expected:

- 全部通过

- [ ] **Step 3: 验证 `dx serve`**

Run:

```bash
dx serve
```

Expected:

- web 开发预览成功拉起
- 不再出现 `dx 0.7.3` 与项目版本不兼容报错

- [ ] **Step 4: 做 web 端手工 smoke**

至少验证：

1. 用 Step 1 生成的 `USERNAME` / `PASSWORD` 登录
2. 登录后仍需 `/auth/me` 验证才能进工作台
3. 把 `CASE_ID` 粘进左栏 Case Switcher 后，能进入案件上下文
2. 进入案件后仍是聊天主轴工作台
3. 快捷入口可以打开 Canvas
4. Canvas 打开时 composer 仍可用
5. 显式“带入对话”仍然有效
6. `Search / Logs` 仍可从次级工具面发现
7. `Search / Logs / Files / Persons` 在缺少 token 或 `case_id` 时空态不炸
8. `References` 在没有 inserted contexts 时仍正常显示空态

- [ ] **Step 5: 做 desktop 端最小交互 smoke**

Run:

```bash
cargo run -p legalminds-frontend --no-default-features --features desktop
```

至少验证：

1. 能进入认证页或工作台，不是空白窗口
2. 用 Step 1 生成的 `USERNAME` / `PASSWORD` 登录，或粘贴相同会话 token 恢复会话
3. 把同一个 `CASE_ID` 粘进 Case Switcher 后能进入聊天主轴工作台
4. 能打开至少一个 Canvas
5. Canvas 打开时 composer 仍存在并可聚焦

- [ ] **Step 6: 如果 smoke 暴露问题，修复后重跑全部验证**

不要只重跑单个命令。任何运行时问题修复后，都回到：

```bash
cargo test -p legalminds-frontend
cargo build -p legalminds-frontend --target wasm32-unknown-unknown
cargo build -p legalminds-frontend --no-default-features --features desktop
```

然后还必须重跑：

```bash
dx serve
```

并重新完成：

- web 端手工 smoke
- desktop 端最小交互 smoke

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml frontend/Cargo.toml Dioxus.toml frontend/src .github/workflows/ci.yml README.md docs/RUNBOOK_LOCAL.md docs/RUNBOOK_DOCKER.md Cargo.lock
git commit -m "build: upgrade frontend to dioxus 0.7.3"
```
