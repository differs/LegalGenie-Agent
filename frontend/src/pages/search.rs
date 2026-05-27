use crate::{api, models, AppCtx};
use dioxus::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchLanguageMode {
    Zh,
    Source,
    Bilingual,
}

impl SearchLanguageMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Zh => "zh",
            Self::Source => "source",
            Self::Bilingual => "bilingual",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Zh => "中文",
            Self::Source => "原文",
            Self::Bilingual => "双语",
        }
    }
}

#[component]
pub fn SearchPage() -> Element {
    rsx! { SearchPageContent { utility_mode: false } }
}

#[component]
pub fn SearchPageContent(utility_mode: bool) -> Element {
    let ctx = use_context::<AppCtx>();

    let mut keyword = use_signal(String::new);
    let mut use_case_filter = use_signal(|| true);

    let mut suggest_tick = use_signal(|| 0u64);
    let mut suggest_query = use_signal(String::new);

    let mut ot_case = use_signal(|| true);
    let mut ot_evidence = use_signal(|| true);
    let mut ot_node = use_signal(|| true);
    let mut ot_person = use_signal(|| true);
    let mut language_mode = use_signal(|| SearchLanguageMode::Zh);

    let mut page = use_signal(|| 1i64);
    let page_size = use_signal(|| 20i64);
    let mut refresh_tick = use_signal(|| 0u64);

    let history_page = use_signal(|| 1i64);
    let history_page_size = use_signal(|| 20i64);
    let mut history_refresh = use_signal(|| 0u64);

    let results = use_resource(move || {
        let _ = refresh_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let keyword = keyword();
        let case_id = ctx.case_id();
        let page = page();
        let page_size = page_size();
        let current_language_mode = language_mode();

        let ot_case = ot_case();
        let ot_evidence = ot_evidence();
        let ot_node = ot_node();
        let ot_person = ot_person();
        let use_case_filter = use_case_filter();

        async move {
            if token.trim().is_empty() {
                return Ok(None);
            }
            if keyword.trim().is_empty() {
                return Ok(None);
            }

            let object_types = build_object_types(ot_case, ot_evidence, ot_node, ot_person);
            let case_filter = if use_case_filter && !case_id.trim().is_empty() {
                Some(case_id.trim())
            } else {
                None
            };

            api::get_search_with_language_mode(
                &base,
                token.trim(),
                keyword.trim(),
                object_types.as_deref(),
                case_filter,
                Some(current_language_mode.as_str()),
                page,
                page_size,
            )
            .await
            .map(Some)
        }
    });

    let history = use_resource(move || {
        let _ = history_refresh();
        let base = ctx.api_base();
        let token = ctx.token();
        let page = history_page();
        let page_size = history_page_size();
        async move {
            if token.trim().is_empty() {
                return Ok(None);
            }
            api::get_search_history(&base, token.trim(), page, page_size)
                .await
                .map(Some)
        }
    });

    let suggestions = use_resource(move || {
        let _ = suggest_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let keyword = suggest_query();
        async move {
            if token.trim().is_empty() {
                return Ok(None);
            }
            if keyword.trim().is_empty() {
                return Ok(None);
            }
            api::get_search_suggestions(&base, token.trim(), keyword.trim())
                .await
                .map(Some)
        }
    });

    let on_search = move |_| {
        page.set(1);
        refresh_tick.set(refresh_tick() + 1);
        history_refresh.set(history_refresh() + 1);
    };

    let on_suggest = move |_| {
        suggest_query.set(keyword());
        suggest_tick.set(suggest_tick() + 1);
    };

    let on_clear_history = move |_| {
        let base = ctx.api_base();
        let token = ctx.token();
        let mut status = ctx.status;
        let mut history_refresh = history_refresh;
        spawn(async move {
            if token.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                return;
            }
            status.set(Some("Clearing search history...".to_string()));
            match api::delete_search_history(&base, token.trim()).await {
                Ok(_) => {
                    history_refresh.set(history_refresh() + 1);
                    status.set(Some("Cleared".to_string()));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    rsx! {
        section { class: if utility_mode { "panel panel--embedded" } else { "panel" },
            if !utility_mode {
                header { class: "panel__head",
                    h2 { "Search" }
                    p { class: "muted", "默认中文搜索；需要时可切到原文或双语，并直接识别证据回退与命中语言。" }
                }
            }

            div { class: "grid",
                div { class: "card",
                    h3 { "Query" }
                    label { "Keyword"
                        input {
                            value: keyword(),
                            placeholder: "type keyword...",
                            oninput: move |e| keyword.set(e.value()),
                        }
                    }
                    label { "Scope"
                        div { class: "actions",
                            label { "Cases"
                                input {
                                    r#type: "checkbox",
                                    checked: ot_case(),
                                    onclick: move |_| ot_case.set(!ot_case()),
                                }
                            }
                            label { "Evidence"
                                input {
                                    r#type: "checkbox",
                                    checked: ot_evidence(),
                                    onclick: move |_| ot_evidence.set(!ot_evidence()),
                                }
                            }
                            label { "Nodes"
                                input {
                                    r#type: "checkbox",
                                    checked: ot_node(),
                                    onclick: move |_| ot_node.set(!ot_node()),
                                }
                            }
                            label { "Persons"
                                input {
                                    r#type: "checkbox",
                                    checked: ot_person(),
                                    onclick: move |_| ot_person.set(!ot_person()),
                                }
                            }
                        }
                    }
                    label { "Case filter"
                        div { class: "actions",
                            label { "Only current case"
                                input {
                                    r#type: "checkbox",
                                    checked: use_case_filter(),
                                    onclick: move |_| use_case_filter.set(!use_case_filter()),
                                }
                            }
                            if use_case_filter() {
                                span { class: "muted", "case_id: " code { "{ctx.case_id()}" } }
                            }
                        }
                    }
                    label { "Language mode"
                        div { class: "actions",
                            for mode in [SearchLanguageMode::Zh, SearchLanguageMode::Source, SearchLanguageMode::Bilingual] {
                                button {
                                    class: if mode == language_mode() { "btn btn--accent btn--small" } else { "btn btn--small btn--ghost" },
                                    onclick: move |_| {
                                        language_mode.set(mode);
                                        page.set(1);
                                    },
                                    "{mode.label()}"
                                }
                            }
                        }
                    }
                    div { class: "actions",
                        button { class: "btn btn--accent", onclick: on_search, "Search" }
                        button { class: "btn", onclick: on_suggest, "Suggest" }
                        button { class: "btn btn--ghost", onclick: move |_| { keyword.set(String::new()); }, "Clear" }
                    }

                    if !suggest_query().trim().is_empty() {
                        div {
                            p { class: "muted", "Suggestions (API)" }
                            match suggestions() {
                                None => rsx! { p { class: "muted", "Loading suggestions..." } },
                                Some(Err(e)) => rsx! { p { class: "error", "{e}" } },
                                Some(Ok(None)) => rsx! { p { class: "muted", "No suggestions." } },
                                Some(Ok(Some(s))) => rsx! {
                                    if s.suggestions.is_empty() {
                                        p { class: "muted", "No suggestions." }
                                    } else {
                                        div { class: "actions",
                                            for it in s.suggestions.iter() {
                                                button {
                                                    class: "btn btn--small",
                                                    onclick: {
                                                        let s = it.clone();
                                                        move |_| {
                                                            keyword.set(s.clone());
                                                            page.set(1);
                                                            refresh_tick.set(refresh_tick() + 1);
                                                            history_refresh.set(history_refresh() + 1);
                                                        }
                                                    },
                                                    "{it}"
                                                }
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }

                div { class: "card",
                    h3 { "Results" }
                    match results() {
                        None => rsx! { p { class: "muted", "Enter a keyword and search." } },
                        Some(Err(e)) => rsx! { p { class: "error", "{e}" } },
                        Some(Ok(None)) => rsx! { p { class: "muted", "Enter a keyword and search." } },
                        Some(Ok(Some(data))) => rsx! {
                            if !data.suggestions.is_empty() {
                                div {
                                    p { class: "muted", "Suggestions" }
                                    div { class: "actions",
                                        for s in data.suggestions.iter() {
                                            button {
                                                class: "btn btn--small",
                                                onclick: {
                                                    let s = s.clone();
                                                    move |_| {
                                                        keyword.set(s.clone());
                                                        page.set(1);
                                                        refresh_tick.set(refresh_tick() + 1);
                                                    }
                                                },
                                                "{s}"
                                            }
                                        }
                                    }
                                }
                            }

                            div { class: "table table--search",
                                div { class: "table__head",
                                    span { class: "cell", "Type" }
                                    span { class: "cell", "Match" }
                                    span { class: "cell", "Case" }
                                    span { class: "cell", "Time" }
                                    span { class: "cell", "Score" }
                                }
                                for r in data.results.iter() {
                                    SearchResultRow { result: r.clone() }
                                }
                            }

                            div { class: "pager",
                                button {
                                    class: "btn btn--ghost",
                                    disabled: page() <= 1,
                                    onclick: move |_| page.set((page() - 1).max(1)),
                                    "Prev"
                                }
                                span { class: "muted", "Page {page()}  Size {page_size()}  Mode {language_mode().label()}" }
                                button {
                                    class: "btn btn--ghost",
                                    disabled: (page() * page_size()) >= data.total,
                                    onclick: move |_| page.set(page() + 1),
                                    "Next"
                                }
                            }
                            p { class: "muted", "Total {data.total}" }
                        },
                    }
                }
            }

            div { class: "card card--full",
                h3 { "Search History" }
                div { class: "actions",
                    button { class: "btn btn--ghost", onclick: move |_| history_refresh.set(history_refresh() + 1), "Refresh" }
                    button { class: "btn btn--ghost", onclick: on_clear_history, "Clear History" }
                }
                match history() {
                    None => rsx! { p { class: "muted", "Loading..." } },
                    Some(Err(e)) => rsx! { p { class: "error", "{e}" } },
                    Some(Ok(None)) => rsx! { p { class: "muted", "Login to load history." } },
                    Some(Ok(Some(h))) => rsx! {
                        div { class: "table table--search-history",
                            div { class: "table__head",
                                span { class: "cell", "Keyword" }
                                span { class: "cell", "Types" }
                                span { class: "cell", "Case" }
                                span { class: "cell", "Results" }
                                span { class: "cell", "Time" }
                            }
                            for it in h.items.iter() {
                                div { class: "table__row",
                                    span { class: "cell",
                                        button {
                                            class: "btn btn--small",
                                            onclick: {
                                                let kw = it.keyword.clone();
                                                move |_| {
                                                    keyword.set(kw.clone());
                                                    page.set(1);
                                                    refresh_tick.set(refresh_tick() + 1);
                                                }
                                            },
                                            "{it.keyword}"
                                        }
                                    }
                                    span { class: "cell", "{it.object_types.clone().unwrap_or_default()}" }
                                    span { class: "cell",
                                        if let Some(cid) = &it.case_id { code { "{cid}" } } else { span { class: "muted", "-" } }
                                    }
                                    span { class: "cell", "{it.result_count.unwrap_or(0)}" }
                                    span { class: "cell", "{it.searched_at}" }
                                }
                            }
                        }
                        p { class: "muted", "Total {h.total}" }
                    },
                }
            }
        }
    }
}

#[component]
fn SearchResultRow(result: models::SearchResult) -> Element {
    let is_evidence = result.object_type == "evidence";

    rsx! {
        div { class: "table__row",
            span { class: "cell", span { class: "badge", "{result.object_type}" } }
            span { class: "cell",
                strong { "{result.title}" }
                if let Some(label) = &result.display_label {
                    if !label.trim().is_empty() {
                        div { class: "muted", "{label}" }
                    }
                }
                if is_evidence {
                    div { class: "actions",
                        if let Some(mode) = result.language_mode.as_deref() {
                            span { class: "badge badge--warn", "{language_mode_label(mode)}" }
                        }
                        if let Some(lang) = result.matched_language.as_deref() {
                            span { class: "badge", "{matched_language_label(lang)}" }
                        }
                        if result.source_fallback == Some(true) {
                            span { class: "badge badge--warn", "原文回退" }
                        }
                        if result.translation_incomplete == Some(true) {
                            span { class: "badge badge--warn", "中文索引构建中" }
                        }
                    }
                    if let Some(src) = &result.snippet_source {
                        if !src.trim().is_empty() {
                            div { class: "muted", "原文: {src}" }
                        }
                    }
                    if let Some(zh) = &result.snippet_translated {
                        if !zh.trim().is_empty() {
                            div { class: "muted", "中文: {zh}" }
                        }
                    }
                    if let Some(file_name) = &result.file_name {
                        if !file_name.trim().is_empty() && file_name != &result.title {
                            div { class: "muted", "文件: {file_name}" }
                        }
                    }
                    if let Some(chunk_id) = &result.chunk_id {
                        div { class: "muted", code { "{chunk_id}" } }
                    } else {
                        div { class: "muted", code { "{result.object_id}" } }
                    }
                } else {
                    if let Some(c) = &result.content {
                        if !c.trim().is_empty() {
                            span { class: "muted", "  {c}" }
                        }
                    }
                    div { class: "muted", code { "{result.object_id}" } }
                }
            }
            span { class: "cell",
                if let Some(name) = &result.case_name {
                    span { "{name}" }
                } else {
                    span { class: "muted", "-" }
                }
            }
            span { class: "cell", "{result.created_at}" }
            span { class: "cell", {format!("{:.3}", result.score)} }
        }
    }
}

fn build_object_types(
    case_on: bool,
    evidence_on: bool,
    node_on: bool,
    person_on: bool,
) -> Option<String> {
    let mut out = Vec::new();
    if case_on {
        out.push("case");
    }
    if evidence_on {
        out.push("evidence");
    }
    if node_on {
        out.push("node");
    }
    if person_on {
        out.push("person");
    }
    if out.is_empty() {
        None
    } else {
        Some(out.join(","))
    }
}

fn language_mode_label(mode: &str) -> &'static str {
    match mode {
        "zh" => "中文搜索",
        "source" => "原文搜索",
        "bilingual" => "双语搜索",
        _ => "搜索",
    }
}

fn matched_language_label(language: &str) -> &'static str {
    match language {
        "translated" => "命中中文",
        "source" => "命中原文",
        _ => "命中",
    }
}
