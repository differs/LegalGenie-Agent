use dioxus::prelude::*;

use crate::shell::{PanelMode, RightPanelTab};

use super::view_model::RightPanelSurfaceViewModel;

#[component]
pub fn RightPanel(
    view: RightPanelSurfaceViewModel,
    on_switch_tab: EventHandler<RightPanelTab>,
    on_toggle_collapse: EventHandler<()>,
    on_toggle_pin: EventHandler<()>,
) -> Element {
    rsx! {
        section { class: "workspace-placeholder workspace-placeholder--right card right-panel-surface",
            div { class: "card__topline",
                h3 { "Research Surface" }
                div { class: "shell__pillbar",
                    button {
                        class: "btn btn--ghost btn--small",
                        onclick: move |_| on_toggle_pin.call(()),
                        "{view.pin_label}"
                    }
                    button {
                        class: "btn btn--ghost btn--small",
                        onclick: move |_| on_toggle_collapse.call(()),
                        "{view.collapse_label}"
                    }
                }
            }

            if view.mode == PanelMode::Collapsed {
                p { class: "muted", "{right_panel_tab_label(view.active_tab)}" }
            } else {
                nav { class: "right-panel-surface__tabs",
                    for item in view.tabs {
                        button {
                            class: if item.tab == view.active_tab {
                                "btn btn--accent btn--small"
                            } else {
                                "btn btn--ghost btn--small"
                            },
                            onclick: {
                                let tab = item.tab;
                                move |_| on_switch_tab.call(tab)
                            },
                            "{item.label}"
                        }
                    }
                }
                div { class: "card__topline",
                    h4 { "{view.section_title}" }
                    p { class: "muted", "{view.section_hint}" }
                }
                if view.items.is_empty() {
                    p { class: "muted", "{view.empty_text}" }
                } else {
                    div { class: "stack-list right-panel-surface__items",
                        for item in view.items {
                            article { class: "right-panel-item",
                                div { class: "right-panel-item__meta",
                                    span { class: "eyebrow", "{item.eyebrow}" }
                                    if let Some(badge) = item.badge_text {
                                        span { class: item.badge_class, "{badge}" }
                                    }
                                }
                                strong { "{item.title}" }
                                if let Some(summary) = item.summary {
                                    p { class: "muted", "{summary}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn right_panel_tab_label(tab: RightPanelTab) -> &'static str {
    match tab {
        RightPanelTab::Plan => "Plan",
        RightPanelTab::References => "References",
        RightPanelTab::Results => "Results",
    }
}
