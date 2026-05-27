use dioxus::prelude::*;

use crate::pages::{ExportsPage, FilesPage, PersonsPage, TimelinePage};
use crate::shell::{CanvasKind, SelectedObject};
use crate::workspace::view_model::canvas_context_candidates;

#[component]
pub fn CanvasOverlay(
    canvas: CanvasKind,
    selected_object_id: Option<String>,
    on_select_object: EventHandler<SelectedObject>,
    on_insert_selected: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    let title = match canvas {
        CanvasKind::Timeline => "Timeline Workspace",
        CanvasKind::Evidence => "Evidence Workspace",
        CanvasKind::Persons => "Persons Workspace",
        CanvasKind::Exports => "Exports Workspace",
    };
    let context_candidates = canvas_context_candidates(canvas);

    rsx! {
        section { class: "canvas-overlay card",
            header { class: "canvas-overlay__header",
                div {
                    span { class: "eyebrow", "Deep Research Layer" }
                    h3 { "{title}" }
                }
                button {
                    class: "btn btn--ghost",
                    onclick: move |_| on_close.call(()),
                    "Close"
                }
            }
            section { class: "canvas-overlay__context-strip",
                div { class: "card__topline",
                    h4 { "Bring Into Conversation" }
                    p { class: "muted", "先在这里点选对象，再显式带入主对话。" }
                }
                div { class: "canvas-overlay__context-grid",
                    for candidate in context_candidates {
                        button {
                            class: if selected_object_id.as_deref() == Some(candidate.id) {
                                "quick-entry-card quick-entry-card--active"
                            } else {
                                "quick-entry-card"
                            },
                            onclick: {
                                let selected = SelectedObject {
                                    id: candidate.id.to_string(),
                                    kind: candidate.kind,
                                    title: candidate.title.to_string(),
                                };
                                move |_| on_select_object.call(selected.clone())
                            },
                            strong { "{candidate.title}" }
                            p { class: "muted", "{candidate.summary}" }
                        }
                    }
                }
                div { class: "actions" ,
                    button {
                        class: "btn btn--accent btn--small",
                        disabled: selected_object_id.is_none(),
                        onclick: move |_| on_insert_selected.call(()),
                        "带入对话"
                    }
                }
            }
            div { class: "canvas-overlay__body",
                match canvas {
                    CanvasKind::Timeline => rsx! { TimelinePage { embedded: true } },
                    CanvasKind::Evidence => rsx! { FilesPage { embedded: true } },
                    CanvasKind::Persons => rsx! { PersonsPage { embedded: true } },
                    CanvasKind::Exports => rsx! { ExportsPage { embedded: true } },
                }
            }
            footer { class: "canvas-overlay__footer",
                button {
                    class: "btn btn--accent",
                    onclick: move |_| on_close.call(()),
                    "Back to Conversation"
                }
            }
        }
    }
}
