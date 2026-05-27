use dioxus::prelude::*;

use crate::pages::cases::CasesPage;
use crate::pages::logs::LogsPageContent;
use crate::pages::search::SearchPageContent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UtilitySurface {
    Cases,
    Search,
    Logs,
}

impl UtilitySurface {
    fn id(self) -> &'static str {
        match self {
            UtilitySurface::Cases => "cases",
            UtilitySurface::Search => "search",
            UtilitySurface::Logs => "logs",
        }
    }

    fn label(self) -> &'static str {
        match self {
            UtilitySurface::Cases => "Cases",
            UtilitySurface::Search => "Search",
            UtilitySurface::Logs => "Logs",
        }
    }
}

#[component]
pub fn UtilityLauncher(
    active_utility: Option<UtilitySurface>,
    on_toggle_utility: EventHandler<UtilitySurface>,
) -> Element {
    rsx! {
        section { class: "utility-launcher",
            div {
                span { class: "eyebrow", "Tool Menu" }
                h3 { "Cases, Search And Audit" }
            }
            div { class: "actions",
                for utility in [UtilitySurface::Cases, UtilitySurface::Search, UtilitySurface::Logs] {
                    button {
                        class: if Some(utility) == active_utility {
                            "btn btn--accent btn--small"
                        } else {
                            "btn btn--ghost btn--small"
                        },
                        onclick: move |_| on_toggle_utility.call(utility),
                        "{utility.label()}"
                    }
                }
            }
        }
    }
}

#[component]
pub fn UtilityHost(active_utility: UtilitySurface, on_close: EventHandler<()>) -> Element {
    rsx! {
        section { class: "card utility-host", key: "{active_utility.id()}",
            header { class: "utility-host__header",
                div {
                    span { class: "eyebrow", "Secondary Utility Surface" }
                    h3 { "{active_utility.label()}" }
                }
                button {
                    class: "btn btn--ghost btn--small",
                    onclick: move |_| on_close.call(()),
                    "Close"
                }
            }
            div { class: "utility-host__body",
                match active_utility {
                    UtilitySurface::Cases => rsx! { CasesPage {} },
                    UtilitySurface::Search => rsx! { SearchPageContent { utility_mode: true } },
                    UtilitySurface::Logs => rsx! { LogsPageContent { utility_mode: true } },
                }
            }
        }
    }
}
