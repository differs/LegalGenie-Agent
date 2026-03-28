use dioxus::prelude::*;

use crate::shell::{HistoryScope, Tab};

use super::conversation::ConversationPane;
use super::left_rail::LeftRail;
use super::view_model::{
    ConversationViewModel, InsertedContextViewModel, LeftRailHistoryItemViewModel,
    SuggestedActionSummaryViewModel,
};

#[derive(Clone, PartialEq)]
pub struct WorkspaceFrameViewData {
    pub active_tab: Tab,
    pub session_line: String,
    pub case_label_text: String,
    pub role_badge_text: &'static str,
    pub role_badge_class: &'static str,
    pub auth_gate_badge_text: &'static str,
    pub auth_gate_badge_class: &'static str,
    pub history_scope: HistoryScope,
    pub history_items: Vec<LeftRailHistoryItemViewModel>,
    pub conversation: ConversationViewModel,
    pub action_summaries: Vec<SuggestedActionSummaryViewModel>,
    pub inserted_contexts: Vec<InsertedContextViewModel>,
}

#[derive(Clone, PartialEq)]
pub struct WorkspaceFrameBindings {
    pub tab: Signal<Tab>,
    pub case_id: Signal<String>,
    pub history_scope: Signal<HistoryScope>,
}

#[component]
pub fn WorkspaceFrame(
    view: WorkspaceFrameViewData,
    bindings: WorkspaceFrameBindings,
    on_start_conversation: EventHandler<String>,
) -> Element {
    let WorkspaceFrameViewData {
        active_tab,
        session_line,
        case_label_text,
        role_badge_text,
        role_badge_class,
        auth_gate_badge_text,
        auth_gate_badge_class,
        history_scope,
        history_items,
        conversation,
        action_summaries,
        inserted_contexts,
    } = view;
    let WorkspaceFrameBindings {
        tab,
        case_id,
        history_scope: history_scope_signal,
    } = bindings;

    rsx! {
        section { class: "workspace-shell",
            LeftRail {
                active_tab,
                case_label_text: case_label_text.clone(),
                session_line: session_line.clone(),
                history_scope,
                history_items,
                tab,
                case_id,
                history_scope_signal,
            }
            ConversationPane {
                case_label_text,
                session_line,
                role_badge_text,
                role_badge_class,
                auth_gate_badge_text,
                auth_gate_badge_class,
                conversation,
                action_summaries,
                inserted_contexts,
                on_use_starter: on_start_conversation,
            }
            section { class: "workspace-placeholder workspace-placeholder--right card",
                h3 { "Right Panel Placeholder" }
                p { class: "muted", "Task 4 will implement plan/references/results panel." }
            }
            section { class: "workspace-placeholder workspace-placeholder--canvas card",
                h3 { "Canvas Placeholder" }
                p { class: "muted", "Task 5 will wire timeline/evidence/persons/export canvas overlays." }
            }
        }
    }
}
