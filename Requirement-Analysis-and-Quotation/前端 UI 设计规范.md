# 前端 UI 设计规范

**文档版本**：v1.0  
**编写日期**：2026 年 3 月 14 日  
**开发负责人**：王舟  

---

## 一、设计原则

### 1.1 设计理念

| 原则 | 说明 |
|------|------|
| 专业 | 法律科技产品，稳重大气 |
| 简洁 | 减少视觉干扰，聚焦核心功能 |
| 高效 | 减少操作步骤，快速完成任务 |
| 一致 | 统一的视觉语言和交互模式 |

### 1.2 设计系统

```
LegalMinds Design System
├── 色彩系统
├── 字体系统
├── 组件库
├── 图标系统
└── 布局规范
```

---

## 二、色彩系统

### 2.1 主色调

| 颜色 | 色值 | 用途 |
|------|------|------|
| Primary | `#1a365d` | 主按钮、链接、选中状态 |
| Primary Light | `#2c5282` | 悬停状态、次要元素 |
| Primary Dark | `#0f2744` | 按下状态、强调 |

### 2.2 辅助色

| 颜色 | 色值 | 用途 |
|------|------|------|
| Success | `#38a169` | 成功提示、完成状态 |
| Warning | `#d69e2e` | 警告提示、注意 |
| Danger | `#e53e3e` | 错误提示、删除操作 |
| Info | `#3182ce` | 信息提示、帮助 |

### 2.3 中性色

| 颜色 | 色值 | 用途 |
|------|------|------|
| Text Primary | `#1a202c` | 主标题、正文 |
| Text Secondary | `#4a5568` | 次要文本、描述 |
| Text Disabled | `#a0aec0` | 禁用文本 |
| Border | `#e2e8f0` | 边框、分割线 |
| Background | `#f7fafc` | 页面背景 |
| Surface | `#ffffff` | 卡片、弹窗背景 |

### 2.4 功能色

| 颜色 | 色值 | 用途 |
|------|------|------|
| 原告 | `#ef4444` | 红色系 |
| 被告 | `#3b82f6` | 蓝色系 |
| 证人 | `#10b981` | 绿色系 |
| 律师 | `#8b5cf6` | 紫色系 |
| 其他 | `#6b7280` | 灰色系 |

---

## 三、字体系统

### 3.1 字体栈

```css
/* 中文字体 */
font-family: "Noto Sans SC", "PingFang SC", "Microsoft YaHei", sans-serif;

/* 英文字体 */
font-family: "Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;

/* 代码字体 */
font-family: "Fira Code", "Cascadia Code", "Consolas", monospace;
```

### 3.2 字号规范

| 级别 | 字号 | 行高 | 用途 |
|------|------|------|------|
| H1 | 32px | 40px | 页面标题 |
| H2 | 24px | 32px | 模块标题 |
| H3 | 20px | 28px | 分组标题 |
| H4 | 16px | 24px | 小标题 |
| Body | 14px | 20px | 正文 |
| Small | 12px | 16px | 辅助文字 |
| Caption | 11px | 14px | 标注、提示 |

### 3.3 字重

| 字重 | 值 | 用途 |
|------|-----|------|
| Regular | 400 | 正文 |
| Medium | 500 | 强调文本 |
| Semibold | 600 | 标题、按钮 |
| Bold | 700 | 重要标题 |

---

## 四、组件规范

### 4.1 按钮

```rust
// frontend/src/components/Button.rs

#[derive(Props, Clone, PartialEq)]
pub struct ButtonProps {
    variant: ButtonVariant,  // primary/secondary/ghost/danger
    size: ButtonSize,        // sm/md/lg
    disabled: bool,
    loading: bool,
    onclick: EventHandler<MouseEvent>,
    children: Element,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ButtonVariant {
    Primary,    // 主按钮
    Secondary,  // 次按钮
    Ghost,      // 幽灵按钮
    Danger,     // 危险按钮
}

#[derive(Clone, Copy, PartialEq)]
pub enum ButtonSize {
    Sm,  // 32px
    Md,  // 40px
    Lg,  // 48px
}

// 使用示例
rsx! {
    Button { variant: ButtonVariant::Primary, size: ButtonSize::Md,
        "确认"
    }
    
    Button { variant: ButtonVariant::Secondary, disabled: true,
        "取消"
    }
    
    Button { variant: ButtonVariant::Danger, loading: true,
        "删除"
    }
}
```

