use dioxus::prelude::*;
use crate::{
    pages,
    shell::{
        risk_class, risk_label, tab_description, tab_label, ActionIntent, OperatorPanel,
        ShellAction, ShellBrief, ShellState, Tab,
    },
};
use super::view_model::{
    conversation_view_model, left_rail_history_item, left_rail_view_model_default,
    primary_nav_tabs, utility_entries_view_model, QuickEntryCardViewModel,
};

#[derive(Clone, PartialEq)]
pub struct WorkspaceFrameViewData {
    pub shell_state: ShellState,
    pub brief: ShellBrief,
    pub action_queue: Vec<ShellAction>,
    pub operator_panel: OperatorPanel,
    pub session_line: String,
    pub case_label_text: String,
    pub role_badge_text: &'static str,
    pub role_badge_class: &'static str,
    pub auth_gate_badge_text: &'static str,
    pub auth_gate_badge_class: &'static str,
    pub operator_avatar: String,
}

#[derive(Clone, PartialEq)]
pub struct WorkspaceFrameBindings {
    pub session_verifying: bool,
    pub api_base: Signal<String>,
    pub token_draft: Signal<String>,
    pub case_id: Signal<String>,
    pub tab: Signal<Tab>,
    pub show_devtools: Signal<bool>,
    pub status: Signal<Option<String>>,
}

