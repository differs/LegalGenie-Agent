# Dioxus 0.7.3 双端升级设计规格

## 状态

- 已确认
- 本规格只覆盖前端框架升级，不改变既有产品交互定稿

## 背景

当前项目前端仍停留在 Dioxus 0.5.x：

- workspace 依赖为 `dioxus = 0.5`、`dioxus-web = 0.5`、`dioxus-desktop = 0.5`
- web 入口仍使用 `dioxus_web::launch::launch_cfg(...)`
- desktop 入口仍使用 `dioxus_desktop::launch::launch(...)`

当前机器上的 `dx` 已是 `0.7.3`。继续维持项目在 `0.5.x` 会带来两个直接问题：

1. 本地 `dx serve` 与项目版本不兼容，正常开发链路不可用
2. `dx 0.5.x` 与当前构建出来的 `wasm-bindgen` 版本不再稳定匹配，预览链路脆弱

因此本轮目标不是做“前端重构”，而是把项目前端升级到与本机 `dx 0.7.3` 对齐的 Dioxus 0.7.3，并保持 `web + desktop` 两端都可运行。

## 目标

把 LegalMinds 前端从 Dioxus 0.5.x 升级到 Dioxus 0.7.3，并满足以下结果：

1. `dx 0.7.3` 可以启动 web 开发链路
2. desktop 端仍可构建并启动
3. 现有前端交互语义不回退
4. 现有前端测试继续通过
5. wasm 构建继续通过

## 非目标

- 不在这一轮重做聊天工作台交互
- 不顺手重写页面组织结构
- 不修改后端 API 契约
- 不引入 router、fullstack 或新状态管理方案
- 不把单纯依赖升级扩展成“全面前端现代化重构”

## 约束

### 1. 双端必须一起保住

本次升级不能只让 web 可用。成功标准是：

- `dx serve` 可用
- `cargo run -p legalminds-frontend` 或等价 desktop 启动链保持可用

### 2. 交互语义保持稳定

现有已落地的交互不允许因升级而回退，包括但不限于：

- `/auth/me` 验证闸门
- 聊天主轴工作台
- Canvas 工作层
- 显式“带入对话”上下文链路
- 右侧 `Plan / References / Results` 单面板

### 3. 升级优先于美化

本轮允许的代码调整，只限于为通过 Dioxus 0.7.3 编译和运行所必需的适配。任何“顺手更优雅”的改写，只有在明显降低迁移风险时才允许进入。

## 迁移依据

本次升级遵循 Dioxus 官方迁移文档，而不是靠试错推进：

- 0.5 -> 0.6：`https://dioxuslabs.com/learn/0.7/migration/to_06`
- 0.6 -> 0.7：`https://dioxuslabs.com/learn/0.7/migration/to_07/`

这两份官方文档明确指出，本项目会直接受到以下破坏性变更影响：

- `Element` 从可空语义变为结果语义，组件不能直接返回 `None`
- `launch` 启动 API 发生变化
- `derive(Props)` 与 prelude 暴露项收紧
- 0.7 的表单默认行为改变，提交不再默认阻止
- 传递依赖升级，尤其是 desktop 侧的 `wry` 等依赖

## 设计原则

### 1. 先兼容，再谈收敛

先把项目完整迁到 0.7.3 并恢复 `web + desktop + tests + wasm build`，再考虑是否需要第二轮代码清理。

### 2. 不跨越多个重构目标

升级任务只解决“版本兼容”和“开发链路恢复”，不混入新的 UI 改造目标。

### 3. 优先官方推荐写法

只要 Dioxus 0.7 官方文档对某类变更给出了明确替代方案，本次迁移优先采用官方路径，避免写临时兼容层。

### 4. 用编译错误驱动适配顺序

迁移顺序应当从依赖、启动入口、共有类型、组件返回类型、事件与表单，再到 desktop/web 差异化问题，而不是随机修。

## 影响面

### 1. 依赖层

需要统一升级的最小集合：

- workspace `dioxus`
- workspace `dioxus-web`
- workspace `dioxus-desktop`
- 前端 `wasm-bindgen` 及与之联动的 lockfile

本轮不主动新增无关第三方前端依赖。

### 2. 启动层

重点文件：

- [Cargo.toml](/home/de/works/LegalMinds/Cargo.toml)
- [frontend/Cargo.toml](/home/de/works/LegalMinds/frontend/Cargo.toml)
- [frontend/src/main.rs](/home/de/works/LegalMinds/frontend/src/main.rs)

这部分要完成：

- web 启动方式迁到 0.7.3 兼容 API
- desktop 启动方式迁到 0.7.3 兼容 API
- `dx serve` 所需的最小项目配置恢复正常

### 3. 组件与渲染层

本项目存在大量组件和资源加载分支，最可能受影响的是：

- 空渲染写法
- 回调签名与事件类型
- prelude 缩减后的显式导入
- 表单提交与默认行为

重点风险文件：

- [frontend/src/main.rs](/home/de/works/LegalMinds/frontend/src/main.rs)
- [frontend/src/pages/timeline.rs](/home/de/works/LegalMinds/frontend/src/pages/timeline.rs)
- [frontend/src/pages/files.rs](/home/de/works/LegalMinds/frontend/src/pages/files.rs)
- [frontend/src/pages/persons.rs](/home/de/works/LegalMinds/frontend/src/pages/persons.rs)
- [frontend/src/pages/search.rs](/home/de/works/LegalMinds/frontend/src/pages/search.rs)
- [frontend/src/pages/logs.rs](/home/de/works/LegalMinds/frontend/src/pages/logs.rs)
- [frontend/src/workspace/](/home/de/works/LegalMinds/frontend/src/workspace)

