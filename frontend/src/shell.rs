#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Brief,
    Cases,
    Timeline,
    Files,
    Persons,
    Search,
    Exports,
    Logs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthGate {
    SignedOut,
    Verifying,
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionRisk {
    Auto,
    ReviewRequired,
    Guarded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionIntent {
    OpenAuth,
    OpenCases,
    OpenBrief,
    OpenTimeline,
    OpenFiles,
    OpenPersons,
    OpenSearch,
    OpenExports,
    OpenLogs,
    ToggleDevtools,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversationStage {
    Empty,
    Active,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasKind {
    Timeline,
    Evidence,
    Persons,
    Exports,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryScope {
    CurrentCase,
    AllConversations,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RightPanelTab {
    Plan,
    References,
    Results,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelMode {
    Expanded,
    Collapsed,
    Pinned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    TimelineNode,
    Evidence,
    Person,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellBrief {
    pub eyebrow: String,
    pub title: String,
    pub summary: String,
    pub insights: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellAction {
    pub title: String,
    pub summary: String,
    pub cta: String,
    pub risk: ActionRisk,
    pub intent: ActionIntent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellState {
    pub signed_in: bool,
    pub has_case: bool,
    pub role_in_case: Option<String>,
    pub active_tab: Tab,
    pub username: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperatorPanel {
    pub title: String,
    pub subtitle: String,
    pub case_label: String,
    pub role_label: String,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InsertedContext {
    pub id: String,
    pub kind: ObjectKind,
    pub title: String,
    pub expanded: bool,
}

pub fn submit_local_conversation_message(
    conversation_items: &mut Vec<String>,
    draft: &mut String,
) -> bool {
    let message = draft.trim();
    if message.is_empty() {
        return false;
    }
    conversation_items.push(message.to_string());
    draft.clear();
    true
}

pub fn insert_selected_object(
    selected: &Option<SelectedObject>,
    inserted_contexts: &mut Vec<InsertedContext>,
) -> bool {
    let Some(selected) = selected else {
        return false;
    };
    if inserted_contexts.iter().any(|item| item.id == selected.id) {
        return false;
    }
    inserted_contexts.push(InsertedContext {
        id: selected.id.clone(),
        kind: selected.kind,
        title: selected.title.clone(),
        expanded: false,
    });
    true
}

pub fn toggle_inserted_context_expanded(
    inserted_contexts: &mut [InsertedContext],
    id: &str,
) -> bool {
    let Some(context) = inserted_contexts.iter_mut().find(|item| item.id == id) else {
        return false;
    };
    context.expanded = !context.expanded;
    true
}

pub fn remove_inserted_context(inserted_contexts: &mut Vec<InsertedContext>, id: &str) -> bool {
    let original_len = inserted_contexts.len();
    inserted_contexts.retain(|item| item.id != id);
    inserted_contexts.len() != original_len
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuggestedAction {
    pub title: String,
    pub risk: ActionRisk,
}

impl SuggestedAction {
    pub fn new(title: impl Into<String>, risk: ActionRisk) -> Self {
        Self {
            title: title.into(),
            risk,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RightPanelState {
    pub tab: RightPanelTab,
    pub mode: PanelMode,
}

impl Default for RightPanelState {
    fn default() -> Self {
        Self {
            tab: RightPanelTab::Plan,
            mode: PanelMode::Expanded,
        }
    }
}

pub fn default_right_panel_state() -> RightPanelState {
    RightPanelState::default()
}

pub fn switch_right_panel_tab(mut state: RightPanelState, tab: RightPanelTab) -> RightPanelState {
    state.tab = tab;
    if state.mode == PanelMode::Collapsed {
        state.mode = PanelMode::Expanded;
    }
    state
}

pub fn collapse_right_panel(mut state: RightPanelState) -> RightPanelState {
    if matches!(state.mode, PanelMode::Expanded | PanelMode::Pinned) {
        state.mode = PanelMode::Collapsed;
    }
    state
}

pub fn expand_right_panel(mut state: RightPanelState) -> RightPanelState {
    if state.mode == PanelMode::Collapsed {
        state.mode = PanelMode::Expanded;
    }
    state
}

pub fn toggle_right_panel_pin(mut state: RightPanelState) -> RightPanelState {
    state.mode = match state.mode {
        PanelMode::Pinned => PanelMode::Expanded,
        PanelMode::Expanded | PanelMode::Collapsed => PanelMode::Pinned,
    };
    state
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedObject {
    pub id: String,
    pub kind: ObjectKind,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceState {
    pub has_case: bool,
    pub conversation_stage: ConversationStage,
    pub canvas: Option<CanvasKind>,
    pub history_scope: HistoryScope,
    pub right_panel: RightPanelState,
    pub selected_object: Option<SelectedObject>,
    pub inserted_contexts: Vec<InsertedContext>,
    pub actions: Vec<SuggestedAction>,
}

impl WorkspaceState {
    pub fn new() -> Self {
        Self {
            has_case: false,
            conversation_stage: ConversationStage::Empty,
            canvas: None,
            history_scope: HistoryScope::CurrentCase,
            right_panel: RightPanelState::default(),
            selected_object: None,
            inserted_contexts: Vec::new(),
            actions: Vec::new(),
        }
    }

    pub fn with_case(mut self, has_case: bool) -> Self {
        self.has_case = has_case;
        self
    }

    pub fn with_history_scope(mut self, scope: HistoryScope) -> Self {
        self.history_scope = scope;
        self
    }

    pub fn with_canvas(mut self, canvas: Option<CanvasKind>) -> Self {
        self.canvas = canvas;
        self
    }

    pub fn after_user_submission(mut self) -> Self {
        self.conversation_stage = ConversationStage::Active;
        self
    }

    pub fn with_right_panel(mut self, tab: RightPanelTab, mode: PanelMode) -> Self {
        self.right_panel.tab = tab;
        self.right_panel.mode = mode;
        self
    }

    pub fn toggle_panel_collapsed(mut self) -> Self {
        self.right_panel = match self.right_panel.mode {
            PanelMode::Collapsed => expand_right_panel(self.right_panel),
            PanelMode::Expanded | PanelMode::Pinned => collapse_right_panel(self.right_panel),
        };
        self
    }

    pub fn switch_right_panel_tab(mut self, tab: RightPanelTab) -> Self {
        self.right_panel = switch_right_panel_tab(self.right_panel, tab);
        self
    }

    pub fn collapse_right_panel(mut self) -> Self {
        self.right_panel = collapse_right_panel(self.right_panel);
        self
    }

    pub fn expand_right_panel(mut self) -> Self {
        self.right_panel = expand_right_panel(self.right_panel);
        self
    }

    pub fn select_object(
        mut self,
        id: impl Into<String>,
        kind: ObjectKind,
        title: impl Into<String>,
    ) -> Self {
        self.selected_object = Some(SelectedObject {
            id: id.into(),
            kind,
            title: title.into(),
        });
        self
    }

    pub fn with_inserted_context(
        mut self,
        id: impl Into<String>,
        kind: ObjectKind,
        title: impl Into<String>,
    ) -> Self {
        self.inserted_contexts.push(InsertedContext {
            id: id.into(),
            kind,
            title: title.into(),
            expanded: false,
        });
        self
    }

    pub fn with_actions(mut self, actions: Vec<SuggestedAction>) -> Self {
        self.actions = actions;
        self
    }

    pub fn open_canvas(mut self, canvas_kind: CanvasKind) -> Self {
        self.canvas = Some(canvas_kind);
        self
    }

    pub fn close_canvas(mut self) -> Self {
        self.canvas = None;
        self
    }
}

pub fn derive_auth_gate(active_token: &str, verifying: bool, verified: bool) -> AuthGate {
    if active_token.trim().is_empty() {
        AuthGate::SignedOut
    } else if verifying {
        AuthGate::Verifying
    } else if verified {
        AuthGate::Ready
    } else {
        AuthGate::SignedOut
    }
}

pub fn auth_gate_label(gate: AuthGate) -> &'static str {
    match gate {
        AuthGate::SignedOut => "Not connected",
        AuthGate::Verifying => "Verifying session",
        AuthGate::Ready => "Session verified",
    }
}

pub fn workspace_tab_for_auth_gate(gate: AuthGate) -> Option<Tab> {
    match gate {
        AuthGate::Ready => Some(Tab::Brief),
        AuthGate::SignedOut | AuthGate::Verifying => None,
    }
}

pub fn build_operator_panel(
    state: &ShellState,
    auth_gate: AuthGate,
    current_case_label: &str,
) -> OperatorPanel {
    let title = state.username.clone().unwrap_or_else(|| match auth_gate {
        AuthGate::Ready => "Verified Operator".to_string(),
        AuthGate::Verifying => "Verifying Operator".to_string(),
        AuthGate::SignedOut => "Guest".to_string(),
    });
    let subtitle = auth_gate_label(auth_gate).to_string();
    let role_label = match state.role_in_case.as_deref() {
        Some("owner") => "Owner".to_string(),
        Some("member") => "Member".to_string(),
        Some("viewer") => "Viewer".to_string(),
        Some(other) => other.to_string(),
        None => "No case role".to_string(),
    };
    let summary = if state.has_case {
        "AI is scoped to the current single-case workspace.".to_string()
    } else {
        "Choose a case to unlock AI actions, timeline focus, and case-bound evidence views."
            .to_string()
    };

    OperatorPanel {
        title,
        subtitle,
        case_label: current_case_label.to_string(),
        role_label,
        summary,
    }
}

pub fn build_shell_brief(_state: &ShellState) -> ShellBrief {
    let state = _state;
    if !state.signed_in {
        return ShellBrief {
            eyebrow: "Connection".to_string(),
            title: "连接到 LegalGenie Agent".to_string(),
            summary: "先登录或粘贴访问令牌，再让 AI 接管当前案件的时间轴、证据和人物协作。"
                .to_string(),
            insights: vec![
                "认证成功后，主界面会切换到单案件 AI 指挥台。".to_string(),
                "开发连接参数被收进开发者抽屉，不再占据第一屏。".to_string(),
                "低风险操作会自动执行，高风险写入会进入待确认队列。".to_string(),
            ],
        };
    }

    if !state.has_case {
        return ShellBrief {
            eyebrow: "Case Context".to_string(),
            title: "先选择一个案件".to_string(),
            summary: "AI 已经在线，但还没有案件上下文。先创建或选择案件，再开始生成简报、时间轴建议和导出草案。".to_string(),
            insights: vec![
                "进入案件后，Brief 会优先显示关键争点和证据缺口。".to_string(),
                "案件上下文确定后，AI 才会给出可执行动作队列。".to_string(),
                "人物、文件、时间轴都会被锁到当前案件范围。".to_string(),
            ],
        };
    }

    match state.active_tab {
        Tab::Brief => ShellBrief {
            eyebrow: "AI Lead".to_string(),
            title: "AI 案件简报".to_string(),
            summary: "先看关键争点、证据缺口和建议动作，再决定进入时间轴、证据还是人物视图。"
                .to_string(),
            insights: vec![
                "简报优先展示风险、冲突点和下一步建议。".to_string(),
                "动作队列按自动执行、待确认、高风险三层展示。".to_string(),
                "案件内所有视图都从这个简报入口展开。".to_string(),
            ],
        },
        Tab::Cases => ShellBrief {
            eyebrow: "Case Library".to_string(),
            title: "管理案件上下文".to_string(),
            summary: "在这里创建、选择和切换案件，决定 AI 接下来围绕哪个案卷工作。".to_string(),
            insights: vec![
                "创建后会立即把案件设为当前上下文。".to_string(),
                "切换案件会同步刷新简报、动作队列和权限标签。".to_string(),
                "案件列表是所有下游工作区的入口。".to_string(),
            ],
        },
        Tab::Timeline => ShellBrief {
            eyebrow: "Timeline Control".to_string(),
            title: "让 AI 驾驶时间轴".to_string(),
            summary: "AI 可以先梳理时间线、识别证据缺口，并把会修改案情结构的动作推入待确认队列。"
                .to_string(),
            insights: vec![
                "自动操作用于定位、筛选和摘要，不直接改写案件事实。".to_string(),
                "节点创建、批量改动和证据关联默认先预览。".to_string(),
                "人物合并和高风险变更仍需人工二次确认。".to_string(),
            ],
        },
        Tab::Files => ShellBrief {
            eyebrow: "Evidence View".to_string(),
            title: "以证据驱动 AI".to_string(),
            summary: "围绕当前案件的证据解析结果生成摘要、疑点和下一步处理建议。".to_string(),
            insights: vec![
                "优先处理仍在解析中的文件和缺少锚点的证据。".to_string(),
                "证据摘要适合自动生成，但入库关联应进入确认队列。".to_string(),
                "导出前可以先由 AI 生成证据包草案。".to_string(),
            ],
        },
        Tab::Persons => ShellBrief {
            eyebrow: "People Graph".to_string(),
            title: "聚焦关键人物与关系".to_string(),
            summary: "AI 会标出人物冲突、关系缺口和需要确认的合并建议。".to_string(),
            insights: vec![
                "新增人物和补充资料适合待确认执行。".to_string(),
                "合并人物和跨案关联属于高风险操作。".to_string(),
                "人物图谱应该服务于案件叙事，而不是独立存在。".to_string(),
            ],
        },
        Tab::Search => ShellBrief {
            eyebrow: "Search".to_string(),
            title: "从搜索回到行动".to_string(),
            summary: "搜索不只是找结果，AI 会把高价值命中整理成下一步动作和证据线索。".to_string(),
            insights: vec![
                "优先展示与当前案件最相关的命中。".to_string(),
                "搜索结果应该能一键回到时间轴、证据或人物。".to_string(),
                "AI 会把重复查询沉淀为可复用策略。".to_string(),
            ],
        },
        Tab::Exports => ShellBrief {
            eyebrow: "Deliverables".to_string(),
            title: "把案件工作转成交付物".to_string(),
            summary: "AI 先整理导出草案和上下文，再把正式导出留给人工确认。".to_string(),
            insights: vec![
                "导出属于待确认写动作，不应静默触发。".to_string(),
                "正式材料前应附带 AI 生成依据和范围说明。".to_string(),
                "导出区应该紧贴当前案件，而不是全局散放。".to_string(),
            ],
        },
        Tab::Logs => ShellBrief {
            eyebrow: "Audit".to_string(),
            title: "先看审计，再决定放权".to_string(),
            summary:
                "当 AI 参与执行后，日志视图需要清楚展示谁触发、系统做了什么、哪些动作仍待确认。"
                    .to_string(),
            insights: vec![
                "自动执行必须可追踪、可解释。".to_string(),
                "高风险动作应在日志里清楚标记审批链。".to_string(),
                "日志视图是建立信任的最后一层。".to_string(),
            ],
        },
    }
}

pub fn build_action_queue(_state: &ShellState) -> Vec<ShellAction> {
    let state = _state;
    if !state.signed_in {
        return vec![
            ShellAction {
                title: "登录到 AI 指挥台".to_string(),
                summary: "进入单案件工作流，让 AI 开始生成案件简报和建议动作。".to_string(),
                cta: "Login".to_string(),
                risk: ActionRisk::Auto,
                intent: ActionIntent::OpenAuth,
            },
            ShellAction {
                title: "校准 API 与令牌".to_string(),
                summary: "在开发者抽屉里修改 API Base、粘贴 Access Token，排除连接问题。"
                    .to_string(),
                cta: "Connection".to_string(),
                risk: ActionRisk::Auto,
                intent: ActionIntent::ToggleDevtools,
            },
        ];
    }

    if !state.has_case {
        return vec![
            ShellAction {
                title: "创建或选择案件".to_string(),
                summary: "先确定单案件上下文，AI 才能生成有效的简报和动作建议。".to_string(),
                cta: "Open Cases".to_string(),
                risk: ActionRisk::Auto,
                intent: ActionIntent::OpenCases,
            },
            ShellAction {
                title: "回到总览简报".to_string(),
                summary: "让首页保持为 AI 主驾驶入口，再从简报进入具体工作视图。".to_string(),
                cta: "Open Brief".to_string(),
                risk: ActionRisk::Auto,
                intent: ActionIntent::OpenBrief,
            },
        ];
    }

    match state.active_tab {
        Tab::Brief => vec![
            ShellAction {
                title: "进入时间轴焦点".to_string(),
                summary: "切到时间轴视图，检查 AI 刚刚标出的关键日期和争点。".to_string(),
                cta: "Open Timeline".to_string(),
                risk: ActionRisk::Auto,
                intent: ActionIntent::OpenTimeline,
            },
            ShellAction {
                title: "审阅证据缺口".to_string(),
                summary: "进入证据视图，确认需要补充解析或建立锚点的文件。".to_string(),
                cta: "Open Evidence".to_string(),
                risk: ActionRisk::ReviewRequired,
                intent: ActionIntent::OpenFiles,
            },
            ShellAction {
                title: "生成导出草案".to_string(),
                summary: "把当前案件简报整理成可供复核的导出草案。".to_string(),
                cta: "Open Exports".to_string(),
                risk: ActionRisk::ReviewRequired,
                intent: ActionIntent::OpenExports,
            },
        ],
        Tab::Cases => vec![
            ShellAction {
                title: "回到 AI 简报".to_string(),
                summary: "选中案件后立即回到总览，让 AI 接管后续办案节奏。".to_string(),
                cta: "Open Brief".to_string(),
                risk: ActionRisk::Auto,
                intent: ActionIntent::OpenBrief,
            },
            ShellAction {
                title: "进入时间轴".to_string(),
                summary: "切到时间轴开始梳理事实与证据节点。".to_string(),
                cta: "Open Timeline".to_string(),
                risk: ActionRisk::Auto,
                intent: ActionIntent::OpenTimeline,
            },
            ShellAction {
                title: "预览案件导出".to_string(),
                summary: "先由 AI 生成一版导出草案，再决定是否正式输出。".to_string(),
                cta: "Open Exports".to_string(),
                risk: ActionRisk::ReviewRequired,
                intent: ActionIntent::OpenExports,
            },
        ],
        Tab::Timeline => vec![
            ShellAction {
                title: "刷新时间轴焦点".to_string(),
                summary: "自动定位当前案件最关键的节点区段和上下文。".to_string(),
                cta: "Stay on Timeline".to_string(),
                risk: ActionRisk::Auto,
                intent: ActionIntent::OpenTimeline,
            },
            ShellAction {
                title: "生成候选节点与证据链接".to_string(),
                summary: "先预览 AI 建议的新节点、日期修正和证据关联，再决定是否写入。".to_string(),
                cta: "Review on Timeline".to_string(),
                risk: ActionRisk::ReviewRequired,
                intent: ActionIntent::OpenTimeline,
            },
            ShellAction {
                title: "处理人物冲突与合并".to_string(),
                summary: "高风险人物操作应转到人物视图并保持人工二次确认。".to_string(),
                cta: "Open Persons".to_string(),
                risk: ActionRisk::Guarded,
                intent: ActionIntent::OpenPersons,
            },
        ],
        Tab::Files => vec![
            ShellAction {
                title: "扫描待解析证据".to_string(),
                summary: "自动回看仍在处理中或缺少摘要的文件。".to_string(),
                cta: "Stay on Evidence".to_string(),
                risk: ActionRisk::Auto,
                intent: ActionIntent::OpenFiles,
            },
            ShellAction {
                title: "整理证据摘要包".to_string(),
                summary: "先由 AI 生成一版摘要包，再决定是否加入正式导出。".to_string(),
                cta: "Open Exports".to_string(),
                risk: ActionRisk::ReviewRequired,
                intent: ActionIntent::OpenExports,
            },
            ShellAction {
                title: "核对跨人物证据冲突".to_string(),
                summary: "证据涉及人物归属变化时，优先转到人物视图做高风险复核。".to_string(),
                cta: "Open Persons".to_string(),
                risk: ActionRisk::Guarded,
                intent: ActionIntent::OpenPersons,
            },
        ],
        Tab::Persons => vec![
            ShellAction {
                title: "回看关键时间节点".to_string(),
                summary: "自动跳回时间轴，核对人物与事件是否对齐。".to_string(),
                cta: "Open Timeline".to_string(),
                risk: ActionRisk::Auto,
                intent: ActionIntent::OpenTimeline,
            },
            ShellAction {
                title: "补充人物画像".to_string(),
                summary: "先预览 AI 对角色、机构和参与日期的补全建议。".to_string(),
                cta: "Review Persons".to_string(),
                risk: ActionRisk::ReviewRequired,
                intent: ActionIntent::OpenPersons,
            },
            ShellAction {
                title: "合并疑似重复人物".to_string(),
                summary: "人物合并会影响全案上下文，必须作为高风险操作处理。".to_string(),
                cta: "Confirm Merge".to_string(),
                risk: ActionRisk::Guarded,
                intent: ActionIntent::OpenPersons,
            },
        ],
        Tab::Search => vec![
            ShellAction {
                title: "继续搜索案件线索".to_string(),
                summary: "在当前案件范围内继续扩展搜索，再把高价值命中回流到简报和动作队列。"
                    .to_string(),
                cta: "Stay on Search".to_string(),
                risk: ActionRisk::Auto,
                intent: ActionIntent::OpenSearch,
            },
            ShellAction {
                title: "把命中转成候选节点".to_string(),
                summary: "搜索结果进入时间轴前先由你确认结构化建议。".to_string(),
                cta: "Open Timeline".to_string(),
                risk: ActionRisk::ReviewRequired,
                intent: ActionIntent::OpenTimeline,
            },
            ShellAction {
                title: "检查跨人物冲突".to_string(),
                summary: "跨人物命中可能导致错误归因，转到人物视图做高风险检查。".to_string(),
                cta: "Open Persons".to_string(),
                risk: ActionRisk::Guarded,
                intent: ActionIntent::OpenPersons,
            },
        ],
        Tab::Exports => vec![
            ShellAction {
                title: "回到 AI 简报".to_string(),
                summary: "自动补齐导出草案前的背景说明和争点摘要。".to_string(),
                cta: "Open Brief".to_string(),
                risk: ActionRisk::Auto,
                intent: ActionIntent::OpenBrief,
            },
            ShellAction {
                title: "生成正式导出预览".to_string(),
                summary: "导出前先让 AI 生成一版可读预览和覆盖范围说明。".to_string(),
                cta: "Review Export".to_string(),
                risk: ActionRisk::ReviewRequired,
                intent: ActionIntent::OpenExports,
            },
            ShellAction {
                title: "核对敏感内容边界".to_string(),
                summary: "对外导出前应回看日志和权限，避免越权披露。".to_string(),
                cta: "Open Logs".to_string(),
                risk: ActionRisk::Guarded,
                intent: ActionIntent::OpenLogs,
            },
        ],
        Tab::Logs => vec![
            ShellAction {
                title: "返回 AI 简报".to_string(),
                summary: "自动把最近动作沉淀成下一轮简报和工作建议。".to_string(),
                cta: "Open Brief".to_string(),
                risk: ActionRisk::Auto,
                intent: ActionIntent::OpenBrief,
            },
            ShellAction {
                title: "复核待确认动作".to_string(),
                summary: "先确认 AI 已经准备好的写入动作，再释放下一轮执行。".to_string(),
                cta: "Review Queue".to_string(),
                risk: ActionRisk::ReviewRequired,
                intent: ActionIntent::OpenLogs,
            },
            ShellAction {
                title: "检查权限与越权风险".to_string(),
                summary: "高风险操作前应先看成员角色和跨案件访问边界。".to_string(),
                cta: "Open Persons".to_string(),
                risk: ActionRisk::Guarded,
                intent: ActionIntent::OpenPersons,
            },
        ],
    }
}

pub fn risk_label(risk: ActionRisk) -> &'static str {
    match risk {
        ActionRisk::Auto => "Auto",
        ActionRisk::ReviewRequired => "Review Required",
        ActionRisk::Guarded => "Guarded",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(
        signed_in: bool,
        has_case: bool,
        role_in_case: Option<&str>,
        active_tab: Tab,
    ) -> ShellState {
        ShellState {
            signed_in,
            has_case,
            role_in_case: role_in_case.map(str::to_string),
            active_tab,
            username: Some("analyst".to_string()),
        }
    }

    #[test]
    fn logged_out_brief_prioritizes_connection() {
        let brief = build_shell_brief(&state(false, false, None, Tab::Brief));
        let actions = build_action_queue(&state(false, false, None, Tab::Brief));

        assert!(brief.title.contains("连接"));
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].intent, ActionIntent::OpenAuth);
        assert_eq!(actions[1].intent, ActionIntent::ToggleDevtools);
    }

    #[test]
    fn signed_in_without_case_points_to_case_selection() {
        let brief = build_shell_brief(&state(true, false, None, Tab::Brief));
        let actions = build_action_queue(&state(true, false, None, Tab::Brief));

        assert!(brief.summary.contains("案件"));
        assert_eq!(actions[0].intent, ActionIntent::OpenCases);
        assert!(actions.iter().any(|action| action.title.contains("创建")));
    }

    #[test]
    fn timeline_case_mix_surfaces_auto_review_and_guarded_actions() {
        let actions = build_action_queue(&state(true, true, Some("owner"), Tab::Timeline));

        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].risk, ActionRisk::Auto);
        assert_eq!(actions[1].risk, ActionRisk::ReviewRequired);
        assert_eq!(actions[2].risk, ActionRisk::Guarded);
        assert_eq!(actions[2].intent, ActionIntent::OpenPersons);
    }

    #[test]
    fn exports_action_keeps_log_cta_and_intent_aligned() {
        let actions = build_action_queue(&state(true, true, Some("owner"), Tab::Exports));

        let sensitive_action = &actions[2];
        assert_eq!(sensitive_action.cta, "Open Logs");
        assert_eq!(sensitive_action.intent, ActionIntent::OpenLogs);
    }

    #[test]
    fn auth_gate_stays_verifying_after_login_until_auth_me_succeeds() {
        assert_eq!(derive_auth_gate("", false, false), AuthGate::SignedOut);
        assert_eq!(
            derive_auth_gate("draft-token", true, false),
            AuthGate::Verifying
        );
        assert_eq!(
            derive_auth_gate("draft-token", false, false),
            AuthGate::SignedOut
        );
        assert_eq!(
            derive_auth_gate("verified-token", false, true),
            AuthGate::Ready
        );
    }

    #[test]
    fn auth_gate_label_matches_gate_state() {
        assert_eq!(auth_gate_label(AuthGate::SignedOut), "Not connected");
        assert_eq!(auth_gate_label(AuthGate::Verifying), "Verifying session");
        assert_eq!(auth_gate_label(AuthGate::Ready), "Session verified");
    }

    #[test]
    fn risk_label_returns_review_required_clarity() {
        assert_eq!(risk_label(ActionRisk::ReviewRequired), "Review Required");
    }

    #[test]
    fn operator_panel_for_verified_case_owner_surfaces_identity_and_scope() {
        let panel = build_operator_panel(
            &state(true, true, Some("owner"), Tab::Brief),
            AuthGate::Ready,
            "Case 123456789abc",
        );

        assert_eq!(panel.title, "analyst");
        assert_eq!(panel.case_label, "Case 123456789abc");
        assert_eq!(panel.role_label, "Owner");
        assert!(panel.summary.contains("single-case"));
    }

    #[test]
    fn operator_panel_without_case_pushes_case_selection() {
        let mut shell_state = state(true, false, None, Tab::Brief);
        shell_state.username = None;

        let panel = build_operator_panel(&shell_state, AuthGate::Ready, "No case selected");

        assert_eq!(panel.title, "Verified Operator");
        assert_eq!(panel.role_label, "No case role");
        assert!(panel.summary.contains("Choose a case"));
    }

    #[test]
    fn workspace_route_only_opens_after_verified_session() {
        assert_eq!(workspace_tab_for_auth_gate(AuthGate::SignedOut), None);
        assert_eq!(workspace_tab_for_auth_gate(AuthGate::Verifying), None);
        assert_eq!(
            workspace_tab_for_auth_gate(AuthGate::Ready),
            Some(Tab::Brief)
        );
    }

    #[test]
    fn conversation_can_stay_empty_while_canvas_is_open() {
        let state = WorkspaceState::new()
            .with_case(true)
            .with_canvas(Some(CanvasKind::Timeline));

        assert_eq!(state.conversation_stage, ConversationStage::Empty);
        assert_eq!(state.canvas, Some(CanvasKind::Timeline));
    }

    #[test]
    fn opening_canvas_does_not_clear_active_conversation() {
        let state = WorkspaceState::new()
            .with_case(true)
            .after_user_submission()
            .with_inserted_context("ctx-1", ObjectKind::Evidence, "合同原件")
            .open_canvas(CanvasKind::Timeline);

        assert_eq!(state.conversation_stage, ConversationStage::Active);
        assert_eq!(state.canvas, Some(CanvasKind::Timeline));
        assert_eq!(state.inserted_contexts.len(), 1);
    }

    #[test]
    fn closing_canvas_returns_to_none_without_touching_context_blocks() {
        let state = WorkspaceState::new()
            .with_case(true)
            .after_user_submission()
            .with_inserted_context("ctx-1", ObjectKind::TimelineNode, "付款节点")
            .open_canvas(CanvasKind::Persons)
            .close_canvas();

        assert_eq!(state.canvas, None);
        assert_eq!(state.conversation_stage, ConversationStage::Active);
        assert_eq!(state.inserted_contexts.len(), 1);
        assert_eq!(state.inserted_contexts[0].title, "付款节点");
    }

    #[test]
    fn conversation_only_becomes_active_after_first_submission() {
        let mut state = WorkspaceState::new().with_case(true);
        assert_eq!(state.conversation_stage, ConversationStage::Empty);

        state = state.after_user_submission();
        assert_eq!(state.conversation_stage, ConversationStage::Active);
    }

    #[test]
    fn plan_is_the_default_right_panel_tab() {
        let right_panel = default_right_panel_state();
        assert_eq!(right_panel.tab, RightPanelTab::Plan);
        assert_eq!(right_panel.mode, PanelMode::Expanded);
    }

    #[test]
    fn collapsing_right_panel_keeps_selected_tab() {
        let state = WorkspaceState::new()
            .with_right_panel(RightPanelTab::Results, PanelMode::Expanded)
            .toggle_panel_collapsed();

        assert_eq!(state.right_panel.tab, RightPanelTab::Results);
        assert_eq!(state.right_panel.mode, PanelMode::Collapsed);
    }

    #[test]
    fn selected_object_does_not_enter_prompt_until_inserted() {
        let state =
            WorkspaceState::new().select_object("node-1", ObjectKind::TimelineNode, "付款节点");

        assert!(state.selected_object.is_some());
        assert!(state.inserted_contexts.is_empty());
    }

    #[test]
    fn explicit_insert_moves_selected_object_into_inserted_contexts() {
        let selected = Some(SelectedObject {
            id: "node-1".to_string(),
            kind: ObjectKind::TimelineNode,
            title: "付款节点".to_string(),
        });
        let mut inserted = Vec::new();

        let inserted_now = insert_selected_object(&selected, &mut inserted);

        assert!(inserted_now);
        assert_eq!(inserted.len(), 1);
        assert_eq!(inserted[0].title, "付款节点");
        assert_eq!(inserted[0].kind, ObjectKind::TimelineNode);
        assert!(!inserted[0].expanded);
    }

    #[test]
    fn submitting_non_empty_message_appends_and_clears_draft() {
        let mut messages = vec!["既有消息".to_string()];
        let mut draft = "请总结当前争议焦点".to_string();

        let submitted = submit_local_conversation_message(&mut messages, &mut draft);

        assert!(submitted);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1], "请总结当前争议焦点");
        assert!(draft.is_empty());
    }

    #[test]
    fn pinned_panel_can_collapse_without_losing_tab() {
        let state = WorkspaceState::new()
            .with_right_panel(RightPanelTab::Results, PanelMode::Pinned)
            .toggle_panel_collapsed();

        assert_eq!(state.right_panel.mode, PanelMode::Collapsed);
        assert_eq!(state.right_panel.tab, RightPanelTab::Results);
    }

    #[test]
    fn suggested_actions_keep_their_risk_level() {
        let state = WorkspaceState::new().with_actions(vec![
            SuggestedAction::new("生成节点草案", ActionRisk::ReviewRequired),
            SuggestedAction::new("合并人物", ActionRisk::Guarded),
        ]);

        assert_eq!(state.actions[0].risk, ActionRisk::ReviewRequired);
        assert_eq!(state.actions[1].risk, ActionRisk::Guarded);
    }

    #[test]
    fn workspace_state_tracks_history_scope_variants() {
        let state = WorkspaceState::new().with_history_scope(HistoryScope::AllConversations);

        assert_eq!(state.history_scope, HistoryScope::AllConversations);

        let state = state.with_history_scope(HistoryScope::CurrentCase);
        assert_eq!(state.history_scope, HistoryScope::CurrentCase);
    }
}
