use dioxus::prelude::*;

use super::canvas::CanvasOverlay;
use super::context_block::ContextBlock;
use crate::shell::{CanvasKind, SelectedObject};
use crate::workspace::view_model::{ConversationViewModel, InsertedContextViewModel};

#[component]
pub fn ConversationPane(
    case_label_text: String,
    session_line: String,
    role_badge_text: &'static str,
    role_badge_class: &'static str,
    auth_gate_badge_text: &'static str,
    auth_gate_badge_class: &'static str,
    conversation: ConversationViewModel,
    conversation_items: Vec<String>,
    inserted_contexts: Vec<InsertedContextViewModel>,
    composer_draft: String,
    canvas: Option<CanvasKind>,
    selected_object_id: Option<String>,
    on_use_starter: EventHandler<String>,
    on_change_draft: EventHandler<String>,
    on_submit_message: EventHandler<()>,
    on_open_canvas: EventHandler<CanvasKind>,
    on_select_canvas_object: EventHandler<SelectedObject>,
    on_insert_canvas_object: EventHandler<()>,
    on_toggle_context: EventHandler<String>,
    on_remove_context: EventHandler<String>,
    on_close_canvas: EventHandler<()>,
) -> Element {
    let composer_ready = !composer_draft.trim().is_empty();
    rsx! {
        main { class: "conversation-pane",
            header { class: "conversation-pane__header card",
                div { class: "conversation-pane__header-copy",
                    span { class: "eyebrow", "Active Case" }
                    h2 { "{case_label_text}" }
                    p { class: "muted", "{conversation.hero_summary}" }
                }
                div { class: "shell__pillbar",
                    span { class: auth_gate_badge_class, "{auth_gate_badge_text}" }
                    span { class: role_badge_class, "{role_badge_text}" }
                    span { class: "badge", "{session_line}" }
                }
            }

            section { class: "card conversation-pane__hero",
                div { class: "conversation-pane__hero-topline",
                    div {
                        span { class: "eyebrow", "{conversation.stage_label}" }
                        h3 { "{conversation.hero_title}" }
                    }
                    p { class: "muted", "{conversation.composer_hint}" }
                }
                div { class: "conversation-pane__surface-row" ,
                    for entry in conversation.quick_entries {
                        if let Some(canvas_kind) = quick_entry_canvas_kind(entry.id) {
                            button {
                                class: "quick-entry-card",
                                onclick: move |_| on_open_canvas.call(canvas_kind),
                                span { class: "eyebrow", "{entry.eyebrow}" }
                                strong { "{entry.label}" }
                                p { class: "muted", "{entry.description}" }
                            }
                        } else {
                            article { class: "quick-entry-card",
                                span { class: "eyebrow", "{entry.eyebrow}" }
                                strong { "{entry.label}" }
                                p { class: "muted", "{entry.description}" }
                            }
                        }
                    }
                }
                if conversation.show_starters {
                    div { class: "conversation-pane__starters-grid",
                        for prompt in conversation.starter_prompts {
                            button {
                                class: "prompt-card",
                                onclick: {
                                    let prompt_text = prompt.title.to_string();
                                    move |_| on_use_starter.call(prompt_text.clone())
                                },
                                strong { "{prompt.title}" }
                                p { class: "muted", "{prompt.description}" }
                            }
                        }
                    }
                }
            }

            section { class: "card conversation-pane__history-shell",
                div { class: "card__topline",
                    h3 { "Conversation History" }
                    p { class: "muted", "Local runtime only (no backend persistence)." }
                }
                div { class: "conversation-pane__history" ,
                    if conversation_items.is_empty() {
                        div { class: "conversation-pane__empty-state",
                            span { class: "eyebrow", "No Messages Yet" }
                            h4 { "先发起一个问题，再由 AI 组织计划、引用和工作层。" }
                            p { class: "muted", "你可以直接输入案件问题，或者先点上面的起手问题和 Canvas 入口。" }
                        }
                    } else {
                        div { class: "queue queue--conversation",
                            for (idx, item) in conversation_items.iter().enumerate() {
                                article {
                                    class: if idx % 2 == 0 {
                                        "queue__item queue__item--conversation queue__item--user"
                                    } else {
                                        "queue__item queue__item--conversation queue__item--assistant"
                                    },
                                    div { class: "queue__meta",
                                        span { class: "badge", if idx % 2 == 0 { "User" } else { "Workspace" } }
                                    }
                                    div { class: "queue__body",
                                        p { "{item}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(canvas_kind) = canvas {
                CanvasOverlay {
                    canvas: canvas_kind,
                    selected_object_id,
                    on_select_object: on_select_canvas_object,
                    on_insert_selected: on_insert_canvas_object,
                    on_close: on_close_canvas,
                }
            }

            section { class: "card conversation-pane__composer",
                div { class: "card__topline",
                    div {
                        h3 { "{conversation.composer_label}" }
                        p { class: "muted", "{conversation.composer_hint}" }
                    }
                    span { class: "badge", "Context persists until removed" }
                }
                if !inserted_contexts.is_empty() {
                    div { class: "stack-list" ,
                    for context in inserted_contexts {
                        ContextBlock {
                            view: context.clone(),
                            on_toggle: {
                                let id = context.id.clone();
                                move |_| on_toggle_context.call(id.clone())
                            },
                            on_remove: {
                                let id = context.id.clone();
                                move |_| on_remove_context.call(id.clone())
                            },
                        }
                    }
                }
                }
                textarea {
                    value: composer_draft,
                    rows: 4,
                    placeholder: "输入你的问题，或引用时间轴 / 证据 / 人物上下文…",
                    oninput: move |e| on_change_draft.call(e.value()),
                }
                div { class: "conversation-pane__composer-actions",
                    p { class: "muted", "登录后主对话始终可用；打开 Canvas 时这里不会消失。" }
                    button {
                        class: "btn btn--accent",
                        disabled: !composer_ready,
                        onclick: move |_| on_submit_message.call(()),
                        "发送到主对话"
                    }
                }
            }
        }
    }
}

fn quick_entry_canvas_kind(id: &str) -> Option<CanvasKind> {
    match id {
        "timeline" => Some(CanvasKind::Timeline),
        "evidence" => Some(CanvasKind::Evidence),
        "persons" => Some(CanvasKind::Persons),
        "exports" => Some(CanvasKind::Exports),
        _ => None,
    }
}