#[component]
pub fn WorkspaceFrame(
    view: WorkspaceFrameViewData,
    bindings: WorkspaceFrameBindings,
    on_me: EventHandler<MouseEvent>,
    on_logout: EventHandler<MouseEvent>,
    on_apply_token: EventHandler<MouseEvent>,
    on_clear_connection: EventHandler<MouseEvent>,
) -> Element {
    let WorkspaceFrameViewData {
        shell_state,
        brief,
        action_queue,
        operator_panel,
        session_line,
        case_label_text,
        role_badge_text,
        role_badge_class,
        auth_gate_badge_text,
        auth_gate_badge_class,
        operator_avatar,
    } = view;
    let WorkspaceFrameBindings {
        session_verifying,
        mut api_base,
        mut token_draft,
        mut case_id,
        tab,
        show_devtools,
        status,
    } = bindings;
    let active_tab = shell_state.active_tab;
    let conversation_stage = if active_tab == Tab::Brief {
        crate::shell::ConversationStage::Active
    } else {
        crate::shell::ConversationStage::Empty
    };
    let conversation_vm = conversation_view_model(conversation_stage);
    let left_rail_vm = left_rail_view_model_default(None);
    let history_items = vec![
        left_rail_history_item(
            "Latest workspace run",
            crate::shell::HistoryScope::CurrentCase,
            Some(&case_label_text),
        ),
        left_rail_history_item(
            "Prior conversation",
            crate::shell::HistoryScope::CurrentCase,
            Some(&case_label_text),
        ),
    ];
    let primary_nav_tabs = primary_nav_tabs();
    let utility_entries = utility_entries_view_model();
    let selected_case_id = case_id();
    let workspace_brief = brief.clone();
    let workspace_actions = action_queue.clone();

    rsx! {
        div { class: "shell",
            aside { class: "shell__sidebar",
                div { class: "shell__brand",
                    div { class: "logo", "LM" }
                    div {
                        h1 { "LegalMinds" }
                        p { "AI case command center" }
                    }
                }

                div { class: "shell__sidebar-card",
                    span { class: "eyebrow", "Current Case" }
                    strong { "{case_label_text}" }
                    p { class: "muted", "{tab_description(active_tab)}" }
                }

                nav { class: "navrail",
                    for nav_tab in primary_nav_tabs.iter().copied() {
                        button {
                            class: if active_tab == nav_tab {
                                "navrail__btn navrail__btn--active"
                            } else {
                                "navrail__btn"
                            },
                            onclick: {
                                let mut tab = tab;
                                move |_| tab.set(nav_tab)
                            },
                            span { class: "navrail__label", "{tab_label(nav_tab)}" }
                        }
                    }
                }

                div { class: "shell__sidebar-card",
                    span { class: "eyebrow", "Conversation Scope" }
                    p { class: "muted", "{left_rail_vm.active_scope_label}" }
                    for item in history_items.iter() {
                        div {
                            strong { "{item.title}" }
                            if let Some(case_label) = item.case_label.as_deref() {
                                p { class: "muted", "{case_label}" }
                            }
                        }
                    }
                }

                div { class: "shell__sidebar-card",
                    span { class: "eyebrow", "Utilities" }
                    for item in utility_entries.iter() {
                        button {
                            class: if item.tab == active_tab {
                                "navrail__btn navrail__btn--active"
                            } else {
                                "navrail__btn"
                            },
                            onclick: {
                                let mut tab = tab;
                                let target = item.tab;
                                move |_| tab.set(target)
                            },
                            span { class: "navrail__label", "{item.label}" }
                        }
                    }
                }

                div { class: "shell__sidebar-card shell__sidebar-card--footer",
                    span { class: "eyebrow", "Session" }
                    p { class: "muted", "{session_line}" }
                }
            }

            div { class: "shell__stage",
                header { class: "shell__topbar",
                    div { class: "shell__heading",
                        span { class: "eyebrow", "AI Lead" }
                        h1 { "{tab_label(active_tab)}" }
                        p { "{tab_description(active_tab)}" }
                    }

                    div { class: "shell__topbar-right",
                        section { class: "shell__operator",
                            div { class: "shell__operator-head",
                                div { class: "shell__avatar", "{operator_avatar}" }
                                div { class: "shell__operator-copy",
                                    strong { "{operator_panel.title}" }
                                    p { "{operator_panel.subtitle}" }
                                }
                            }
                            div { class: "shell__operator-meta",
                                div {
                                    span { class: "eyebrow", "Case" }
                                    p { "{operator_panel.case_label}" }
                                }
                                div {
                                    span { class: "eyebrow", "Role" }
                                    p { "{operator_panel.role_label}" }
                                }
                            }
                            p { class: "muted shell__operator-summary", "{operator_panel.summary}" }
                        }
                        div { class: "shell__pillbar",
                            span { class: auth_gate_badge_class, "{auth_gate_badge_text}" }
                            span { class: "badge badge--run", "Single-case scope" }
                            span { class: "badge badge--run", "Low-risk auto" }
                            span { class: "badge badge--warn", "Writes need review" }
                            span { class: role_badge_class, "{role_badge_text}" }
                        }
                        div { class: "actions shell__actions",
                            button {
                                class: "btn btn--accent",
                                onclick: {
                                    let mut tab = tab;
                                    move |_| tab.set(Tab::Brief)
                                },
                                "AI Brief"
                            }
                            button {
                                class: "btn btn--ghost",
                                onclick: {
                                    let mut show_devtools = show_devtools;
                                    move |_| show_devtools.set(!show_devtools())
                                },
                                if show_devtools() { "Hide Connection" } else { "Connection" }
                            }
                            button {
                                class: "btn btn--ghost",
                                onclick: move |evt| on_me.call(evt),
                                "Refresh Me"
                            }
                            button {
                                class: "btn btn--ghost",
                                onclick: move |evt| on_logout.call(evt),
                                "Logout"
                            }
                        }
                    }
                }

                if show_devtools() {
                    section { class: "card dev-panel",
                        div { class: "dev-panel__head",
                            div {
                                h3 { "Developer Connection" }
                                p { class: "muted", "Connection parameters stay available, but no longer occupy the first screen." }
                            }
                            span { class: auth_gate_badge_class, "{auth_gate_badge_text}" }
                        }
                        div { class: "dev-panel__grid",
                            label { "API Base"
                                input {
                                    value: api_base(),
                                    placeholder: "http://127.0.0.1:8001",
                                    oninput: move |e| api_base.set(e.value()),
                                }
                            }
                            label { "Access Token"
                                textarea {
                                    value: token_draft(),
                                    placeholder: "Paste Bearer token here...",
                                    oninput: move |e| token_draft.set(e.value()),
                                    rows: 3,
                                }
                            }
                        }
                        div { class: "actions",
                            button {
                                class: "btn btn--accent",
                                disabled: session_verifying,
                                onclick: move |evt| on_apply_token.call(evt),
                                if session_verifying { "Verifying..." } else { "Verify Session" }
                            }
                            button {
                                class: "btn btn--ghost",
                                onclick: move |evt| on_clear_connection.call(evt),
                                "Clear Token"
                            }
                        }
                    }
                }

                if let Some(msg) = &status() {
                    div { class: "status status--shell", "{msg}" }
                }

                main { class: "shell__content",
                    section { class: "command-deck",
                        article { class: "card command-deck__lead",
                            span { class: "eyebrow", "{brief.eyebrow}" }
                            h2 { "{brief.title}" }
                            p { class: "command-deck__summary", "{brief.summary}" }
                            ul { class: "command-deck__insights",
                                for insight in brief.insights.iter() {
                                    li { "{insight}" }
                                }
                            }
                        }

                        article { class: "card command-deck__actions",
                            div { class: "card__topline",
                                h3 { "Suggested Actions" }
                                p { class: "muted", "AI puts low-risk actions on rails and escalates higher-risk writes." }
                            }
                            div { class: "queue",
                                for action in action_queue.iter() {
                                    article { class: "queue__item",
                                        div { class: "queue__meta",
                                            span { class: risk_class(action.risk), "{risk_label(action.risk)}" }
                                            span { class: "muted", "{action.summary}" }
                                        }
                                        div { class: "queue__body",
                                            strong { "{action.title}" }
                                            button {
                                                class: "btn btn--small",
                                                onclick: {
                                                    let intent = action.intent;
                                                    let tab = tab;
                                                    let show_devtools = show_devtools;
                                                    let status = status;
                                                    move |_| apply_action_intent(intent, tab, show_devtools, status)
                                                },
                                                "{action.cta}"
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        article { class: "card command-deck__context",
                            div { class: "card__topline",
                                h3 { "Current Context" }
                                p { class: "muted", "Single-case mode keeps AI suggestions scoped and auditable." }
                            }
                            div { class: "context-grid",
                                label { "Case ID"
                                    input {
                                        value: selected_case_id.clone(),
                                        placeholder: "Select in Cases or paste UUID",
                                        oninput: move |e| case_id.set(e.value()),
                                    }
                                }
                                div { class: "context-grid__facts",
                                    div {
                                        span { class: "eyebrow", "Session" }
                                        p { "{session_line}" }
                                    }
                                    div {
                                        span { class: "eyebrow", "Policy" }
                                        p { "Auto for low-risk. Confirm for writes. Guard rails for destructive changes." }
                                    }
                                }
                            }
                        }
                    }

                    section { class: "workspace",
                        if active_tab == Tab::Brief {
                            BriefWorkspace {
                                brief: workspace_brief,
                                actions: workspace_actions,
                                case_id: selected_case_id.clone(),
                                role_in_case: shell_state.role_in_case.clone(),
                                conversation_stage_label: conversation_vm.stage_label,
                                quick_entries: conversation_vm.quick_entries,
                                on_action: move |intent| {
                                    apply_action_intent(intent, tab, show_devtools, status);
                                },
                            }
                        } else {
                            div { class: "workspace__surface",
                                match active_tab {
                                    Tab::Brief => rsx! {},
                                    Tab::Cases => rsx! { pages::CasesPage {} },
                                    Tab::Timeline => rsx! { pages::TimelinePage {} },
                                    Tab::Files => rsx! { pages::FilesPage {} },
                                    Tab::Persons => rsx! { pages::PersonsPage {} },
                                    Tab::Search => rsx! { pages::SearchPage {} },
                                    Tab::Exports => rsx! { pages::ExportsPage {} },
                                    Tab::Logs => rsx! { pages::LogsPage {} },
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
fn BriefWorkspace(
    brief: ShellBrief,
    actions: Vec<ShellAction>,
    case_id: String,
    role_in_case: Option<String>,
    conversation_stage_label: &'static str,
    quick_entries: Vec<QuickEntryCardViewModel>,
    on_action: EventHandler<ActionIntent>,
) -> Element {
    let case_summary = if case_id.trim().is_empty() {
        "No case selected".to_string()
    } else {
        case_id.clone()
    };
    let role_summary = role_in_case.unwrap_or_else(|| "No case role".to_string());

    rsx! {
        section { class: "panel brief-page",
            header { class: "panel__head",
                h2 { "AI Case Brief" }
                p { class: "muted", "从这里进入案件节奏：先看简报，再批量处理建议动作，最后进入具体工作视图。" }
            }

            div { class: "brief-grid",
                article { class: "card",
                    span { class: "eyebrow", "{brief.eyebrow}" }
                    h3 { "{brief.title}" }
                    p { class: "muted", "{brief.summary}" }
                    ul { class: "command-deck__insights",
                        for insight in brief.insights.iter() {
                            li { "{insight}" }
                        }
                    }
                }

                article { class: "card",
                    h3 { "Action Queue" }
                    div { class: "queue",
                        for action in actions.iter() {
                            article { class: "queue__item",
                                div { class: "queue__meta",
                                    span { class: risk_class(action.risk), "{risk_label(action.risk)}" }
                                    span { class: "muted", "{action.summary}" }
                                }
                                div { class: "queue__body",
                                    strong { "{action.title}" }
                                    button {
                                        class: "btn btn--small",
                                        onclick: {
                                            let intent = action.intent;
                                            move |_| on_action.call(intent)
                                        },
                                        "{action.cta}"
                                    }
                                }
                            }
                        }
                    }
                }

                article { class: "card",
                    h3 { "Operating Context" }
                    div { class: "stack-list",
                        div {
                            span { class: "eyebrow", "Current case" }
                            p { "{case_summary}" }
                        }
                        div {
                            span { class: "eyebrow", "Execution policy" }
                            p { "低风险自动执行。写动作待确认。破坏性动作始终受限。" }
                        }
                        div {
                            span { class: "eyebrow", "Role in case" }
                            p { "{role_summary}" }
                        }
                    }
                }

                article { class: "card",
                    h3 { "Quick Entry" }
                    p { class: "muted", "{conversation_stage_label}" }
                    div { class: "actions",
                        for entry in quick_entries.iter() {
                            button { class: "btn btn--ghost", "{entry.label}" }
                        }
                    }
                }
            }
        }
    }
}

fn apply_action_intent(
    intent: ActionIntent,
    mut tab: Signal<Tab>,
    mut show_devtools: Signal<bool>,
    mut status: Signal<Option<String>>,
) {
    match intent {
        ActionIntent::OpenAuth => status.set(Some(
            "Use the sign-in panel to start a session.".to_string(),
        )),
        ActionIntent::OpenCases => {
            tab.set(Tab::Cases);
            status.set(Some("Opened cases workspace.".to_string()));
        }
        ActionIntent::OpenBrief => {
            tab.set(Tab::Brief);
            status.set(Some("Returned to AI brief.".to_string()));
        }
        ActionIntent::OpenTimeline => {
            tab.set(Tab::Timeline);
            status.set(Some("Opened timeline workspace.".to_string()));
        }
        ActionIntent::OpenFiles => {
            tab.set(Tab::Files);
            status.set(Some("Opened evidence workspace.".to_string()));
        }
        ActionIntent::OpenPersons => {
            tab.set(Tab::Persons);
            status.set(Some("Opened persons workspace.".to_string()));
        }
        ActionIntent::OpenSearch => {
            tab.set(Tab::Search);
            status.set(Some("Opened search workspace.".to_string()));
        }
        ActionIntent::OpenExports => {
            tab.set(Tab::Exports);
            status.set(Some("Opened exports workspace.".to_string()));
        }
        ActionIntent::OpenLogs => {
            tab.set(Tab::Logs);
            status.set(Some("Opened audit logs.".to_string()));
        }
        ActionIntent::ToggleDevtools => {
            show_devtools.set(!show_devtools());
            status.set(Some("Toggled connection panel.".to_string()));
        }
    }
}
