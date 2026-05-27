use crate::{api, AppCtx};
use dioxus::prelude::*;

#[component]
pub fn PersonsPage(embedded: Option<bool>) -> Element {
    let embedded = embedded.unwrap_or(false);
    let ctx = use_context::<AppCtx>();
    let role_in_case = ctx.case_role();
    let read_only = !matches!(role_in_case.as_deref(), Some("owner") | Some("member"));

    let mut page = use_signal(|| 1i64);
    let page_size = use_signal(|| 20i64);
    let mut refresh_tick = use_signal(|| 0u64);
    let mut case_id_sig = ctx.case_id;

    let mut selected = use_signal(|| None::<String>);

    // Create form.
    let mut name = use_signal(String::new);
    let mut gender = use_signal(String::new);
    let mut phone = use_signal(String::new);
    let mut email = use_signal(String::new);
    let mut organization = use_signal(String::new);
    let mut position = use_signal(String::new);
    let mut role_type = use_signal(|| "other".to_string());
    let mut role_detail = use_signal(String::new);
    let mut involved_date = use_signal(String::new);

    // Edit form (uses fetched detail as source of truth, but we keep draft values).
    let mut edit_notes = use_signal(String::new);

    // Link another case to a person.
    let mut link_case_id = use_signal(String::new);
    let mut link_role_type = use_signal(|| "other".to_string());
    let mut link_role_detail = use_signal(String::new);
    let mut link_involved_date = use_signal(String::new);

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
            api::get_case_persons(&base, token.trim(), case_id.trim(), page, page_size)
                .await
                .map(Some)
        }
    });

    let graph = use_resource(move || {
        let base = ctx.api_base();
        let token = ctx.token();
        let case_id = ctx.case_id();
        async move {
            if token.trim().is_empty() || case_id.trim().is_empty() {
                return Ok(None);
            }
            api::get_case_person_graph(&base, token.trim(), case_id.trim())
                .await
                .map(Some)
        }
    });

    let detail = use_resource(move || {
        let _ = refresh_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let person_id = selected();
        async move {
            let Some(pid) = person_id else {
                return Ok(None);
            };
            if token.trim().is_empty() {
                return Ok(None);
            }
            api::get_person_detail(&base, token.trim(), &pid)
                .await
                .map(Some)
        }
    });

    // Keep edit draft in sync when selected person changes.
    use_effect(move || {
        if let Some(Ok(Some(d))) = detail() {
            edit_notes.set(d.notes.clone().unwrap_or_default());
        }
    });

    let on_link_case = move |_| {
        let base = ctx.api_base();
        let token = ctx.token();
        let person_id = selected();
        let case_id = link_case_id();
        let role_type = link_role_type();
        let role_detail = link_role_detail();
        let involved_date = link_involved_date();
        let mut status = ctx.status;
        let mut refresh_tick = refresh_tick;
        let mut link_case_id = link_case_id;
        let mut link_role_detail = link_role_detail;
        let mut link_involved_date = link_involved_date;
        spawn(async move {
            let Some(pid) = person_id else {
                status.set(Some("Select a person first".to_string()));
                return;
            };
            if token.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                return;
            }
            if case_id.trim().is_empty() {
                status.set(Some("case_id required".to_string()));
                return;
            }
            if role_type.trim().is_empty() {
                status.set(Some("role_type required".to_string()));
                return;
            }

            status.set(Some("Linking case...".to_string()));
            match api::post_person_link_case(
                &base,
                token.trim(),
                &pid,
                case_id.trim(),
                role_type.trim(),
                opt_str(&role_detail),
                opt_str(&involved_date),
            )
            .await
            {
                Ok(_) => {
                    link_case_id.set(String::new());
                    link_role_detail.set(String::new());
                    link_involved_date.set(String::new());
                    refresh_tick.set(refresh_tick() + 1);
                    status.set(Some("Linked".to_string()));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    let on_create = move |_| {
        let base = ctx.api_base();
        let token = ctx.token();
        let case_id = ctx.case_id();
        let name_v = name();
        let gender_v = gender();
        let phone_v = phone();
        let email_v = email();
        let org_v = organization();
        let pos_v = position();
        let role_type_v = role_type();
        let role_detail_v = role_detail();
        let involved_date_v = involved_date();
        let mut status = ctx.status;
        let mut refresh_tick = refresh_tick;
        let mut name = name;
        let mut gender = gender;
        let mut phone = phone;
        let mut email = email;
        let mut organization = organization;
        let mut position = position;
        let mut role_type = role_type;
        let mut role_detail = role_detail;
        let mut involved_date = involved_date;
        spawn(async move {
            if token.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                return;
            }
            if case_id.trim().is_empty() {
                status.set(Some("Missing case_id".to_string()));
                return;
            }
            if name_v.trim().is_empty() {
                status.set(Some("name required".to_string()));
                return;
            }

            status.set(Some("Creating person...".to_string()));
            let r = api::post_create_case_person(
                &base,
                token.trim(),
                case_id.trim(),
                name_v.trim(),
                opt_str(&gender_v),
                opt_str(&phone_v),
                opt_str(&email_v),
                opt_str(&org_v),
                opt_str(&pos_v),
                opt_str(&role_type_v),
                opt_str(&role_detail_v),
                opt_str(&involved_date_v),
            )
            .await;

            match r {
                Ok(created) => {
                    refresh_tick.set(refresh_tick() + 1);
                    name.set(String::new());
                    gender.set(String::new());
                    phone.set(String::new());
                    email.set(String::new());
                    organization.set(String::new());
                    position.set(String::new());
                    role_type.set("other".to_string());
                    role_detail.set(String::new());
                    involved_date.set(String::new());
                    status.set(Some(format!("Created person: {}", created.id)));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    let on_update_notes = move |_| {
        let base = ctx.api_base();
        let token = ctx.token();
        let person_id = selected();
        let notes = edit_notes();
        let mut status = ctx.status;
        let mut refresh_tick = refresh_tick;
        spawn(async move {
            let Some(pid) = person_id else {
                status.set(Some("Select a person first".to_string()));
                return;
            };
            if token.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                return;
            }

            status.set(Some("Updating person...".to_string()));
            match api::put_update_person(
                &base,
                token.trim(),
                &pid,
                None,
                None,
                None,
                None,
                None,
                None,
                opt_str(&notes),
            )
            .await
            {
                Ok(_) => {
                    refresh_tick.set(refresh_tick() + 1);
                    status.set(Some("Updated".to_string()));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    let on_delete_selected = move |_| {
        let base = ctx.api_base();
        let token = ctx.token();
        let person_id = selected();
        let mut status = ctx.status;
        let mut refresh_tick = refresh_tick;
        let mut selected = selected;
        spawn(async move {
            let Some(pid) = person_id else {
                status.set(Some("Select a person first".to_string()));
                return;
            };
            if token.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                return;
            }
            status.set(Some("Deleting person...".to_string()));
            match api::delete_person(&base, token.trim(), &pid).await {
                Ok(_) => {
                    selected.set(None);
                    refresh_tick.set(refresh_tick() + 1);
                    status.set(Some("Deleted".to_string()));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    rsx! {
        section { class: if embedded { "panel canvas-embedded" } else { "panel" },
            if !embedded {
                header { class: "panel__head",
                    h2 { "Persons" }
                    p { class: "muted", "Create persons in the case, edit notes, and browse graph nodes." }
                }
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
                        button { class: "btn", onclick: move |_| refresh_tick.set(refresh_tick() + 1), "Refresh" }
                    }
                }

                div { class: "card",
                    h3 { "Create Person" }
                    label { "Name"
                        input { value: name(), placeholder: "Name", oninput: move |e| name.set(e.value()) }
                    }
                    div { class: "row",
                        label { "Role type"
                            input { value: role_type(), placeholder: "plaintiff / defendant / witness ...", oninput: move |e| role_type.set(e.value()) }
                        }
                        label { "Involved date"
                            input { value: involved_date(), placeholder: "YYYY-MM-DD (optional)", oninput: move |e| involved_date.set(e.value()) }
                        }
                    }
                    label { "Role detail"
                        input { value: role_detail(), placeholder: "Optional", oninput: move |e| role_detail.set(e.value()) }
                    }
                    div { class: "row",
                        label { "Gender"
                            input { value: gender(), placeholder: "male/female/unknown", oninput: move |e| gender.set(e.value()) }
                        }
                        label { "Phone"
                            input { value: phone(), placeholder: "Optional", oninput: move |e| phone.set(e.value()) }
                        }
                    }
                    div { class: "row",
                        label { "Email"
                            input { value: email(), placeholder: "Optional", oninput: move |e| email.set(e.value()) }
                        }
                        label { "Organization"
                            input { value: organization(), placeholder: "Optional", oninput: move |e| organization.set(e.value()) }
                        }
                    }
                    label { "Position"
                        input { value: position(), placeholder: "Optional", oninput: move |e| position.set(e.value()) }
                    }
                    div { class: "actions",
                        button { class: "btn btn--accent", disabled: read_only, onclick: on_create, "Create" }
                    }
                    if read_only {
                        p { class: "muted", "当前为只读权限：可以查看人物，但不能创建、编辑或删除。" }
                    }
                }
            }

            div { class: "card card--full",
                h3 { "Case Persons" }
                match list() {
                    None => rsx!{ p { class: "muted", "Loading..." } },
                    Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                    Some(Ok(None)) => rsx!{ p { class: "muted", "Enter token + case_id to load persons." } },
                    Some(Ok(Some(data))) => rsx!{
                        div { class: "table table--persons",
                            div { class: "table__head",
                                span { class: "cell", "Name" }
                                span { class: "cell", "Role" }
                                span { class: "cell", "Contact" }
                                span { class: "cell", "Org" }
                                span { class: "cell", "" }
                            }
                            for p in data.persons.iter() {
                                div { class: "table__row",
                                    span { class: "cell",
                                        strong { "{p.name}" }
                                        div { class: "muted", code { "{p.id}" } }
                                    }
                                    span { class: "cell",
                                        span { class: "badge", "{p.role_type}" }
                                        if let Some(d) = &p.role_detail {
                                            if !d.trim().is_empty() {
                                                span { class: "muted", "  {d}" }
                                            }
                                        }
                                    }
                                    span { class: "cell",
                                        if let Some(ph) = &p.phone { span { "{ph}" } } else { span { class: "muted", "-" } }
                                        if let Some(em) = &p.email { span { class: "muted", "  {em}" } }
                                    }
                                    span { class: "cell",
                                        if let Some(o) = &p.organization { span { "{o}" } } else { span { class: "muted", "-" } }
                                    }
                                    span { class: "cell",
                                        button {
                                            class: if selected().as_deref() == Some(p.id.as_str()) { "btn btn--small btn--accent" } else { "btn btn--small" },
                                            onclick: {
                                                let id = p.id.clone();
                                                move |_| selected.set(Some(id.clone()))
                                            },
                                            "Open"
                                        }
                                    }
                                }
                            }
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

            if let Some(Ok(Some(d))) = detail() {
                div { class: "card card--full",
                    h3 { "Person Detail" }
                    p { class: "muted", "id={d.id}  status={d.status}  updated={d.updated_at}" }
                    p { "{d.name}" }

                    label { "Notes"
                        textarea {
                            value: edit_notes(),
                            placeholder: "Notes (editable)",
                            oninput: move |e| edit_notes.set(e.value()),
                            rows: 4,
                        }
                    }
                    div { class: "actions",
                        button { class: "btn btn--accent", disabled: read_only, onclick: on_update_notes, "Save Notes" }
                        button { class: "btn btn--ghost", disabled: read_only, onclick: on_delete_selected, "Delete" }
                        button { class: "btn btn--ghost", onclick: move |_| selected.set(None), "Close" }
                    }

                    h4 { "Linked Cases" }
                    if d.cases.is_empty() {
                        p { class: "muted", "No linked cases visible." }
                    } else {
                        for c in d.cases.iter() {
                            div { class: "history",
                                div { class: "history__meta",
                                    span { class: "badge", "{c.role_type}" }
                                    code { "{c.case_id}" }
                                    span { class: "muted", "{c.created_at}" }
                                }
                                if let Some(rd) = &c.role_detail {
                                    if !rd.trim().is_empty() {
                                        p { class: "muted", "{rd}" }
                                    }
                                }
                            }
                        }
                    }

                    h4 { "Link Another Case" }
                    label { "Case ID"
                        input {
                            value: link_case_id(),
                            placeholder: "UUID",
                            oninput: move |e| link_case_id.set(e.value()),
                        }
                    }
                    div { class: "row",
                        label { "Role type"
                            input {
                                value: link_role_type(),
                                placeholder: "plaintiff / defendant / witness ...",
                                oninput: move |e| link_role_type.set(e.value()),
                            }
                        }
                        label { "Involved date"
                            input {
                                value: link_involved_date(),
                                placeholder: "YYYY-MM-DD (optional)",
                                oninput: move |e| link_involved_date.set(e.value()),
                            }
                        }
                    }
                    label { "Role detail"
                        input {
                            value: link_role_detail(),
                            placeholder: "Optional",
                            oninput: move |e| link_role_detail.set(e.value()),
                        }
                    }
                    div { class: "actions",
                        button { class: "btn", disabled: read_only, onclick: on_link_case, "Link Case" }
                    }
                }
            }

            div { class: "card card--full",
                h3 { "Graph (Minimal)" }
                match graph() {
                    None => rsx!{ p { class: "muted", "Loading..." } },
                    Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                    Some(Ok(None)) => rsx!{ p { class: "muted", "Enter token + case_id to load graph." } },
                    Some(Ok(Some(g))) => rsx!{
                        p { class: "muted", "Nodes: {g.nodes.len()}  Edges: {g.edges.len()}" }
                        for n in g.nodes.iter() {
                            div { class: "history",
                                div { class: "history__meta",
                                    span { class: "badge", "{n.role_type}" }
                                    span { "{n.name}" }
                                    code { "{n.id}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn opt_str(raw: &str) -> Option<&str> {
    let s = raw.trim();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
