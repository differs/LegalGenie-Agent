use crate::shell::{
    ActionRisk, CanvasKind, ConversationStage, HistoryScope, ObjectKind, ShellAction, Tab,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickEntryCardViewModel {
    pub id: &'static str,
    pub label: &'static str,
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
    pub show_starters: bool,
    pub starter_prompts: Vec<&'static str>,
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
    pub tab: Tab,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InsertedContextViewModel {
    pub title: String,
    pub kind_label: &'static str,
    pub expanded: bool,
}

pub fn conversation_view_model(stage: ConversationStage) -> ConversationViewModel {
    ConversationViewModel {
        stage_label: match stage {
            ConversationStage::Empty => "新会话",
            ConversationStage::Active => "当前会话",
        },
        show_starters: stage == ConversationStage::Empty,
        starter_prompts: vec![
            "先给我本案争议焦点和风险清单",
            "基于现有材料生成下一步行动建议",
            "总结证据缺口并给出补证优先级",
        ],
        quick_entries: vec![
            QuickEntryCardViewModel {
                id: "timeline",
                label: "时间轴",
            },
            QuickEntryCardViewModel {
                id: "evidence",
                label: "证据",
            },
            QuickEntryCardViewModel {
                id: "persons",
                label: "人物",
            },
            QuickEntryCardViewModel {
                id: "exports",
                label: "导出",
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
            tab: Tab::Search,
        },
        UtilityEntryViewModel {
            id: "logs",
            label: "Logs",
            tab: Tab::Logs,
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
    title: &str,
    kind: ObjectKind,
    _from_canvas: bool,
) -> InsertedContextViewModel {
    InsertedContextViewModel {
        title: title.to_string(),
        kind_label: match kind {
            ObjectKind::TimelineNode => "时间轴节点",
            ObjectKind::Evidence => "证据",
            ObjectKind::Person => "人物",
        },
        expanded: false,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::{CanvasKind, ConversationStage, HistoryScope, ObjectKind};

    #[test]
    fn starter_prompts_only_show_for_empty_conversation() {
        assert!(conversation_view_model(ConversationStage::Empty).show_starters);
        assert!(!conversation_view_model(ConversationStage::Active).show_starters);
    }

    #[test]
    fn inserted_context_block_starts_collapsed() {
        let vm = inserted_context_view_model("付款节点", ObjectKind::TimelineNode, false);
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
    fn utility_links_hold_search_and_logs_out_of_primary_nav() {
        let vm = utility_entries_view_model();
        assert!(vm.iter().any(|item| item.id == "search"));
        assert!(vm.iter().any(|item| item.id == "logs"));
        assert!(!vm.iter().any(|item| item.id == "timeline_nav"));
        assert!(vm.iter().any(|item| item.tab == Tab::Search));
        assert!(vm.iter().any(|item| item.tab == Tab::Logs));
    }

    #[test]
    fn primary_nav_tabs_exclude_utility_tabs() {
        let nav_tabs = primary_nav_tabs();
        assert!(!nav_tabs.contains(&Tab::Search));
        assert!(!nav_tabs.contains(&Tab::Logs));
        assert!(nav_tabs.contains(&Tab::Brief));
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
}