### 4.2 表单输入

```rust
// frontend/src/components/Input.rs

#[derive(Props, Clone, PartialEq)]
pub struct InputProps {
    r#type: InputType,       // text/password/email/number/date
    value: String,
    placeholder: String,
    label: Option<String>,
    error: Option<String>,
    disabled: bool,
    readonly: bool,
    required: bool,
    oninput: EventHandler<Event>,
}

// 使用示例
rsx! {
    Input {
        r#type: InputType::Text,
        label: "案件名称",
        placeholder: "请输入案件名称",
        value: case_name(),
        oninput: move |e| case_name.set(e.value()),
        error: if case_name().is_empty() { Some("案件名称不能为空") } else { None },
    }
}
```

### 4.3 卡片

```rust
// frontend/src/components/Card.rs

#[derive(Props, Clone, PartialEq)]
pub struct CardProps {
    variant: CardVariant,  // default/elevated/outlined
    padding: bool,
    children: Element,
}

#[derive(Clone, Copy, PartialEq)]
pub enum CardVariant {
    Default,    // 默认
    Elevated,   // 带阴影
    Outlined,   // 带边框
}

// 使用示例
rsx! {
    Card { variant: CardVariant::Elevated, padding: true,
        h3 { "案件信息" }
        p { "案件描述内容..." }
    }
}
```

### 4.4 表格

```rust
// frontend/src/components/Table.rs

#[derive(Props, Clone, PartialEq)]
pub struct TableProps {
    columns: Vec<Column>,
    data: Vec<RowData>,
    sortable: bool,
    pagination: Option<PaginationProps>,
    on_row_click: EventHandler<usize>,
}

// 使用示例
rsx! {
    Table {
        columns: vec![
            Column { header: "案件名称", field: "name", sortable: true },
            Column { header: "创建时间", field: "created_at", sortable: true },
            Column { header: "操作", field: "actions" },
        ],
        data: cases(),
        sortable: true,
        pagination: Some(PaginationProps { current: 1, total: 10 }),
        on_row_click: move |idx| view_case(idx),
    }
}
```

### 4.5 弹窗

```rust
// frontend/src/components/Modal.rs

#[derive(Props, Clone, PartialEq)]
pub struct ModalProps {
    open: bool,
    title: String,
    size: ModalSize,  // sm/md/lg/full
    on_close: EventHandler<()>,
    children: Element,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ModalSize {
    Sm,  // 400px
    Md,  // 600px
    Lg,  // 800px
    Full, // 100%
}

// 使用示例
rsx! {
    Modal {
        open: show_modal(),
        title: "确认删除",
        size: ModalSize::Md,
        on_close: move |_| show_modal.set(false),
        
        div { class: "modal-content",
            p { "确定要删除这个案件吗？" }
        }
        
        div { class: "modal-actions",
            Button { variant: Secondary, onclick: move |_| show_modal.set(false), "取消" }
            Button { variant: Danger, onclick: handle_delete, "删除" }
        }
    }
}
```

### 4.6 Toast 提示

```rust
// frontend/src/components/Toast.rs

#[derive(Clone, Copy, PartialEq)]
pub enum ToastType {
    Success,
    Info,
    Warning,
    Error,
}

// 使用示例
toast::success("操作成功", "案件已创建");
toast::error("操作失败", "文件上传失败，请检查网络");
toast::warning("注意", "该操作不可撤销");
toast::info("提示", "系统将在 5 分钟后维护");
```

---

## 五、布局规范

### 5.1 页面布局

```
┌─────────────────────────────────────────────────────────────┐
│                      Header (64px)                           │
│  Logo | 导航 | 搜索 | 用户菜单                                │
├─────────────────────────────────────────────────────────────┤
│        │                                                    │
│ Sidebar │                 Main Content                      │
│ (240px) │                                                    │
│        │                                                    │
│ 案件列表│  案件详情/时间轴/证据...                            │
│        │                                                    │
└────────┴────────────────────────────────────────────────────┘
```

### 5.2 间距规范

