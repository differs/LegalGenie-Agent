use dioxus::prelude::*;

use crate::shell::{
    collapse_right_panel, default_right_panel_state, expand_right_panel, switch_right_panel_tab,
    toggle_right_panel_pin, CanvasKind, HistoryScope, PanelMode, SelectedObject, ShellBrief, Tab,
};

use super::conversation::ConversationPane;
use super::left_rail::LeftRail;
use super::right_panel::RightPanel;
use super::utilities::{UtilityHost, UtilityLauncher, UtilitySurface};
use super::view_model::{
    right_panel_surface_view_model, ConversationViewModel, InsertedContextViewModel,
    LeftRailHistoryItemViewModel, SuggestedActionSummaryViewModel,
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
    pub brief: ShellBrief,
    pub conversation: ConversationViewModel,
    pub canvas: Option<CanvasKind>,
    pub selected_object_id: Option<String>,
    pub conversation_items: Vec<String>,
    pub action_summaries: Vec<SuggestedActionSummaryViewModel>,
    pub inserted_contexts: Vec<InsertedContextViewModel>,
    pub composer_draft: String,
    pub active_utility: Option<UtilitySurface>,
}

#[derive(Clone, PartialEq)]
pub struct WorkspaceFrameBindings {
    pub tab: Signal<Tab>,
    pub case_id: Signal<String>,
    pub case_id_draft: Signal<String>,
    pub history_scope: Signal<HistoryScope>,
    pub active_utility: Signal<Option<UtilitySurface>>,
    pub canvas: Signal<Option<CanvasKind>>,
}

#[component]
pub fn WorkspaceFrame(
    view: WorkspaceFrameViewData,
    bindings: WorkspaceFrameBindings,
    on_start_conversation: EventHandler<String>,
    on_change_draft: EventHandler<String>,
    on_submit_message: EventHandler<()>,
    on_open_canvas: EventHandler<CanvasKind>,
    on_select_canvas_object: EventHandler<SelectedObject>,
    on_insert_canvas_object: EventHandler<()>,
    on_toggle_context: EventHandler<String>,
    on_remove_context: EventHandler<String>,
    on_close_canvas: EventHandler<()>,
) -> Element {
    let WorkspaceFrameViewData {
        active_tab: _active_tab,
        session_line,
        case_label_text,
        role_badge_text,
        role_badge_class,
        auth_gate_badge_text,
        auth_gate_badge_class,
        history_scope,
        history_items,
        brief,
        conversation,
        canvas,
        selected_object_id,
        conversation_items,
        action_summaries,
        inserted_contexts,
        composer_draft,
        active_utility,
    } = view;
    let WorkspaceFrameBindings {
        tab: _tab,
        case_id,
        case_id_draft,
        history_scope: history_scope_signal,
        active_utility: mut active_utility_signal,
        canvas: mut canvas_signal,
    } = bindings;
    let mut right_panel_state = use_signal(default_right_panel_state);
    let panel_state = right_panel_state();
    let right_panel_view = right_panel_surface_view_model(
        panel_state.tab,
        panel_state.mode,
        &brief,
        &action_summaries,
        &inserted_contexts,
    );
    let shell_grid_style = if panel_state.mode == PanelMode::Collapsed {
        "grid-template-columns:260px minmax(0, 1fr) 64px;"
    } else {
        "grid-template-columns:260px minmax(0, 1fr) 280px;"
    };

    rsx! {
        section { class: "workspace-shell", style: shell_grid_style,
            LeftRail {
                case_label_text: case_label_text.clone(),
                session_line: session_line.clone(),
                history_scope,
                history_items,
                case_id,
                case_id_draft,
                history_scope_signal,
            }
            section { class: "workspace-center-stack",
                UtilityLauncher {
                    active_utility,
                    on_toggle_utility: move |utility| {
                        if active_utility_signal() == Some(utility) {
                            active_utility_signal.set(None);
                        } else {
                            canvas_signal.set(None);
                            active_utility_signal.set(Some(utility));
                        }
                    },
                }
                if let Some(utility) = active_utility {
                    UtilityHost {
                        active_utility: utility,
                        on_close: move |_| active_utility_signal.set(None),
                    }
                }
                ConversationPane {
                    case_label_text,
                    session_line,
                    role_badge_text,
                    role_badge_class,
                    auth_gate_badge_text,
                    auth_gate_badge_class,
                    conversation,
                    conversation_items,
                    inserted_contexts,
                    composer_draft,
                    canvas,
                    selected_object_id,
                    on_use_starter: on_start_conversation,
                    on_change_draft,
                    on_submit_message,
                    on_open_canvas,
                    on_select_canvas_object,
                    on_insert_canvas_object,
                    on_toggle_context,
                    on_remove_context,
                    on_close_canvas,
                }
            }
            RightPanel {
                view: right_panel_view,
                on_switch_tab: move |tab| {
                    let next = switch_right_panel_tab(right_panel_state(), tab);
                    right_panel_state.set(next);
                },
                on_toggle_collapse: move |_| {
                    let current = right_panel_state();
                    let next = match current.mode {
                        PanelMode::Collapsed => expand_right_panel(current),
                        PanelMode::Expanded | PanelMode::Pinned => collapse_right_panel(current),
                    };
                    right_panel_state.set(next);
                },
                on_toggle_pin: move |_| {
                    let next = toggle_right_panel_pin(right_panel_state());
                    right_panel_state.set(next);
                },
            }
        }
    }
}
