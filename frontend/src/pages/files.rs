use crate::{api, models, AppCtx};
use dioxus::prelude::*;
use std::sync::Arc;

#[derive(Clone)]
struct PickedFile {
    name: String,
    bytes: Arc<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileReaderViewMode {
    Bilingual,
    Zh,
    Source,
}

impl FileReaderViewMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Bilingual => "bilingual",
            Self::Zh => "zh",
            Self::Source => "source",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Bilingual => "双语",
            Self::Zh => "中文",
            Self::Source => "原文",
        }
    }
}

fn default_file_reader_view_mode() -> FileReaderViewMode {
    FileReaderViewMode::Bilingual
}

#[derive(Clone, PartialEq)]
struct ReaderData {
    detail: models::EvidenceFileDetail,
    translation: models::EvidenceFileTranslationDetail,
    chunks: Option<models::EvidenceFileChunkListData>,
}

#[component]
pub fn FilesPage(embedded: Option<bool>) -> Element {
    let embedded = embedded.unwrap_or(false);
    let ctx = use_context::<AppCtx>();

    let role_in_case = ctx.case_role();
    let read_only = !matches!(role_in_case.as_deref(), Some("owner") | Some("member"));

    let mut page = use_signal(|| 1i64);
    let page_size = use_signal(|| 20i64);
    let mut refresh_tick = use_signal(|| 0u64);
    let mut auto_refresh_scheduled = use_signal(|| false);

    let mut case_id_sig = ctx.case_id;

    let mut picked_file = use_signal(|| None::<PickedFile>);
    let uploading = use_signal(|| false);

    let mut selected_file_id = use_signal(|| None::<String>);
    let mut reader_page = use_signal(|| 1i64);
    let reader_page_size = 1i64;
    let mut reader_view_mode = use_signal(default_file_reader_view_mode);
    let mut reader_refresh_tick = use_signal(|| 0u64);

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

    let reader = use_resource(move || {
        let _ = refresh_tick();
        let _ = reader_refresh_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let file_id = selected_file_id();
        let chunk_page = reader_page();
        let view_mode = reader_view_mode();
        async move {
            let Some(file_id) = file_id else {
                return Ok::<Option<ReaderData>, String>(None);
            };
            if token.trim().is_empty() {
                return Ok::<Option<ReaderData>, String>(None);
            }

            let detail = api::get_file_detail(&base, token.trim(), &file_id).await?;
            let translation = api::get_file_translation(&base, token.trim(), &file_id).await?;

            let chunks = if detail.parse_status == "done" && translation.translation.chunk_count > 0
            {
                Some(
                    api::get_file_chunks(
                        &base,
                        token.trim(),
                        &file_id,
                        chunk_page,
                        reader_page_size,
                        Some(view_mode.as_str()),
                    )
                    .await?,
                )
            } else {
                None
            };

            Ok(Some(ReaderData {
                detail,
                translation,
                chunks,
            }))
        }
    });

    use_effect(move || {
        let should_poll = match list() {
            Some(Ok(Some(data))) => data.files.iter().any(file_requires_polling),
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

    let on_pick_file = move |e: Event<FormData>| {
        let mut status = ctx.status;
        let mut picked_file = picked_file;

        let files = e.files();
        let Some(file) = files.first().cloned() else {
            picked_file.set(None);
            status.set(Some("No file selected".to_string()));
            return;
        };

        let name = file.name();
        status.set(Some(format!("Reading file: {name}...")));
        spawn(async move {
            match file.read_bytes().await {
                Ok(bytes) => {
                    let bytes = bytes.to_vec();
                    let size = bytes.len();
                    picked_file.set(Some(PickedFile {
                        name: name.clone(),
                        bytes: Arc::new(bytes),
                    }));
                    status.set(Some(format!("Selected: {name} ({size} bytes)")));
                }
                Err(_) => {
                    picked_file.set(None);
                    status.set(Some("Failed to read selected file".to_string()));
                }
            }
        });
    };

    let on_upload = move |_| {
        let base = ctx.api_base();
        let token = ctx.token();
        let case_id = ctx.case_id();
        let picked = picked_file();

        let mut status = ctx.status;
        let mut picked_file = picked_file;
        let mut uploading = uploading;
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
            let Some(picked) = picked else {
                status.set(Some("Pick a file first".to_string()));
                return;
            };
            if uploading() {
                return;
            }

            const MAX_BYTES: usize = 100 * 1024 * 1024;
            if picked.bytes.len() > MAX_BYTES {
                status.set(Some("File too large (max 100MB)".to_string()));
                return;
            }

            uploading.set(true);
            status.set(Some("Uploading...".to_string()));
            match api::post_upload_case_file(
                &base,
                token.trim(),
                case_id.trim(),
                &picked.name,
                &picked.bytes,
            )
            .await
            {
                Ok(detail) => {
                    picked_file.set(None);
                    status.set(Some(format!(
                        "Uploaded: {} ({})",
                        detail.original_name, detail.parse_status
                    )));
                    let mut refresh_tick = refresh_tick;
                    refresh_tick.set(refresh_tick() + 1);
                }
                Err(e) => status.set(Some(e)),
            }
            uploading.set(false);
        });
    };

    let on_clear_pick = move |_| {
        picked_file.set(None);
    };

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
        let mut reader_refresh_tick = reader_refresh_tick;
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
            reader_refresh_tick.set(reader_refresh_tick() + 1);
        });
    };

    let mut open_reader = move |file_id: String| {
        selected_file_id.set(Some(file_id));
        reader_page.set(1);
        reader_view_mode.set(default_file_reader_view_mode());
        reader_refresh_tick.set(reader_refresh_tick() + 1);
    };

    let clear_reader = move |_| {
        selected_file_id.set(None);
    };

    let retry_translation = move |file_id: String| {
        let base = ctx.api_base();
        let token = ctx.token();
        let mut status = ctx.status;
        let mut refresh_tick = refresh_tick;
        let mut reader_refresh_tick = reader_refresh_tick;
        spawn(async move {
            if token.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                return;
            }
            status.set(Some("Retrying translation...".to_string()));
            match api::post_retry_translation(&base, token.trim(), &file_id, "all", None).await {
                Ok(result) => {
                    status.set(Some(format!(
                        "Translation retry queued: retried {}, skipped {}",
                        result.retried_count, result.skipped_count
                    )));
                    refresh_tick.set(refresh_tick() + 1);
                    reader_refresh_tick.set(reader_refresh_tick() + 1);
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    rsx! {
        section { class: if embedded { "panel canvas-embedded" } else { "panel" },
            if !embedded {
                header { class: "panel__head",
                    h2 { "Files" }
                    p { class: "muted", "Upload evidence, inspect bilingual chunks, and retry translation without leaving the file workspace." }
                }
            }

            div { class: "grid",
                div { class: "card",
                    h3 { "Upload" }
                    label { "Case ID"
                        input {
                            value: ctx.case_id(),
                            placeholder: "UUID",
                            oninput: move |e| case_id_sig.set(e.value()),
                        }
                    }
                    label { "File"
                        input {
                            r#type: "file",
                            accept: ".pdf,.doc,.docx,.xls,.xlsx,.jpg,.jpeg,.png,.mp3,.wav,.txt,.json",
                            onchange: on_pick_file,
                        }
                    }
                    if let Some(p) = picked_file() {
                        p { class: "muted",
                            "Selected: "
                            code { "{p.name}" }
                            "  {p.bytes.len()} bytes"
                        }
                    }
                    div { class: "actions",
                        button { class: "btn btn--accent", disabled: uploading() || read_only, onclick: on_upload, "Upload" }
                        button { class: "btn btn--ghost", disabled: uploading(), onclick: on_clear_pick, "Clear" }
                        button { class: "btn btn--ghost", disabled: uploading(), onclick: move |_| refresh_tick.set(refresh_tick() + 1), "Refresh" }
                    }
                    if read_only {
                        p { class: "muted", "当前为只读权限：可以浏览、下载和阅读双语分片，但不能上传、重解析或触发翻译重试。" }
                    }
                }

                div { class: "card",
                    h3 { "File List" }
                    match list() {
                        None => rsx! { p { class: "muted", "Loading..." } },
                        Some(Err(e)) => rsx! { p { class: "error", "{e}" } },
                        Some(Ok(None)) => rsx! { p { class: "muted", "Enter token + case_id to load files." } },
                        Some(Ok(Some(data))) => rsx! {
                            if data.files.is_empty() {
                                p { class: "muted", "No files yet." }
                            } else {
                                FileTable {
                                    data: data.clone(),
                                    can_write: !read_only,
                                    selected_file_id: selected_file_id(),
                                    on_open_reader: move |id| {
                                        open_reader(id);
                                    },
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
                        },
                    }
                }
            }

            if selected_file_id().is_some() {
                div { class: "card card--full",
                    div { class: "actions",
                        h3 { "Reader" }
                        button { class: "btn btn--ghost", onclick: clear_reader, "Close" }
                    }
                    match reader() {
                        None => rsx! { p { class: "muted", "Loading selected file..." } },
                        Some(Err(e)) => rsx! { p { class: "error", "{e}" } },
                        Some(Ok(None)) => rsx! { p { class: "muted", "Select a file to inspect its bilingual chunks." } },
                        Some(Ok(Some(data))) => rsx! {
                            FileReaderCard {
                                data: data,
                                can_write: !read_only,
                                view_mode: reader_view_mode(),
                                current_page: reader_page(),
                                on_set_view_mode: move |mode| {
                                    reader_view_mode.set(mode);
                                    reader_page.set(1);
                                    reader_refresh_tick.set(reader_refresh_tick() + 1);
                                },
                                on_prev_page: move |_| {
                                    reader_page.set((reader_page() - 1).max(1));
                                },
                                on_next_page: move |_| {
                                    reader_page.set(reader_page() + 1);
                                },
                                on_refresh: move |_| {
                                    reader_refresh_tick.set(reader_refresh_tick() + 1);
                                },
                                on_retry_translation: move |file_id| {
                                    retry_translation(file_id);
                                },
                                on_download_original: move |(id, name)| {
                                    run_download(format!("/api/v1/files/{id}/download"), name);
                                },
                                on_download_parsed: move |(id, fallback)| {
                                    run_download(format!("/api/v1/files/{id}/parsed"), fallback);
                                },
                            }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn FileTable(
    data: models::EvidenceFileListData,
    can_write: bool,
    selected_file_id: Option<String>,
    on_open_reader: EventHandler<String>,
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
                span { class: "cell cell--status", "Translate" }
                span { class: "cell cell--time", "Uploaded" }
                span { class: "cell cell--act", "" }
            }
            for f in data.files.iter() {
                div { class: if selected_file_id.as_deref() == Some(f.id.as_str()) { "table__row table__row--selected" } else { "table__row" },
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
                    span { class: "cell cell--status",
                        span { class: status_badge_class(&f.translation.translation_status), "{f.translation.translation_status}" }
                        if f.translation.translation_incomplete {
                            span { class: "muted", "  (building)" }
                        }
                    }
                    span { class: "cell cell--time", "{f.created_at}" }
                    span { class: "cell cell--act",
                        button {
                            class: "btn btn--small",
                            disabled: f.parse_status != "done",
                            onclick: {
                                let id = f.id.clone();
                                move |_| on_open_reader.call(id.clone())
                            },
                            if selected_file_id.as_deref() == Some(f.id.as_str()) { "Reading" } else { "Open Reader" }
                        }
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
                            disabled: !can_write || f.parse_status == "processing",
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

#[component]
fn FileReaderCard(
    data: ReaderData,
    can_write: bool,
    view_mode: FileReaderViewMode,
    current_page: i64,
    on_set_view_mode: EventHandler<FileReaderViewMode>,
    on_prev_page: EventHandler<()>,
    on_next_page: EventHandler<()>,
    on_refresh: EventHandler<()>,
    on_retry_translation: EventHandler<String>,
    on_download_original: EventHandler<(String, String)>,
    on_download_parsed: EventHandler<(String, String)>,
) -> Element {
    let detail = &data.detail;
    let translation = &data.translation.translation;
    let chunk_list = data.chunks.clone();
    let source_language = translation.source_language.as_deref().unwrap_or("-");
    let target_language = translation.target_language.as_deref().unwrap_or("zh-CN");

    let total_pages = chunk_list
        .as_ref()
        .map(|list| ((list.total.max(1) + list.page_size - 1) / list.page_size).max(1))
        .unwrap_or(1);

    rsx! {
        div {
            div { class: "actions",
                div {
                    h4 { "{detail.original_name}" }
                    p { class: "muted", "解析完成后默认进入双语阅读；可切换到中文或原文视图。" }
                }
                div { class: "actions",
                    span { class: status_badge_class(&detail.parse_status), "Parse {detail.parse_status}" }
                    span { class: status_badge_class(&translation.translation_status), "Translate {translation.translation_status}" }
                    button {
                        class: "btn btn--ghost",
                        onclick: {
                            let id = detail.id.clone();
                            let name = detail.original_name.clone();
                            move |_| on_download_original.call((id.clone(), name.clone()))
                        },
                        "Download Original"
                    }
                    button {
                        class: "btn btn--ghost",
                        disabled: detail.parse_status != "done",
                        onclick: {
                            let id = detail.id.clone();
                            let fallback = fallback_parsed_name(&detail.original_name);
                            move |_| on_download_parsed.call((id.clone(), fallback.clone()))
                        },
                        "Parsed JSON"
                    }
                    button {
                        class: "btn btn--ghost",
                        onclick: move |_| on_refresh.call(()),
                        "Refresh"
                    }
                    button {
                        class: "btn btn--accent",
                        disabled: !can_write || detail.parse_status != "done",
                        onclick: {
                            let id = detail.id.clone();
                            move |_| on_retry_translation.call(id.clone())
                        },
                        "Retry Translation"
                    }
                }
            }

            div { class: "actions",
                span { class: "muted", "Source {source_language}" }
                span { class: "muted", "Target {target_language}" }
                span { class: "muted", "Chunks {translation.chunk_count} / Done {translation.translated_chunk_count} / Failed {translation.failed_chunk_count}" }
                if let Some(provider) = &translation.translation_provider {
                    span { class: "muted", "Provider {provider}" }
                }
                if let Some(model) = &translation.translation_model {
                    span { class: "muted", "Model {model}" }
                }
            }

            if translation.translation_incomplete {
                p { class: "muted", "中文索引构建中，可切到原文或双语搜索。" }
            }

            if let Some(err) = &translation.translation_error {
                if !err.trim().is_empty() {
                    p { class: "error", "{err}" }
                }
            }

            div { class: "actions",
                for mode in [FileReaderViewMode::Bilingual, FileReaderViewMode::Zh, FileReaderViewMode::Source] {
                    button {
                        class: if mode == view_mode { "btn btn--accent btn--small" } else { "btn btn--small btn--ghost" },
                        onclick: move |_| on_set_view_mode.call(mode),
                        "{mode.label()}"
                    }
                }
            }

            if detail.parse_status != "done" {
                p { class: "muted", "Parsing is not complete yet. This reader will populate automatically once parsing finishes." }
            } else if translation.chunk_count == 0 {
                div { class: "card",
                    h4 { "Legacy File Fallback" }
                    p { class: "muted", "This file has no bilingual chunks yet. The reader falls back to parsed_text until the file is re-parsed." }
                    if let Some(text) = &detail.parsed_text {
                        pre { "{text}" }
                    } else {
                        p { class: "muted", "No parsed text available." }
                    }
                }
            } else if let Some(chunks) = chunk_list {
                if let Some(chunk) = chunks.items.first() {
                    div { class: "actions",
                        div {
                            strong { "{chunk.display_label}" }
                            span { class: "muted", "  {chunk.chunk_kind}" }
                        }
                        div { class: "pager",
                            button {
                                class: "btn btn--ghost",
                                disabled: current_page <= 1,
                                onclick: move |_| on_prev_page.call(()),
                                "Prev"
                            }
                            span { class: "muted", "Chunk {current_page} / {total_pages}" }
                            button {
                                class: "btn btn--ghost",
                                disabled: current_page >= total_pages,
                                onclick: move |_| on_next_page.call(()),
                                "Next"
                            }
                        }
                    }

                    if let Some(err) = &chunk.translation_error {
                        if !err.trim().is_empty() && chunk.translation_status == "failed" {
                            p { class: "error", "{err}" }
                        }
                    }

                    match view_mode {
                        FileReaderViewMode::Bilingual => rsx! {
                            div { class: "grid",
                                div { class: "card",
                                    h4 { "Original" }
                                    pre { "{chunk.source_text.clone().unwrap_or_default()}" }
                                }
                                div { class: "card",
                                    h4 { "中文" }
                                    if let Some(text) = chunk.translated_text.clone() {
                                        pre { "{text}" }
                                    } else {
                                        p { class: "muted", "{translated_placeholder(chunk)}" }
                                    }
                                }
                            }
                        },
                        FileReaderViewMode::Zh => rsx! {
                            div { class: "card",
                                h4 { "中文" }
                                if let Some(text) = chunk.translated_text.clone() {
                                    pre { "{text}" }
                                } else {
                                    p { class: "muted", "{translated_placeholder(chunk)}" }
                                }
                            }
                        },
                        FileReaderViewMode::Source => rsx! {
                            div { class: "card",
                                h4 { "Original" }
                                pre { "{chunk.source_text.clone().unwrap_or_default()}" }
                            }
                        },
                    }
                } else {
                    p { class: "muted", "No chunk found for this page." }
                }
            } else {
                p { class: "muted", "Loading bilingual chunks..." }
            }
        }
    }
}

fn file_requires_polling(file: &models::EvidenceFileSummary) -> bool {
    file.parse_status == "processing" || file.translation.translation_status == "processing"
}

fn translated_placeholder(chunk: &models::EvidenceFileChunkItem) -> String {
    match chunk.translation_status.trim() {
        "failed" => "翻译失败，可重试".to_string(),
        "processing" | "pending" => "翻译中，稍后刷新".to_string(),
        _ => "暂无中文译文".to_string(),
    }
}

fn status_badge_class(status: &str) -> &'static str {
    match status.trim() {
        "done" => "badge badge--ok",
        "failed" => "badge badge--bad",
        "partial" => "badge badge--warn",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_reader_defaults_to_bilingual_view() {
        assert_eq!(
            default_file_reader_view_mode(),
            FileReaderViewMode::Bilingual
        );
        assert_eq!(FileReaderViewMode::Bilingual.as_str(), "bilingual");
    }
}
