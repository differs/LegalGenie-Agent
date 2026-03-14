use crate::{api, models, AppCtx};
use dioxus::prelude::*;

#[component]
pub fn CasesPage() -> Element {
    let ctx = use_context::<AppCtx>();

    let mut case_name = use_signal(String::new);
    let mut case_desc = use_signal(String::new);

    let mut page = use_signal(|| 1i64);
    let page_size = use_signal(|| 20i64);
    let mut refresh_tick = use_signal(|| 0u64);

    let mut case_id_sig = ctx.case_id;
    let mut status_sig = ctx.status;

    let list = use_resource(move || {
        let _ = refresh_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let page = page();
        let page_size = page_size();
        async move {
            if token.trim().is_empty() {
                return Ok(None);
            }
            api::get_cases(&base, &token, page, page_size)
                .await
                .map(Some)
        }
    });

    let on_create = move |_| {
        let base = ctx.api_base();
        let token = ctx.token();
        let name = case_name();
        let desc = case_desc();
        let mut status = ctx.status;
        let mut case_id = ctx.case_id;
        let mut case_name = case_name;
        let mut case_desc = case_desc;
        let mut refresh_tick = refresh_tick;
        spawn(async move {
            if token.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                return;
            }
            if name.trim().is_empty() {
                status.set(Some("Case name required".to_string()));
                return;
            }

            let desc_opt = if desc.trim().is_empty() {
                None
            } else {
                Some(desc.trim().to_string())
            };

            status.set(Some("Creating case...".to_string()));
            match api::post_create_case(&base, &token, name.trim(), desc_opt.as_deref()).await {
                Ok(created) => {
                    case_id.set(created.id.clone());
                    case_name.set(String::new());
                    case_desc.set(String::new());
                    refresh_tick.set(refresh_tick() + 1);
                    status.set(Some(format!("Created and selected case: {}", created.id)));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    rsx! {
        section { class: "panel",
            header { class: "panel__head",
                h2 { "Cases" }
                p { class: "muted", "Create a case, browse your cases, select one for exports and logs." }
            }

            div { class: "grid",
                div { class: "card",
                    h3 { "Create Case" }
                    label { "Name"
                        input {
                            value: case_name(),
                            placeholder: "Case name",
                            oninput: move |e| case_name.set(e.value()),
                        }
                    }
                    label { "Description"
                        textarea {
                            value: case_desc(),
                            placeholder: "Optional description",
                            oninput: move |e| case_desc.set(e.value()),
                            rows: 3,
                        }
                    }
                    div { class: "actions",
                        button { class: "btn btn--accent", onclick: on_create, "Create" }
                        button { class: "btn btn--ghost", onclick: move |_| refresh_tick.set(refresh_tick() + 1), "Refresh" }
                    }
                    if !ctx.case_id().trim().is_empty() {
                        p { class: "muted",
                            "Selected case_id: "
                            code { "{ctx.case_id()}" }
                        }
                    }
                }

                div { class: "card",
                    h3 { "Case List" }
                    match list() {
                        None => rsx!{ p { class: "muted", "Loading..." } },
                        Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                        Some(Ok(None)) => rsx!{ p { class: "muted", "Login to load cases." } },
                        Some(Ok(Some(data))) => rsx!{
                            CasesTable { data: data.clone(), on_select: move |id: String| {
                                case_id_sig.set(id.clone());
                                status_sig.set(Some(format!("Selected case: {id}")));
                            }}
                            div { class: "pager",
                                button {
                                    class: "btn btn--ghost",
                                    disabled: page() <= 1,
                                    onclick: move |_| page.set((page() - 1).max(1)),
                                    "Prev"
                                }
                                span { class: "muted", "Page {page()}  Size {page_size()}" }
                                button {
                                    class: "btn btn--ghost",
                                    disabled: (page() * page_size()) >= data.total,
                                    onclick: move |_| page.set(page() + 1),
                                    "Next"
                                }
                            }
                            p { class: "muted", "Total {data.total}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn CasesTable(data: models::CaseListData, on_select: EventHandler<String>) -> Element {
    rsx! {
        div { class: "table table--cases",
            div { class: "table__head",
                span { class: "cell cell--name", "Name" }
                span { class: "cell cell--status", "Status" }
                span { class: "cell cell--counts", "Counts" }
                span { class: "cell cell--time", "Created" }
                span { class: "cell cell--act", "" }
            }
            for c in data.cases.iter() {
                div { class: "table__row",
                    span { class: "cell cell--name",
                        strong { "{c.name}" }
                        if let Some(desc) = &c.description {
                            if !desc.trim().is_empty() {
                                span { class: "muted", "  {desc}" }
                            }
                        }
                        div { class: "muted",
                            code { "{c.id}" }
                        }
                    }
                    span { class: "cell cell--status",
                        span { class: "badge", "{c.status}" }
                    }
                    span { class: "cell cell--counts",
                        span { class: "muted", "members {c.member_count}" }
                        span { class: "muted", "  files {c.evidence_count}" }
                        span { class: "muted", "  nodes {c.node_count}" }
                    }
                    span { class: "cell cell--time", "{c.created_at}" }
                    span { class: "cell cell--act",
                        button {
                            class: "btn btn--small",
                            onclick: {
                                let id = c.id.clone();
                                move |_| on_select.call(id.clone())
                            },
                            "Use"
                        }
                    }
                }
            }
        }
    }
}
