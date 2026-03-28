use crate::shell::{CanvasKind, ConversationStage, HistoryScope, Tab};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickEntryCardViewModel {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationViewModel {
    pub stage_label: &'static str,
    pub quick_entries: Vec<QuickEntryCardViewModel>,
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

pub fn conversation_view_model(
    stage: ConversationStage,
) -> ConversationViewModel {
    ConversationViewModel {
        stage_label: match stage {
            ConversationStage::Empty => "Start a new conversation",
            ConversationStage::Active => "Continue the conversation",
        },
        quick_entries: vec![
            QuickEntryCardViewModel {
                id: "timeline",
                label: "Build timeline",
            },
            QuickEntryCardViewModel {
                id: "evidence",
                label: "Review evidence",
            },
            QuickEntryCardViewModel {
                id: "persons",
                label: "Map persons",
            },
            QuickEntryCardViewModel {
                id: "exports",
                label: "Draft export",
            },
        ],
    }
}

pub fn history_scope_label(scope: HistoryScope) -> &'static str {
    match scope {
        HistoryScope::CurrentCase => "Current case",
        HistoryScope::AllConversations => "All conversations",
    }
}

pub fn left_rail_view_model(scope: HistoryScope) -> LeftRailViewModel {
    LeftRailViewModel {
        active_scope_label: history_scope_label(scope),
    }
}

pub fn left_rail_view_model_default(
    scope: Option<HistoryScope>,
) -> LeftRailViewModel {
    left_rail_view_model(scope.unwrap_or(HistoryScope::CurrentCase))
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
    let utility_tabs = utility_entries_view_model()
        .into_iter()
        .map(|entry| entry.tab)
        .collect::<Vec<_>>();
    crate::shell::ALL_TABS
        .iter()
        .copied()
        .filter(|tab| !utility_tabs.contains(tab))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::{CanvasKind, ConversationStage, HistoryScope};

    #[test]
    fn quick_entry_cards_are_always_visible_in_conversation_view() {
        let empty_vm = conversation_view_model(ConversationStage::Empty);
        let active_vm = conversation_view_model(ConversationStage::Active);
        assert_eq!(empty_vm.quick_entries.len(), 4);
        assert_eq!(active_vm.quick_entries.len(), 4);
        assert_eq!(empty_vm.stage_label, "Start a new conversation");
        assert_eq!(active_vm.stage_label, "Continue the conversation");
        assert_eq!(
            empty_vm
                .quick_entries
                .iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>(),
            active_vm
                .quick_entries
                .iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn left_rail_defaults_to_current_case_history() {
        let vm = left_rail_view_model_default(None);
        assert_eq!(vm.active_scope_label, "Current case");
    }

    #[test]
    fn all_conversation_entries_show_their_case_label() {
        let entries = vec!["会话 A", "会话 B", "会话 C"];
        let vm_items = entries
            .iter()
            .map(|title| {
                left_rail_history_item(title, HistoryScope::AllConversations, Some("劳动争议案"))
            })
            .collect::<Vec<_>>();
        assert!(vm_items
            .iter()
            .all(|item| item.case_label.as_deref() == Some("劳动争议案")));
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
        assert!(nav_tabs.contains(&Tab::Timeline));
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
