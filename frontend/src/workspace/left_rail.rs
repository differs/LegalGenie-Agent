use dioxus::prelude::*;

use crate::shell::{HistoryScope, Tab};

use crate::workspace::view_model::{
    history_scope_label, left_rail_primary_entries, left_rail_view_model,
    LeftRailHistoryItemViewModel,
};

#[component]
pub fn LeftRail(
    active_tab: Tab,
    case_label_text: String,
    session_line: String,
    history_scope: HistoryScope,
    history_items: Vec<LeftRailHistoryItemViewModel>,
    mut tab: Signal<Tab>,
) -> Element {
    let primary_entries = left_rail_primary_entries();
    let scope_vm = left_rail_view_model(history_scope);

    rsx! {
        aside { class: "left-rail",
            div { class: "left-rail__brand",
                div { class: "logo", "LM" }
                div {
                    strong { "LegalMinds" }
                    p { class: "muted", "Conversation-first workspace" }
                }
            }

            section { class: "left-rail__card",
                span { class: "eyebrow", "Case Switcher" }
                strong { "{case_label_text}" }
                p { class: "muted", "{session_line}" }
            }

            nav { class: "left-rail__nav",
                for item in primary_entries {
                    button {
                        class: if item.tab == active_tab {
                            "navrail__btn navrail__btn--active"
                        } else {
                            "navrail__btn"
                        },
                        onclick: move |_| tab.set(item.tab),
                        "{item.label}"
                    }
                }
            }

            section { class: "left-rail__card",
                div { class: "left-rail__scope",
                    span { class: "eyebrow", "Conversation History" }
                    span { class: "badge", "{scope_vm.active_scope_label}" }
                }
                div { class: "left-rail__scope-actions",
                    button { class: "btn btn--ghost btn--small", "{history_scope_label(HistoryScope::CurrentCase)}" }
                    button { class: "btn btn--ghost btn--small", "{history_scope_label(HistoryScope::AllConversations)}" }
                }
                div { class: "left-rail__history",
                    for item in history_items {
                        article { class: "left-rail__history-item",
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
