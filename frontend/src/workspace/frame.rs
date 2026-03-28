use dioxus::prelude::*;

use crate::shell::{
    ConversationStage, HistoryScope, ObjectKind, OperatorPanel, ShellAction, ShellBrief,
    ShellState, Tab,
};

use super::view_model::{
    conversation_view_model, inserted_context_view_model, left_rail_history_item,
    suggested_action_summaries,
};

#[path = "context_block.rs"]
mod context_block;
#[path = "conversation.rs"]
mod conversation;
#[path = "left_rail.rs"]
mod left_rail;

use conversation::ConversationPane;
use left_rail::LeftRail;

#[derive(Clone, PartialEq)]
pub struct WorkspaceFrameViewData {
    pub shell_state: ShellState,
    pub brief: ShellBrief,
    pub action_queue: Vec<ShellAction>,
    pub operator_panel: OperatorPanel,
    pub session_line: String,
    pub case_label_text: String,
    pub role_badge_text: &'static str,
    pub role_badge_class: &'static str,
    pub auth_gate_badge_text: &'static str,
    pub auth_gate_badge_class: &'static str,
    pub operator_avatar: String,
}

#[derive(Clone, PartialEq)]
pub struct WorkspaceFrameBindings {
    pub session_verifying: bool,
    pub api_base: Signal<String>,
    pub token_draft: Signal<String>,
    pub case_id: Signal<String>,
    pub tab: Signal<Tab>,
    pub show_devtools: Signal<bool>,
    pub status: Signal<Option<String>>,
}

#[component]
pub fn WorkspaceFrame(
    view: WorkspaceFrameViewData,
    bindings: WorkspaceFrameBindings,
    on_me: EventHandler<MouseEvent>,
    on_logout: EventHandler<MouseEvent>,
    on_apply_token: EventHandler<MouseEvent>,
    on_clear_connection: EventHandler<MouseEvent>,
) -> Element {
    let _ = (&on_me, &on_logout, &on_apply_token, &on_clear_connection);
    let WorkspaceFrameViewData {
        shell_state,
        brief: _,
        action_queue,
        operator_panel: _,
        session_line,
        case_label_text,
        role_badge_text,
        role_badge_class,
        auth_gate_badge_text,
        auth_gate_badge_class,
        operator_avatar: _,
    } = view;
    let WorkspaceFrameBindings {
        session_verifying: _,
        api_base: _,
        token_draft: _,
        case_id: _,
        tab,
        show_devtools: _,
        status: _,
    } = bindings;

    let conversation_stage = if shell_state.has_case {
        ConversationStage::Active
    } else {
        ConversationStage::Empty
    };
    let conversation_vm = conversation_view_model(conversation_stage);

    let history_scope = if shell_state.active_tab == Tab::Cases {
        HistoryScope::AllConversations
    } else {
        HistoryScope::CurrentCase
    };
    let history_items = vec![
        left_rail_history_item("会话 A", history_scope, Some("劳动争议案")),
        left_rail_history_item("会话 B", history_scope, Some("民间借贷案")),
        left_rail_history_item("会话 C", history_scope, Some(&case_label_text)),
    ];

    let inserted_contexts = vec![inserted_context_view_model(
        "付款节点",
        ObjectKind::TimelineNode,
        false,
    )];
    let action_summaries = suggested_action_summaries(&action_queue);

    rsx! {
        section { class: "workspace-shell",
            LeftRail {
                active_tab: shell_state.active_tab,
                case_label_text: case_label_text.clone(),
                session_line: session_line.clone(),
                history_scope,
                history_items,
                tab,
            }
            ConversationPane {
                case_label_text,
                session_line,
                role_badge_text,
                role_badge_class,
                auth_gate_badge_text,
                auth_gate_badge_class,
                conversation: conversation_vm,
                action_summaries,
                inserted_contexts,
            }
            section { class: "workspace-placeholder card",
                h3 { "Right Panel Placeholder" }
                p { class: "muted", "Task 4 will implement plan/references/results panel." }
            }
            section { class: "workspace-placeholder card",
                h3 { "Canvas Placeholder" }
                p { class: "muted", "Task 5 will wire timeline/evidence/persons/export canvas overlays." }
            }
        }
    }
}
