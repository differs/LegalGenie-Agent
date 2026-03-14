use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
fn main() {
    dioxus_web::launch::launch_cfg(App, dioxus_web::Config::default());
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let cfg = dioxus_desktop::Config::new().with_window(
        dioxus_desktop::WindowBuilder::new()
            .with_title("LegalMinds")
            .with_inner_size(dioxus_desktop::LogicalSize::new(1200.0, 800.0)),
    );
    dioxus_desktop::launch::launch(App, Vec::new(), cfg);
}

#[component]
fn App() -> Element {
    rsx! {
        main { style: "font-family: ui-sans-serif, system-ui, -apple-system; padding: 24px; max-width: 960px; margin: 0 auto;",
            h1 { style: "margin: 0 0 8px 0;", "LegalMinds" }
            p { style: "margin: 0 0 16px 0; color: #334155;",
                "Dioxus frontend scaffold (web + desktop)."
            }
            p { style: "margin: 0; color: #475569;",
                "Next: wire routes + API client; backend health is at ",
                code { "/api/v1/health" }
            }
        }
    }
}
