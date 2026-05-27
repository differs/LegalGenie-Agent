use crate::{api, models, AppCtx};
use dioxus::prelude::*;

#[component]
pub fn LogsPage() -> Element {
    rsx! { LogsPageContent { utility_mode: false } }
}

#[component]
pub fn LogsPageContent(utility_mode: bool) -> Element {
    let ctx = use_context::<AppCtx>();
    let initial_case_filter = ctx.case_id();

    let mut action = use_signal(String::new);
    let mut module = use_signal(String::new);
    let mut keyword = use_signal(String::new);
    let mut page = use_signal(|| 1i64);
    let page_size = use_signal(|| 20i64);
    let mut refresh_tick = use_signal(|| 0u64);

    let mut selected_target = use_signal(|| None::<(String, String)>);
    let mut case_filter = use_signal(move || initial_case_filter);

    let logs = use_resource(move || {
        let _ = refresh_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let case_id = case_filter();
        let page = page();
        let page_size = page_size();
        let action = action();
        let module = module();
        let keyword = keyword();
        async move {
            if token.trim().is_empty() {
                return Ok(None);
            }

            let mut parts = Vec::new();
            parts.push(format!("page={page}"));
            parts.push(format!("page_size={page_size}"));

            if !case_id.trim().is_empty() {
                parts.push(format!("case_id={}", urlencoding::encode(case_id.trim())));
            }
            if !action.trim().is_empty() {
                parts.push(format!(
                    "action={}",
                    urlencoding::encode(action.trim().to_ascii_uppercase().as_str())
                ));
            }
            if !module.trim().is_empty() {
                parts.push(format!(
                    "module={}",
                    urlencoding::encode(module.trim().to_ascii_lowercase().as_str())
                ));
            }
            if !keyword.trim().is_empty() {
                parts.push(format!("keyword={}", urlencoding::encode(keyword.trim())));
            }

            let query = parts.join("&");
            api::get_logs(&base, &token, &query).await.map(Some)
        }
    });

    let history = use_resource(move || {
        let base = ctx.api_base();
        let token = ctx.token();
        let selected = selected_target();
        async move {
            let Some((tt, tid)) = selected else {
                return Ok(None);
            };
            if token.trim().is_empty() {
                return Ok(None);
            }
            api::get_target_history(&base, &token, &tt, &tid)
                .await
                .map(Some)
        }
    });

    let run_download = move |path: String, fallback_name: String| {
        let base = ctx.api_base();
        let token = ctx.token();
        let mut status = ctx.status;
        spawn(async move {
            if token.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                return;
            }
            status.set(Some("Downloading...".to_string()));
            match api::download_and_save(&base, &token, &path, &fallback_name).await {
                Ok(msg) => status.set(Some(msg)),
                Err(e) => status.set(Some(e)),
            }
        });
    };

    let on_export_csv = {
        let run_download = run_download.clone();
        move |_| {
            let case_id = case_filter();
            let mut q = String::new();
            if !case_id.trim().is_empty() {
                q.push_str(&format!("case_id={}", urlencoding::encode(case_id.trim())));
            }
            let path = if q.is_empty() {
                "/api/v1/logs/export".to_string()
            } else {
                format!("/api/v1/logs/export?{q}")
            };
            run_download(path, "operation_logs.csv".to_string());
        }
    };

    let on_export_excel = move |_| {
        let case_id = case_filter();
        let mut parts = vec!["format=excel".to_string()];
        if !case_id.trim().is_empty() {
            parts.push(format!("case_id={}", urlencoding::encode(case_id.trim())));
        }
        run_download(
            format!("/api/v1/logs/export?{}", parts.join("&")),
            "operation_logs.xlsx".to_string(),
        );
    };

    use_effect(move || {
        let _ = case_filter();
        let _ = action();
        let _ = module();
        let _ = keyword();
        let _ = page();
        let _ = page_size();
        let _ = refresh_tick();
        selected_target.set(None);
    });

    rsx! {
        section { class: if utility_mode { "panel panel--embedded" } else { "panel" },
            if !utility_mode {
                header { class: "panel__head",
                    h2 { "Operation Logs" }
                    p { class: "muted", "Search logs, export to CSV/Excel, view target history." }
                }
            }

            div { class: "grid",
                div { class: "card",
                    h3 { "Filters" }
                    label { "Case ID"
                        input {
                            value: case_filter(),
                            placeholder: "UUID (optional)",
                            oninput: move |e| case_filter.set(e.value()),
                        }
                    }
                    div { class: "row",
                        label { "Action"
                            input {
                                value: action(),
                                placeholder: "CREATE / UPDATE / EXPORT ...",
                                oninput: move |e| action.set(e.value()),
                            }
                        }
                        label { "Module"
                            input {
                                value: module(),
                                placeholder: "file / node / export ...",
                                oninput: move |e| module.set(e.value()),
                            }
                        }
                    }
                    label { "Keyword"
                        input {
                            value: keyword(),
                            placeholder: "target title, module, action...",
                            oninput: move |e| keyword.set(e.value()),
                        }
                    }
                    div { class: "actions",
                        button { class: "btn", onclick: move |_| refresh_tick.set(refresh_tick() + 1), "Search" }
                        button { class: "btn btn--ghost", onclick: on_export_csv, "Export CSV" }
                        button { class: "btn btn--accent", onclick: on_export_excel, "Export Excel" }
                    }
                }

                div { class: "card",
                    h3 { "Log List" }
                    match logs() {
                        None => rsx!{ p { class: "muted", "Loading..." } },
                        Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                        Some(Ok(None)) => rsx!{ p { class: "muted", "Login to load logs." } },
                        Some(Ok(Some(data))) => rsx!{
                            LogsTable { data: data.clone(), on_history: move |(tt, tid)| {
                                selected_target.set(Some((tt, tid)));
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

            div { class: "card card--full",
                h3 { "Target History" }
                match history() {
                    None => rsx!{ p { class: "muted", "Select a row to view history." } },
                    Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                    Some(Ok(None)) => rsx!{ p { class: "muted", "Select a row to view history." } },
                    Some(Ok(Some(h))) => rsx!{
                        p { class: "muted", "Target: {h.target_type} / {h.target_id}" }
                        for item in h.history.iter() {
                            div { class: "history",
                                div { class: "history__meta",
                                    span { class: "badge", "{item.action}" }
                                    span { "{item.user_name}" }
                                    span { class: "muted", "{item.created_at}" }
                                }
                                if let Some(changes) = &item.changes {
                                    pre { class: "history__changes", "{serde_json::to_string_pretty(changes).unwrap_or_default()}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn LogsTable(data: models::LogsListData, on_history: EventHandler<(String, String)>) -> Element {
    rsx! {
        div { class: "table table--logs",
            div { class: "table__head",
                span { class: "cell cell--time", "Time" }
                span { class: "cell cell--user", "User" }
                span { class: "cell cell--act", "Action" }
                span { class: "cell cell--mod", "Module" }
                span { class: "cell cell--tgt", "Target" }
                span { class: "cell cell--act2", "" }
            }
            for log in data.logs.iter() {
                div { class: "table__row",
                    span { class: "cell cell--time", "{log.created_at}" }
                    span { class: "cell cell--user", "{log.user_name}" }
                    span { class: "cell cell--act", span { class: "badge", "{log.action}" } }
                    span { class: "cell cell--mod", "{log.module}" }
                    span { class: "cell cell--tgt",
                        code { "{log.target_type}" }
                        if let Some(t) = &log.target_title {
                            span { class: "muted", "  {t}" }
                        }
                    }
                    span { class: "cell cell--act2",
                        if let Some(tid) = &log.target_id {
                            button {
                                class: "btn btn--small",
                                onclick: {
                                    let target_type = log.target_type.clone();
                                    let target_id = tid.clone();
                                    move |_| on_history.call((target_type.clone(), target_id.clone()))
                                },
                                "History"
                            }
                        }
                    }
                }
            }
        }
    }
}