| 间距 | 值 | 用途 |
|------|-----|------|
| xs | 4px | 紧凑元素间距 |
| sm | 8px | 小组件间距 |
| md | 16px | 常规间距 |
| lg | 24px | 大间距 |
| xl | 32px | 模块间距 |
| 2xl | 48px | 页面间距 |

### 5.3 断点规范

| 断点 | 宽度 | 用途 |
|------|------|------|
| sm | 640px | 手机横屏 |
| md | 768px | 平板 |
| lg | 1024px | 小屏电脑 |
| xl | 1280px | 标准屏幕 |
| 2xl | 1536px | 大屏 |

---

## 六、图标系统

### 6.1 图标库

```toml
# Cargo.toml
[dependencies]
dioxus-material-icons = "0.3"
```

### 6.2 常用图标

| 用途 | 图标 | 组件 |
|------|------|------|
| 首页 | 🏠 | `icon::Home` |
| 案件 | 📁 | `icon::Folder` |
| 时间轴 | 📅 | `icon::Timeline` |
| 证据 | 📄 | `icon::Description` |
| 人物 | 👤 | `icon::Person` |
| 搜索 | 🔍 | `icon::Search` |
| 设置 | ⚙️ | `icon::Settings` |
| 删除 | 🗑️ | `icon::Delete` |
| 编辑 | ✏️ | `icon::Edit` |
| 查看 | 👁️ | `icon::Visibility` |

---

## 七、交互规范

### 7.1 加载状态

```rust
// 骨架屏
rsx! {
    Skeleton { width: "100%", height: "20px" }
    Skeleton { width: "80%", height: "20px" }
    Skeleton { width: "60%", height: "20px" }
}

// 加载 Spinner
rsx! {
    Spinner { size: "md", color: "primary" }
}
```

### 7.2 空状态

```rust
rsx! {
    EmptyState {
        icon: icon::FolderOpen,
        title: "暂无案件",
        description: "点击"新建案件"按钮创建第一个案件",
        action: rsx! {
            Button { onclick: create_case, "新建案件" }
        },
    }
}
```

### 7.3 错误状态

```rust
rsx! {
    ErrorState {
        icon: icon::ErrorOutline,
        title: "加载失败",
        description: "请检查网络连接后重试",
        action: rsx! {
            Button { onclick: retry, "重试" }
        },
    }
}
```

---

## 八、响应式设计

### 8.1 移动端适配

```css
/* 移动端隐藏侧边栏，使用底部导航 */
@media (max-width: 768px) {
    .sidebar { display: none; }
    .bottom-nav { display: block; }
    .main-content { margin-left: 0; }
}
```

### 8.2 表格响应式

```rust
// 移动端表格转为卡片
rsx! {
    if is_mobile {
        CardList { data: cases() }
    } else {
        Table { data: cases() }
    }
}
```

---

## 九、主题切换

### 9.1 暗色模式

```rust
// frontend/src/theme.rs

#[derive(Clone, Copy, PartialEq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    pub fn colors(&self) -> ThemeColors {
        match self {
            Theme::Light => ThemeColors {
                background: "#f7fafc",
                surface: "#ffffff",
                text_primary: "#1a202c",
                text_secondary: "#4a5568",
                // ...
            },
            Theme::Dark => ThemeColors {
                background: "#1a202c",
                surface: "#2d3748",
                text_primary: "#f7fafc",
                text_secondary: "#cbd5e0",
                // ...
            },
        }
    }
}
```

---

## 十、总结

### 10.1 设计资源

| 资源 | 链接/位置 |
|------|-----------|
| Figma 设计稿 | [待创建] |
| 图标库 | dioxus-material-icons |
| 色彩规范 | 本章第二节 |
| 组件库 | frontend/src/components |

### 10.2 开发检查清单

| 检查项 | 状态 |
|--------|------|
| 使用统一色彩变量 | ⏳ |
| 使用统一字体规范 | ⏳ |
| 组件使用设计系统 | ⏳ |
| 响应式适配完成 | ⏳ |
| 暗色模式支持 | ⏳ |

---

**文档版本**：v1.0  
**最后更新**：2026 年 3 月 14 日  
**开发负责人**：王舟  
**邮箱**：main@mails.wedevs.org  
**电话**：15378391447
