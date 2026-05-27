use crate::shell::{
    ActionRisk, CanvasKind, ConversationStage, HistoryScope, InsertedContext, ObjectKind,
    PanelMode, RightPanelTab, ShellAction, ShellBrief, Tab,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickEntryCardViewModel {
    pub id: &'static str,
    pub eyebrow: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StarterPromptViewModel {
    pub title: &'static str,
    pub description: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuggestedActionSummaryViewModel {
    pub title: String,
    pub summary: String,
    pub badge_text: &'static str,
    pub badge_class: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationViewModel {
    pub stage_label: &'static str,
    pub hero_title: &'static str,
    pub hero_summary: &'static str,
    pub composer_label: &'static str,
    pub composer_hint: &'static str,
    pub show_starters: bool,
    pub starter_prompts: Vec<StarterPromptViewModel>,
    pub quick_entries: Vec<QuickEntryCardViewModel>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftRailPrimaryEntryViewModel {
    pub id: &'static str,
    pub label: &'static str,
    pub tab: Tab,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftRailHistoryItemViewModel {
    pub title: String,
    pub case_label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftRailViewModel {
    pub active_scope_label: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UtilityEntryViewModel {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InsertedContextViewModel {
    pub id: String,
    pub title: String,
    pub kind_label: &'static str,
    pub expanded: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasContextCandidateViewModel {
    pub id: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub kind: ObjectKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RightPanelTabViewModel {
    pub tab: RightPanelTab,
    pub label: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RightPanelItemViewModel {
    pub eyebrow: String,
    pub title: String,
    pub summary: Option<String>,
    pub badge_text: Option<&'static str>,
    pub badge_class: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RightPanelSurfaceViewModel {
    pub active_tab: RightPanelTab,
    pub mode: PanelMode,
    pub tabs: Vec<RightPanelTabViewModel>,
    pub section_title: &'static str,
    pub section_hint: &'static str,
    pub items: Vec<RightPanelItemViewModel>,
    pub empty_text: &'static str,
    pub collapse_label: &'static str,
    pub pin_label: &'static str,
}

pub fn conversation_view_model(stage: ConversationStage) -> ConversationViewModel {
    ConversationViewModel {
        stage_label: match stage {
            ConversationStage::Empty => "新会话",
            ConversationStage::Active => "当前会话",
        },
        hero_title: match stage {
            ConversationStage::Empty => "从一个问题开始，AI 会先整理计划再展开工作区",
            ConversationStage::Active => "继续当前案件对话，并按需进入时间轴、证据或人物工作层",
        },
        hero_summary: match stage {
            ConversationStage::Empty => {
                "先给出案件问题、争点或目标，右侧计划会跟着生成，专业工作区保持按需打开。"
            }
            ConversationStage::Active => {
                "主对话始终保留，右侧计划和引用随时可切换，Canvas 只在你需要时覆盖中央工作区。"
            }
        },
        composer_label: "描述你要推进的案件问题",
        composer_hint: "主对话始终是入口。先提问，再决定是否进入时间轴、证据、人物或导出工作层。",
        show_starters: stage == ConversationStage::Empty,
        starter_prompts: vec![
            StarterPromptViewModel {
                title: "先给我本案争议焦点和风险清单",
                description: "快速建立案件简报和执行边界。",
            },
            StarterPromptViewModel {
                title: "基于现有材料生成下一步行动建议",
                description: "把后续动作先排成计划和优先级。",
            },
            StarterPromptViewModel {
                title: "总结证据缺口并给出补证优先级",
                description: "直接聚焦补证、举证和责任分工。",
            },
        ],
        quick_entries: vec![
            QuickEntryCardViewModel {
                id: "timeline",
                eyebrow: "Canvas",
                label: "时间轴",
                description: "打开时间轴工作层，围绕关键节点继续追问。",
            },
            QuickEntryCardViewModel {
                id: "evidence",
                eyebrow: "Canvas",
                label: "证据",
                description: "进入证据工作层，筛材料、看解析、补引用。",
            },
            QuickEntryCardViewModel {
                id: "persons",
                eyebrow: "Canvas",
                label: "人物",
                description: "查看人物网络、职责边界和关系图谱。",
            },
            QuickEntryCardViewModel {
                id: "exports",
                eyebrow: "Canvas",
                label: "导出",
                description: "生成摘要、提纲和导出结果，并回看执行回执。",
            },
        ],
    }
}

pub fn history_scope_label(scope: HistoryScope) -> &'static str {
    match scope {
        HistoryScope::CurrentCase => "当前案件",
        HistoryScope::AllConversations => "全部会话",
    }
}

pub fn left_rail_view_model(scope: HistoryScope) -> LeftRailViewModel {
    LeftRailViewModel {
        active_scope_label: history_scope_label(scope),
    }
}

pub fn left_rail_view_model_default(scope: Option<HistoryScope>) -> LeftRailViewModel {
    left_rail_view_model(scope.unwrap_or(HistoryScope::CurrentCase))
}

pub fn left_rail_primary_entries() -> Vec<LeftRailPrimaryEntryViewModel> {
    vec![
        LeftRailPrimaryEntryViewModel {
            id: "brief",
            label: "会话",
            tab: Tab::Brief,
        },
        LeftRailPrimaryEntryViewModel {
            id: "cases",
            label: "案件",
            tab: Tab::Cases,
        },
    ]
}

pub fn left_rail_history_item(
    title: &str,
    scope: HistoryScope,
    case_label: Option<&str>,
) -> LeftRailHistoryItemViewModel {
    let case_label = match scope {
        HistoryScope::CurrentCase => None,
        HistoryScope::AllConversations => case_label.map(str::to_string),
    };
    LeftRailHistoryItemViewModel {
        title: title.to_string(),
        case_label,
    }
}

pub fn utility_entries_view_model() -> Vec<UtilityEntryViewModel> {
    vec![
        UtilityEntryViewModel {
            id: "search",
            label: "Search",
        },
        UtilityEntryViewModel {
            id: "logs",
            label: "Logs",
        },
    ]
}

pub fn primary_nav_tabs() -> Vec<Tab> {
    left_rail_primary_entries()
        .into_iter()
        .map(|entry| entry.tab)
        .collect()
}

pub fn suggested_action_summaries(actions: &[ShellAction]) -> Vec<SuggestedActionSummaryViewModel> {
    actions
        .iter()
        .map(|action| SuggestedActionSummaryViewModel {
            title: action.title.clone(),
            summary: action.summary.clone(),
            badge_text: match action.risk {
                ActionRisk::Auto => "Auto",
                ActionRisk::ReviewRequired => "ReviewRequired",
                ActionRisk::Guarded => "Guarded",
            },
            badge_class: match action.risk {
                ActionRisk::Auto => "badge badge--ok",
                ActionRisk::ReviewRequired => "badge badge--run",
                ActionRisk::Guarded => "badge badge--warn",
            },
        })
        .collect()
}

pub fn inserted_context_view_model(
    id: &str,
    title: &str,
    kind: ObjectKind,
    expanded: bool,
) -> InsertedContextViewModel {
    InsertedContextViewModel {
        id: id.to_string(),
        title: title.to_string(),
        kind_label: match kind {
            ObjectKind::TimelineNode => "时间轴节点",
            ObjectKind::Evidence => "证据",
            ObjectKind::Person => "人物",
        },
        expanded,
    }
}

pub fn inserted_context_view_models(
    inserted_contexts: &[InsertedContext],
) -> Vec<InsertedContextViewModel> {
    inserted_contexts
        .iter()
        .map(|item| inserted_context_view_model(&item.id, &item.title, item.kind, item.expanded))
        .collect()
}

pub fn canvas_header_title(canvas: Option<CanvasKind>) -> &'static str {
    match canvas {
        None => "Conversation Workspace",
        Some(CanvasKind::Timeline) => "Timeline Canvas",
        Some(CanvasKind::Evidence) => "Evidence Canvas",
        Some(CanvasKind::Persons) => "Persons Canvas",
        Some(CanvasKind::Exports) => "Exports Canvas",
    }
}

pub fn canvas_context_candidates(canvas: CanvasKind) -> Vec<CanvasContextCandidateViewModel> {
    match canvas {
        CanvasKind::Timeline => vec![
            CanvasContextCandidateViewModel {
                id: "timeline-node-payment",
                title: "付款节点",
                summary: "围绕付款时间、金额和履约争议继续追问。",
                kind: ObjectKind::TimelineNode,
            },
            CanvasContextCandidateViewModel {
                id: "timeline-node-signing",
                title: "签约节点",
                summary: "核对合同签订时间和责任分工。",
                kind: ObjectKind::TimelineNode,
            },
        ],
        CanvasKind::Evidence => vec![
            CanvasContextCandidateViewModel {
                id: "evidence-contract",
                title: "合同原件",
                summary: "检查核心条款、签章和履约依据。",
                kind: ObjectKind::Evidence,
            },
            CanvasContextCandidateViewModel {
                id: "evidence-chat",
                title: "聊天记录",
                summary: "提取关键承诺、催款和违约表述。",
                kind: ObjectKind::Evidence,
            },
        ],
        CanvasKind::Persons => vec![
            CanvasContextCandidateViewModel {
                id: "person-project-manager",
                title: "项目经理",
                summary: "聚焦职责边界、汇报链和现场决策。",
                kind: ObjectKind::Person,
            },
            CanvasContextCandidateViewModel {
                id: "person-legal-representative",
                title: "法定代表人",
                summary: "聚焦授权范围、对外承诺和签约责任。",
                kind: ObjectKind::Person,
            },
        ],
        CanvasKind::Exports => vec![
            CanvasContextCandidateViewModel {
                id: "export-case-brief",
                title: "案情摘要草案",
                summary: "检查摘要是否覆盖争议焦点和关键证据。",
                kind: ObjectKind::Evidence,
            },
            CanvasContextCandidateViewModel {
                id: "export-hearing-outline",
                title: "庭审提纲草案",
                summary: "围绕发问顺序和证明目标继续推演。",
                kind: ObjectKind::Evidence,
            },
        ],
    }
}

pub fn right_panel_surface_view_model(
    active_tab: RightPanelTab,
    mode: PanelMode,
    brief: &ShellBrief,
    action_summaries: &[SuggestedActionSummaryViewModel],
    inserted_contexts: &[InsertedContextViewModel],
) -> RightPanelSurfaceViewModel {
    let (section_title, section_hint, items, empty_text) = match active_tab {
        RightPanelTab::Plan => (
            "Plan",
            "来自案件简报的执行计划",
            std::iter::once(RightPanelItemViewModel {
                eyebrow: brief.eyebrow.clone(),
                title: brief.title.clone(),
                summary: Some(brief.summary.clone()),
                badge_text: None,
                badge_class: "badge",
            })
            .chain(
                brief
                    .insights
                    .iter()
                    .map(|insight| RightPanelItemViewModel {
                        eyebrow: "Insight".to_string(),
                        title: insight.clone(),
                        summary: None,
                        badge_text: None,
                        badge_class: "badge",
                    }),
            )
            .collect(),
            "暂无简报计划，请先生成案件简报。",
        ),
        RightPanelTab::References => (
            "References",
            "已插入会话的上下文引用",
            inserted_contexts
                .iter()
                .map(|item| RightPanelItemViewModel {
                    eyebrow: item.kind_label.to_string(),
                    title: item.title.clone(),
                    summary: Some("已带入主对话，可直接围绕这个对象继续追问。".to_string()),
                    badge_text: None,
                    badge_class: "badge",
                })
                .collect(),
            "暂无引用上下文，可从时间轴/证据/人物插入。",
        ),
        RightPanelTab::Results => (
            "Results",
            "执行结果与建议动作回执",
            action_summaries
                .iter()
                .map(|item| RightPanelItemViewModel {
                    eyebrow: "Action".to_string(),
                    title: item.title.clone(),
                    summary: Some(item.summary.clone()),
                    badge_text: Some(item.badge_text),
                    badge_class: item.badge_class,
                })
                .collect(),
            "暂无结果，运行动作后会出现在这里。",
        ),
    };

    RightPanelSurfaceViewModel {
        active_tab,
        mode,
        tabs: vec![
            RightPanelTabViewModel {
                tab: RightPanelTab::Plan,
                label: "Plan",
            },
            RightPanelTabViewModel {
                tab: RightPanelTab::References,
                label: "References",
            },
            RightPanelTabViewModel {
                tab: RightPanelTab::Results,
                label: "Results",
            },
        ],
        section_title,
        section_hint,
        items,
        empty_text,
        collapse_label: match mode {
            PanelMode::Collapsed => "Expand",
            PanelMode::Expanded | PanelMode::Pinned => "Collapse",
        },
        pin_label: match mode {
            PanelMode::Pinned => "Unpin",
            PanelMode::Expanded | PanelMode::Collapsed => "Pin",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::{
        CanvasKind, ConversationStage, HistoryScope, ObjectKind, RightPanelTab, ShellBrief,
        WorkspaceState,
    };

    #[test]
    fn starter_prompts_only_show_for_empty_conversation() {
        assert!(conversation_view_model(ConversationStage::Empty).show_starters);
        assert!(!conversation_view_model(ConversationStage::Active).show_starters);
    }

    #[test]
    fn empty_conversation_home_prioritizes_guided_entry_copy() {
        let vm = conversation_view_model(ConversationStage::Empty);
        assert_eq!(vm.hero_title, "从一个问题开始，AI 会先整理计划再展开工作区");
        assert_eq!(vm.composer_label, "描述你要推进的案件问题");
        assert!(vm
            .quick_entries
            .iter()
            .all(|item| !item.description.is_empty()));
    }

    #[test]
    fn inserted_context_block_starts_collapsed() {
        let vm = inserted_context_view_model(
            "timeline-node-payment",
            "付款节点",
            ObjectKind::TimelineNode,
            false,
        );
        assert!(!vm.expanded);
    }

    #[test]
    fn professional_views_are_not_primary_left_rail_entries() {
        let entries = left_rail_primary_entries();
        assert!(entries.iter().all(|item| item.id != "timeline"));
        assert!(entries.iter().all(|item| item.id != "files"));
        assert!(entries.iter().all(|item| item.id != "persons"));
        assert!(entries.iter().all(|item| item.id != "exports"));
    }

    #[test]
    fn left_rail_defaults_to_current_case_history() {
        let vm = left_rail_view_model_default(None);
        assert_eq!(vm.active_scope_label, "当前案件");
    }

    #[test]
    fn all_conversation_scope_renders_case_badges() {
        let vm =
            left_rail_history_item("会话 B", HistoryScope::AllConversations, Some("民间借贷案"));
        assert_eq!(vm.case_label.as_deref(), Some("民间借贷案"));
    }

    #[test]
    fn utility_menu_contains_search_and_logs() {
        let vm = utility_entries_view_model();
        assert!(vm.iter().any(|item| item.id == "search"));
        assert!(vm.iter().any(|item| item.id == "logs"));
        assert!(!vm.iter().any(|item| item.id == "timeline_nav"));
    }

    #[test]
    fn utility_entries_do_not_show_in_left_rail_primary_list() {
        let primary_entries = left_rail_primary_entries();
        assert!(!primary_entries.iter().any(|item| item.tab == Tab::Search));
        assert!(!primary_entries.iter().any(|item| item.tab == Tab::Logs));
        assert!(primary_entries.iter().any(|item| item.tab == Tab::Brief));

        let nav_tabs = primary_nav_tabs();
        assert!(!nav_tabs.contains(&Tab::Search));
        assert!(!nav_tabs.contains(&Tab::Logs));
    }

    #[test]
    fn action_summaries_emit_risk_badges() {
        let actions = vec![
            ShellAction {
                title: "A".to_string(),
                summary: "S".to_string(),
                cta: "C".to_string(),
                risk: ActionRisk::Auto,
                intent: crate::shell::ActionIntent::OpenBrief,
            },
            ShellAction {
                title: "B".to_string(),
                summary: "S".to_string(),
                cta: "C".to_string(),
                risk: ActionRisk::ReviewRequired,
                intent: crate::shell::ActionIntent::OpenBrief,
            },
            ShellAction {
                title: "C".to_string(),
                summary: "S".to_string(),
                cta: "C".to_string(),
                risk: ActionRisk::Guarded,
                intent: crate::shell::ActionIntent::OpenBrief,
            },
        ];
        let vm = suggested_action_summaries(&actions);
        assert_eq!(vm[0].badge_text, "Auto");
        assert_eq!(vm[1].badge_text, "ReviewRequired");
        assert_eq!(vm[2].badge_text, "Guarded");
        assert_eq!(vm[0].summary, "S");
    }

    #[test]
    fn canvas_header_titles_match_canvas_kind() {
        assert_eq!(canvas_header_title(None), "Conversation Workspace");
        assert_eq!(
            canvas_header_title(Some(CanvasKind::Timeline)),
            "Timeline Canvas"
        );
        assert_eq!(
            canvas_header_title(Some(CanvasKind::Evidence)),
            "Evidence Canvas"
        );
        assert_eq!(
            canvas_header_title(Some(CanvasKind::Persons)),
            "Persons Canvas"
        );
        assert_eq!(
            canvas_header_title(Some(CanvasKind::Exports)),
            "Exports Canvas"
        );
    }

    #[test]
    fn collapsing_then_expanding_restores_previous_tab() {
        let state = WorkspaceState::new()
            .switch_right_panel_tab(RightPanelTab::References)
            .collapse_right_panel()
            .expand_right_panel();
        assert_eq!(state.right_panel.tab, RightPanelTab::References);
    }

    #[test]
    fn plan_panel_uses_brief_data_not_starter_prompts() {
        let brief = ShellBrief {
            eyebrow: "AI Lead".to_string(),
            title: "案件简报标题".to_string(),
            summary: "案件简报摘要".to_string(),
            insights: vec!["洞察 A".to_string(), "洞察 B".to_string()],
        };
        let vm = right_panel_surface_view_model(
            RightPanelTab::Plan,
            crate::shell::PanelMode::Expanded,
            &brief,
            &[],
            &[],
        );
        assert_eq!(vm.items[0].title, "案件简报标题");
        assert_eq!(vm.items[0].summary.as_deref(), Some("案件简报摘要"));
        assert!(!vm
            .items
            .iter()
            .any(|item| item.title.contains("先给我本案争议焦点")));
    }

    #[test]
    fn results_panel_keeps_action_summaries_structured() {
        let actions = vec![ShellAction {
            title: "生成证据缺口清单".to_string(),
            summary: "列出补证优先级和责任人".to_string(),
            cta: "进入结果".to_string(),
            risk: ActionRisk::ReviewRequired,
            intent: crate::shell::ActionIntent::OpenBrief,
        }];
        let vm = right_panel_surface_view_model(
            RightPanelTab::Results,
            crate::shell::PanelMode::Expanded,
            &ShellBrief {
                eyebrow: "AI Lead".to_string(),
                title: "简报".to_string(),
                summary: "摘要".to_string(),
                insights: vec![],
            },
            &suggested_action_summaries(&actions),
            &[],
        );
        assert_eq!(vm.items[0].title, "生成证据缺口清单");
        assert_eq!(
            vm.items[0].summary.as_deref(),
            Some("列出补证优先级和责任人")
        );
        assert_eq!(vm.items[0].badge_text, Some("ReviewRequired"));
    }

    #[test]
    fn canvas_context_candidates_exist_for_each_professional_surface() {
        assert!(canvas_context_candidates(CanvasKind::Timeline).len() >= 2);
        assert!(canvas_context_candidates(CanvasKind::Evidence).len() >= 2);
        assert!(canvas_context_candidates(CanvasKind::Persons).len() >= 2);
        assert!(canvas_context_candidates(CanvasKind::Exports).len() >= 2);
    }
}