### 4. 构建与运行层

必须恢复并验证以下 4 条链：

1. `cargo test -p legalminds-frontend`
2. `cargo build -p legalminds-frontend --target wasm32-unknown-unknown`
3. `dx serve`
4. desktop 启动

## 升级策略

本次采用“最小兼容升级”策略，不做激进重构。

### 阶段 1：依赖统一到 0.7.3

先把 workspace 和 frontend 相关依赖统一升到 `0.7.3`，刷新 lockfile，让后续错误集中暴露。

预期：

- 这一步之后编译大概率会先失败
- 失败是预期结果，不视为回归

### 阶段 2：修启动入口

先修 [frontend/src/main.rs](/home/de/works/LegalMinds/frontend/src/main.rs) 的 web/desktop 启动 API，使项目恢复最外层可启动能力。

原因：

- 启动入口是所有其他问题的外层壳
- 不先修入口，就无法验证 `dx serve` 和 desktop 运行链

### 阶段 3：处理共有破坏性变更

按官方迁移文档处理三类共有破坏点：

1. `Element` / 空渲染
2. prelude 移除项的显式导入
3. 表单默认行为

其中表单默认行为是本项目的高风险点，因为登录、注册、筛选、编辑器和一系列输入区都依赖前端事件模型。

### 阶段 4：清理 desktop/web 侧差异

如果 0.7.3 引入了新的 desktop 传递依赖要求，或 web 端构建需要额外适配，在这一阶段收口。

要求：

- 不通过禁用 desktop 功能来“假装升级完成”
- 不允许只留 wasm 构建通过但 desktop 崩掉

## 重点风险

### 风险 1：表单行为回归

官方 0.7 迁移文档指出，表单默认提交行为发生变化。当前项目里大量输入控件和按钮组合都在无 router、无传统 form 管理的 Dioxus 组件里运行，最容易出现：

- 登录点击后页面刷新
- 输入区触发意外提交
- 工具筛选表单行为异常

这类问题需要浏览器级验证，不靠单元测试替代。

### 风险 2：组件空渲染语义变化

项目里已有很多 `Ok(None)`、条件空渲染和嵌套资源加载分支。0.6 迁移文档已经明确指出组件不能直接返回 `None`，这会集中影响列表页、工具页和时间轴大页中的条件渲染代码。

### 风险 3：desktop 传递依赖升级

0.7 迁移文档明确提到 desktop 侧传递依赖更新。即使 web 编译通过，也不能推断 desktop 一定可运行。

### 风险 4：把升级做成隐性重构

本项目前端已经在连续重构中。如果升级过程中开始顺手重排结构、批量改命名、重写事件流，风险会显著放大。

因此本轮要压住“重构冲动”，只改和 0.7.3 兼容直接相关的代码。

## 验收标准

升级完成后，必须满足以下全部条件：

### 1. 依赖与工具链

- `Cargo.toml` 和 `frontend/Cargo.toml` 中 Dioxus 相关依赖升级到 `0.7.3`
- 本机 `dx 0.7.3` 可直接用于该项目

### 2. 编译与测试

- `cargo test -p legalminds-frontend` 通过
- `cargo build -p legalminds-frontend --target wasm32-unknown-unknown` 通过

### 3. 运行验证

- `dx serve` 可以拉起 web 开发预览
- desktop 端可以启动，并且至少能稳定进入工作台主窗口而非启动即崩溃
- desktop 端必须完成一轮最小交互回归，不能只验证“能起窗”

### 4. 关键交互回归验证

web 端至少手工确认以下链路不回退：

- 登录后仍需 `/auth/me` 验证才能进入工作台
- 进入案件后仍是聊天主轴工作台
- 快捷入口可以打开 Canvas
- Canvas 打开时 composer 仍可用
- 显式“带入对话”仍然有效
- Search / Logs 仍能从次级工具面发现

desktop 端至少手工确认以下最小链路不回退：

- 应用启动后可以进入认证页或工作台，而不是空白窗口/崩溃
- 登录或会话恢复后可以进入聊天主轴工作台
- 快捷入口可以打开至少一个 Canvas
- Canvas 打开时 composer 仍然存在并可聚焦

### 5. 空渲染与空态 smoke 验证

必须至少覆盖一组“空数据/条件不满足”的 UI 分支，证明 0.6/0.7 迁移后的 `Element` / 空渲染适配没有只覆盖主路径。

最小 smoke 集合：

- 未登录或缺少 `case_id` 时，`Search` / `Logs` / `Files` / `Persons` 至少各有一个空态仍正常渲染
- 工作台在没有已插入上下文时，`References` 面板仍正常渲染空态
- 至少一个资源加载返回 `Ok(None)` 的旧分支经过人工或测试验证后，仍不会导致渲染异常

## 成功定义

本轮完成的标志不是“版本号变了”，而是：

- LegalMinds 前端已经真实运行在 Dioxus 0.7.3 上
- `dx serve` 与当前机器环境重新对齐
- web 和 desktop 双端都保住
- 既有聊天工作台交互没有因为升级而退化

## 后续工作边界

本轮完成后，可以再决定是否进入下一轮专项：

1. 清理 0.5/0.6 时代遗留写法
2. 继续收前端 warnings
3. 再做一轮 desktop 体验优化

这些都不属于本次升级验收范围。
