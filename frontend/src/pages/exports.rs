use crate::{api, models, AppCtx};
use dioxus::prelude::*;

#[component]
pub fn ExportsPage() -> Element {
    let ctx = use_context::<AppCtx>();

    let mut start_date = use_signal(String::new);
    let mut end_date = use_signal(String::new);
    let mut page = use_signal(|| 1i64);
    let page_size = use_signal(|| 20i64);
    let refresh_tick = use_signal(|| 0u64);

    let mut case_id_sig = ctx.case_id;

    let history = use_resource(move || {
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
            api::get_export_history(&base, &token, &case_id, page, page_size)
                .await
                .map(Some)
        }
    });

    let run_export = move |path: String, fallback_name: String| {
        let base = ctx.api_base();
        let token = ctx.token();
        let case_id = ctx.case_id();
        let mut status = ctx.status;
        let refresh_tick = refresh_tick;
        spawn(async move {
            if token.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                return;
            }
            if case_id.trim().is_empty() {
                status.set(Some("Missing case_id".to_string()));
                return;
            }
            status.set(Some("Downloading...".to_string()));
            match api::download_and_save(&base, &token, &path, &fallback_name).await {
                Ok(msg) => {
                    status.set(Some(msg));
                    let mut refresh_tick = refresh_tick;
                    refresh_tick.set(refresh_tick() + 1);
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    let on_export_evidence = {
        let run_export = run_export.clone();
        move |_| {
            let case_id = ctx.case_id();
            run_export(
                format!("/api/v1/cases/{case_id}/exports/evidence-list"),
                "evidence_list.xlsx".to_string(),
            );
        }
    };

    let on_export_timeline_png = {
        let run_export = run_export.clone();
        move |_| {
            let case_id = ctx.case_id();
            let mut q = String::from("format=png");
            if !start_date().trim().is_empty() {
                q.push_str(&format!("&start_date={}", start_date()));
            }
            if !end_date().trim().is_empty() {
                q.push_str(&format!("&end_date={}", end_date()));
            }
            run_export(
                format!("/api/v1/cases/{case_id}/exports/timeline?{q}"),
                "timeline.png".to_string(),
            );
        }
    };

    let on_export_report_pdf = {
        let run_export = run_export.clone();
        move |_| {
            let case_id = ctx.case_id();
            let mut q = String::from("format=pdf");
            if !start_date().trim().is_empty() {
                q.push_str(&format!("&start_date={}", start_date()));
            }
            if !end_date().trim().is_empty() {
                q.push_str(&format!("&end_date={}", end_date()));
            }
            run_export(
                format!("/api/v1/cases/{case_id}/exports/timeline-report?{q}"),
                "timeline_report.pdf".to_string(),
            );
        }
    };

    let on_export_report_html = move |_| {
        let case_id = ctx.case_id();
        let mut q = String::from("format=html");
        if !start_date().trim().is_empty() {
            q.push_str(&format!("&start_date={}", start_date()));
        }
        if !end_date().trim().is_empty() {
            q.push_str(&format!("&end_date={}", end_date()));
        }
        run_export(
            format!("/api/v1/cases/{case_id}/exports/timeline-report?{q}"),
            "timeline_report.html".to_string(),
        );
    };

    rsx! {
        section { class: "panel",
            header { class: "panel__head",
                h2 { "Exports" }
                p { class: "muted", "Generate exports, browse history, download records." }
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
                    div { class: "row",
                        label { "Start date"
                            input {
                                value: start_date(),
                                placeholder: "YYYY-MM-DD",
                                oninput: move |e| start_date.set(e.value()),
                            }
                        }
                        label { "End date"
                            input {
                                value: end_date(),
                                placeholder: "YYYY-MM-DD",
                                oninput: move |e| end_date.set(e.value()),
                            }
                        }
                    }
                    div { class: "actions",
                        button { class: "btn", onclick: on_export_evidence, "Evidence List (XLSX)" }
                        button { class: "btn", onclick: on_export_timeline_png, "Timeline (PNG)" }
                        button { class: "btn btn--accent", onclick: on_export_report_pdf, "Timeline Report (PDF)" }
                        button { class: "btn", onclick: on_export_report_html, "Timeline Report (HTML)" }
                    }
                }

                div { class: "card",
                    h3 { "Export History" }
                    match history() {
                        None => rsx!{ p { class: "muted", "Loading..." } },
                        Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                        Some(Ok(None)) => rsx!{ p { class: "muted", "Enter token + case_id to load history." } },
                        Some(Ok(Some(data))) => rsx!{
                            ExportHistoryTable { data: data.clone(), on_download: move |(id, name)| {
                                let case_id = ctx.case_id();
                                run_export(
                                    format!("/api/v1/cases/{case_id}/exports/{id}/download"),
                                    name,
                                );
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
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ExportHistoryTable(
    data: models::ExportHistoryData,
    on_download: EventHandler<(String, String)>,
) -> Element {
    rsx! {
        div { class: "table",
            div { class: "table__head",
                span { class: "cell cell--type", "Type" }
                span { class: "cell cell--name", "File" }
                span { class: "cell cell--time", "Generated" }
                span { class: "cell cell--size", "Size" }
                span { class: "cell cell--act", "" }
            }
            for rec in data.records.iter() {
                div { class: "table__row",
                    span { class: "cell cell--type",
                        span { class: "badge", "{rec.export_type}" }
                    }
                    span { class: "cell cell--name",
                        code { "{rec.file_name}" }
                    }
                    span { class: "cell cell--time", "{rec.generated_at}" }
                    span { class: "cell cell--size",
                        "{rec.file_size.unwrap_or(0)}"
                    }
                    span { class: "cell cell--act",
                        button {
                            class: "btn btn--small",
                            onclick: {
                                let export_id = rec.id.clone();
                                let file_name = rec.file_name.clone();
                                move |_| on_download.call((export_id.clone(), file_name.clone()))
                            },
                            "Download"
                        }
                    }
                }
            }
        }
        p { class: "muted", "Total {data.total}" }
    }
}
