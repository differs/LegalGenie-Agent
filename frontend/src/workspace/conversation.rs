use dioxus::prelude::*;

use super::context_block::ContextBlock;
use crate::workspace::view_model::{
    ConversationViewModel, InsertedContextViewModel, SuggestedActionSummaryViewModel,
};

#[component]
pub fn ConversationPane(
    case_label_text: String,
    session_line: String,
    role_badge_text: &'static str,
    role_badge_class: &'static str,
    auth_gate_badge_text: &'static str,
    auth_gate_badge_class: &'static str,
    conversation: ConversationViewModel,
    action_summaries: Vec<SuggestedActionSummaryViewModel>,
    inserted_contexts: Vec<InsertedContextViewModel>,
) -> Element {
    rsx! {
        main { class: "conversation-pane",
            header { class: "conversation-pane__header card",
                div {
                    span { class: "eyebrow", "Case Header" }
                    h2 { "{case_label_text}" }
                }
                div { class: "shell__pillbar",
                    span { class: auth_gate_badge_class, "{auth_gate_badge_text}" }
                    span { class: role_badge_class, "{role_badge_text}" }
                    span { class: "badge", "{session_line}" }
                }
            }

            section { class: "card conversation-pane__entries",
                div { class: "card__topline",
                    h3 { "Quick Entry" }
                    p { class: "muted", "{conversation.stage_label}" }
                }
                div { class: "quick-entry-grid",
                    for entry in conversation.quick_entries {
                        article { class: "quick-entry-card",
                            strong { "{entry.label}" }
                        }
                    }
                }
            }

            section { class: "card conversation-pane__actions",
                div { class: "card__topline",
                    h3 { "Suggested Actions" }
                    p { class: "muted", "Auto / ReviewRequired / Guarded" }
                }
                div { class: "queue",
                    for item in action_summaries {
                        article { class: "queue__item",
                            div { class: "queue__meta",
                                span { class: item.badge_class, "{item.badge_text}" }
                            }
                            div { class: "queue__body",
                                strong { "{item.title}" }
                                p { class: "muted", "{item.summary}" }
                            }
                        }
                    }
                }
            }

            if conversation.show_starters {
                section { class: "card conversation-pane__starters",
                    div { class: "card__topline",
                        h3 { "Starter Prompts" }
                    }
                    ul { class: "command-deck__insights",
                        for prompt in conversation.starter_prompts {
                            li { "{prompt}" }
                        }
                    }
                }
            }

            section { class: "card conversation-pane__composer",
                div { class: "card__topline",
                    h3 { "Composer" }
                    p { class: "muted", "Inserted context appears above the input." }
                }
                div { class: "stack-list",
                    for context in inserted_contexts {
                        ContextBlock { view: context }
                    }
                }
                textarea {
                    rows: 4,
                    placeholder: "输入你的问题，或引用时间轴 / 证据 / 人物上下文…",
                }
            }
        }
    }
}
