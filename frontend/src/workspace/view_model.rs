use crate::shell::{CanvasKind, ConversationStage, HistoryScope};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickEntryCardViewModel {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationViewModel {
    pub stage: ConversationStage,
    pub quick_entries: Vec<QuickEntryCardViewModel>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftRailHistoryItemViewModel {
    pub title: String,
    pub case_label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftRailViewModel {
    pub has_case: bool,
    pub active_scope_label: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UtilityEntryViewModel {
    pub id: &'static str,
    pub label: &'static str,
}

pub fn conversation_view_model(
    _has_case: bool,
    stage: ConversationStage,
) -> ConversationViewModel {
    ConversationViewModel {
        stage,
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

pub fn left_rail_view_model(has_case: bool, scope: HistoryScope) -> LeftRailViewModel {
    LeftRailViewModel {
        has_case,
        active_scope_label: history_scope_label(scope),
    }
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
        let vm = conversation_view_model(true, ConversationStage::Empty);
        assert_eq!(vm.quick_entries.len(), 4);
    }

    #[test]
    fn left_rail_defaults_to_current_case_history() {
        let vm = left_rail_view_model(true, HistoryScope::CurrentCase);
        assert_eq!(vm.active_scope_label, "Current case");
    }

    #[test]
    fn all_conversation_entries_show_their_case_label() {
        let vm = left_rail_history_item(
            "会话 A",
            HistoryScope::AllConversations,
            Some("劳动争议案"),
        );
        assert_eq!(vm.case_label.as_deref(), Some("劳动争议案"));
    }

    #[test]
    fn utility_links_hold_search_and_logs_out_of_primary_nav() {
        let vm = utility_entries_view_model();
        assert!(vm.iter().any(|item| item.id == "search"));
        assert!(vm.iter().any(|item| item.id == "logs"));
        assert!(!vm.iter().any(|item| item.id == "timeline_nav"));
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
