use dioxus::prelude::*;

use crate::workspace::view_model::InsertedContextViewModel;

#[component]
pub fn ContextBlock(
    view: InsertedContextViewModel,
    on_toggle: EventHandler<()>,
    on_remove: EventHandler<()>,
) -> Element {
    rsx! {
        article { class: "context-block",
            div { class: "context-block__topline",
                div { class: "context-block__meta",
                    span { class: "eyebrow", "{view.kind_label}" }
                    if view.expanded {
                        span { class: "badge badge--run", "Expanded" }
                    } else {
                        span { class: "badge", "Collapsed" }
                    }
                }
                div { class: "actions",
                    button {
                        class: "btn btn--ghost btn--small",
                        onclick: move |_| on_toggle.call(()),
                        if view.expanded { "收起" } else { "展开" }
                    }
                    button {
                        class: "btn btn--ghost btn--small",
                        onclick: move |_| on_remove.call(()),
                        "移除"
                    }
                }
            }
            strong { "{view.title}" }
            if view.expanded {
                p { class: "muted", "该对象已明确带入主对话，后续追问会围绕它继续展开。" }
            }
        }
    }
}
