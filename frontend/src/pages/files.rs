use crate::{api, models, AppCtx};
use dioxus::prelude::*;

#[component]
pub fn FilesPage() -> Element {
    let ctx = use_context::<AppCtx>();

    let mut page = use_signal(|| 1i64);
    let page_size = use_signal(|| 20i64);
    let mut refresh_tick = use_signal(|| 0u64);
    let mut auto_refresh_scheduled = use_signal(|| false);

    let mut case_id_sig = ctx.case_id;

    let list = use_resource(move || {
        let _ = refresh_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let case_id = ctx.case_id();
        let page = page();
        let page_size = page_size();
        async move {
            if token.trim().is_empty() || case_id.trim().is_empty() {
                return Ok(None);
            }
            api::get_case_files(&base, &token, &case_id, page, page_size)
                .await
                .map(Some)
        }
    });

    // Auto-refresh while any file is processing (lightweight polling).
    use_effect(move || {
        let should_poll = match list() {
            Some(Ok(Some(data))) => data.files.iter().any(|f| f.parse_status == "processing"),
            _ => false,
        };
        if should_poll && !auto_refresh_scheduled() {
            auto_refresh_scheduled.set(true);
            let mut refresh_tick = refresh_tick;
            let mut auto_refresh_scheduled = auto_refresh_scheduled;
            spawn(async move {
                sleep_ms(800).await;
                refresh_tick.set(refresh_tick() + 1);
                auto_refresh_scheduled.set(false);
            });
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

    let run_parse = move |file_id: String, original_name: String| {
        let base = ctx.api_base();
        let token = ctx.token();
        let mut status = ctx.status;
        let refresh_tick = refresh_tick;
        spawn(async move {
            if token.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                return;
            }

            status.set(Some("Parsing...".to_string()));
            if let Err(e) = api::post_parse_file(&base, token.trim(), &file_id).await {
                status.set(Some(e));
                return;
            }

            for _ in 0..80 {
                sleep_ms(250).await;
                match api::get_file_detail(&base, token.trim(), &file_id).await {
                    Ok(detail) => {
                        if detail.parse_status != "processing" {
                            let msg = if detail.parse_status == "done" {
                                format!("Parsed: {original_name}")
                            } else {
                                format!(
                                    "Parse failed: {} ({})",
                                    original_name,
                                    detail.parse_error.unwrap_or_default()
                                )
                            };
                            status.set(Some(msg));
                            break;
                        }
                    }
                    Err(_) => {}
                }
            }

            let mut refresh_tick = refresh_tick;
            refresh_tick.set(refresh_tick() + 1);
        });
    };

    rsx! {
        section { class: "panel",
            header { class: "panel__head",
                h2 { "Files" }
                p { class: "muted", "Browse evidence files, re-parse, download originals and parsed JSON." }
            }

            div { class: "grid",
                div { class: "card",
                    h3 { "Context" }
                    label { "Case ID"
                        input {
                            value: ctx.case_id(),
                            placeholder: "UUID",
                            oninput: move |e| case_id_sig.set(e.value()),
                        }
                    }
                    div { class: "actions",
                        button { class: "btn btn--accent", onclick: move |_| refresh_tick.set(refresh_tick() + 1), "Refresh" }
                    }
                }

                div { class: "card",
                    h3 { "File List" }
                    match list() {
                        None => rsx!{ p { class: "muted", "Loading..." } },
                        Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                        Some(Ok(None)) => rsx!{ p { class: "muted", "Enter token + case_id to load files." } },
                        Some(Ok(Some(data))) => rsx!{
                            FileTable {
                                data: data.clone(),
                                on_download: move |(id, name)| {
                                    run_download(format!("/api/v1/files/{id}/download"), name);
                                },
                                on_download_parsed: move |(id, fallback)| {
                                    run_download(format!("/api/v1/files/{id}/parsed"), fallback);
                                },
                                on_parse: move |(id, name)| {
                                    run_parse(id, name);
                                },
                            }
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
fn FileTable(
    data: models::EvidenceFileListData,
    on_download: EventHandler<(String, String)>,
    on_download_parsed: EventHandler<(String, String)>,
    on_parse: EventHandler<(String, String)>,
) -> Element {
    rsx! {
        div { class: "table table--files",
            div { class: "table__head",
                span { class: "cell cell--name", "Name" }
                span { class: "cell cell--type", "Type" }
                span { class: "cell cell--size", "Size" }
                span { class: "cell cell--status", "Parse" }
                span { class: "cell cell--time", "Uploaded" }
                span { class: "cell cell--act", "" }
            }
            for f in data.files.iter() {
                div { class: "table__row",
                    span { class: "cell cell--name",
                        code { "{f.original_name}" }
                        if let Some(err) = &f.parse_error {
                            if !err.trim().is_empty() && f.parse_status == "failed" {
                                div { class: "muted", "{truncate(err, 120)}" }
                            }
                        }
                    }
                    span { class: "cell cell--type", "{truncate(&f.file_type, 40)}" }
                    span { class: "cell cell--size", "{f.file_size}" }
                    span { class: "cell cell--status",
                        span { class: status_badge_class(&f.parse_status), "{f.parse_status}" }
                    }
                    span { class: "cell cell--time", "{f.created_at}" }
                    span { class: "cell cell--act",
                        button {
                            class: "btn btn--small",
                            onclick: {
                                let id = f.id.clone();
                                let name = f.original_name.clone();
                                move |_| on_download.call((id.clone(), name.clone()))
                            },
                            "Download"
                        }
                        button {
                            class: "btn btn--small btn--ghost",
                            disabled: f.parse_status != "done",
                            onclick: {
                                let id = f.id.clone();
                                let fallback = fallback_parsed_name(&f.original_name);
                                move |_| on_download_parsed.call((id.clone(), fallback.clone()))
                            },
                            "Parsed JSON"
                        }
                        button {
                            class: "btn btn--small btn--accent",
                            disabled: f.parse_status == "processing",
                            onclick: {
                                let id = f.id.clone();
                                let name = f.original_name.clone();
                                move |_| on_parse.call((id.clone(), name.clone()))
                            },
                            "Re-Parse"
                        }
                    }
                }
            }
        }
    }
}

fn status_badge_class(status: &str) -> &'static str {
    match status.trim() {
        "done" => "badge badge--ok",
        "failed" => "badge badge--bad",
        "processing" => "badge badge--run",
        _ => "badge badge--warn",
    }
}

fn fallback_parsed_name(original_name: &str) -> String {
    let mut base = original_name.trim().to_string();
    if let Some(idx) = base.rfind('.') {
        base.truncate(idx);
    }
    let base = base.trim();
    if base.is_empty() {
        "parsed.json".to_string()
    } else {
        format!("{base}.json")
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut out = s.chars().take(max).collect::<String>();
    out.push_str("...");
    out
}

async fn sleep_ms(ms: u32) {
    #[cfg(target_arch = "wasm32")]
    {
        gloo_timers::future::TimeoutFuture::new(ms).await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        tokio::time::sleep(std::time::Duration::from_millis(ms as u64)).await;
    }
}
