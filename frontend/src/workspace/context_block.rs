use dioxus::prelude::*;

use crate::workspace::view_model::InsertedContextViewModel;

#[component]
pub fn ContextBlock(view: InsertedContextViewModel) -> Element {
    rsx! {
        article { class: "context-block",
            div { class: "context-block__meta",
                span { class: "eyebrow", "{view.kind_label}" }
                if view.expanded {
                    span { class: "badge badge--run", "Expanded" }
                } else {
                    span { class: "badge", "Collapsed" }
                }
            }
            strong { "{view.title}" }
        }
    }
}
