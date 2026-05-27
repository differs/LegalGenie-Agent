use dioxus::prelude::*;

use crate::shell::HistoryScope;

use crate::workspace::view_model::{
    history_scope_label, left_rail_view_model, LeftRailHistoryItemViewModel,
};

#[component]
pub fn LeftRail(
    case_label_text: String,
    session_line: String,
    history_scope: HistoryScope,
    history_items: Vec<LeftRailHistoryItemViewModel>,
    mut case_id: Signal<String>,
    mut case_id_draft: Signal<String>,
    mut history_scope_signal: Signal<HistoryScope>,
) -> Element {
    let scope_vm = left_rail_view_model(history_scope);
    let draft_matches_live = case_id_draft() == case_id();
    let apply_disabled = case_id_draft().trim().is_empty() || draft_matches_live;

    rsx! {
        aside { class: "left-rail",
            div { class: "left-rail__brand",
                div { class: "logo", "LM" }
                div {
                    strong { "LegalGenie Agent" }
                    p { class: "muted", "Conversation-first workspace" }
                }
            }

            section { class: "left-rail__card",
                span { class: "eyebrow", "Case Switcher" }
                strong { "{case_label_text}" }
                p { class: "muted", "选择案件后，计划、引用和 Canvas 都会锁定到当前案件。" }
                input {
                    value: case_id_draft(),
                    placeholder: "Paste case_id (UUID)",
                    oninput: move |e| case_id_draft.set(e.value()),
                }
                div { class: "actions",
                    button {
                        class: "btn btn--accent btn--small",
                        disabled: apply_disabled,
                        onclick: move |_| case_id.set(case_id_draft().trim().to_string()),
                        "Apply"
                    }
                    button {
                        class: "btn btn--ghost btn--small",
                        disabled: draft_matches_live,
                        onclick: move |_| case_id_draft.set(case_id()),
                        "Reset"
                    }
                }
                p { class: "muted", "{session_line}" }
            }

            section { class: "left-rail__card",
                div { class: "left-rail__scope",
                    span { class: "eyebrow", "Conversation History" }
                    span { class: "badge", "{scope_vm.active_scope_label}" }
                }
                div { class: "left-rail__scope-actions",
                    button {
                        class: if history_scope == HistoryScope::CurrentCase {
                            "btn btn--accent btn--small"
                        } else {
                            "btn btn--ghost btn--small"
                        },
                        onclick: move |_| history_scope_signal.set(HistoryScope::CurrentCase),
                        "{history_scope_label(HistoryScope::CurrentCase)}"
                    }
                    button {
                        class: if history_scope == HistoryScope::AllConversations {
                            "btn btn--accent btn--small"
                        } else {
                            "btn btn--ghost btn--small"
                        },
                        onclick: move |_| history_scope_signal.set(HistoryScope::AllConversations),
                        "{history_scope_label(HistoryScope::AllConversations)}"
                    }
                }
                div { class: "left-rail__history",
                    for item in history_items {
                        article { class: "left-rail__history-item",
                            span { class: "eyebrow", "{scope_vm.active_scope_label}" }
                            strong { "{item.title}" }
                            if let Some(case_label) = item.case_label {
                                p { class: "muted", "{case_label}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
