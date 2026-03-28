mod api;
mod models;
mod pages;
mod shell;
mod workspace;

use dioxus::prelude::*;
use models::UserInfo;
use shell::{
    auth_gate_label, build_action_queue, build_operator_panel, build_shell_brief, derive_auth_gate,
    workspace_tab_for_auth_gate, AuthGate, Tab,
};

#[cfg(target_arch = "wasm32")]
fn main() {
    dioxus_web::launch::launch_cfg(App, dioxus_web::Config::default());
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let cfg = dioxus_desktop::Config::new().with_window(
        dioxus_desktop::WindowBuilder::new()
            .with_title("LegalMinds")
            .with_inner_size(dioxus_desktop::LogicalSize::new(1280.0, 820.0)),
    );
    dioxus_desktop::launch::launch(App, Vec::new(), cfg);
}

#[derive(Clone, Copy)]
pub struct AppCtx {
    pub api_base: Signal<String>,
    pub token: Signal<String>,
    pub case_id: Signal<String>,
    pub case_role: Signal<Option<String>>,
    pub case_role_loading: Signal<bool>,
    pub user: Signal<Option<UserInfo>>,
    pub status: Signal<Option<String>>,
}

impl AppCtx {
    pub fn api_base(&self) -> String {
        (self.api_base)()
    }

    pub fn token(&self) -> String {
        (self.token)()
    }

    pub fn case_id(&self) -> String {
        (self.case_id)()
    }

    pub fn user(&self) -> Option<UserInfo> {
        (self.user)()
    }

    pub fn case_role(&self) -> Option<String> {
        (self.case_role)()
    }

    pub fn case_role_loading(&self) -> bool {
        (self.case_role_loading)()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum AuthMode {
    Login,
    Register,
}

#[component]
fn App() -> Element {
    let mut api_base = use_signal(|| "http://127.0.0.1:8001".to_string());
    let mut token = use_signal(String::new);
    let mut token_draft = use_signal(String::new);
    let mut session_refresh_tick = use_signal(|| 0u64);
    let mut case_id = use_signal(String::new);
    let case_role = use_signal(|| None::<String>);
    let case_role_loading = use_signal(|| false);
    let mut user = use_signal(|| None::<UserInfo>);
    let mut status = use_signal(|| None::<String>);
    let mut tab = use_signal(|| Tab::Brief);
    let mut show_devtools = use_signal(|| false);

    provide_context(AppCtx {
        api_base,
        token,
        case_id,
        case_role,
        case_role_loading,
        user,
        status,
    });

    let session_probe = use_resource(move || {
        let _ = session_refresh_tick();
        let base = api_base();
        let active_token = token();
        async move {
            if active_token.trim().is_empty() {
                return Ok(None);
            }
            api::get_me(&base, active_token.trim()).await.map(Some)
        }
    });

    use_effect(move || {
        let active_token = token();
        match session_probe() {
            None => {
                if active_token.trim().is_empty() {
                    user.set(None);
                }
            }
            Some(Ok(None)) => user.set(None),
            Some(Ok(Some(me))) => {
                user.set(Some(me.clone()));
                status.set(Some(format!("Session verified for {}", me.username)));
            }
            Some(Err(e)) => {
                user.set(None);
                status.set(Some(format!("Session verification failed: {e}")));
            }
        }
    });

    // Load the user's role in the currently selected case (owner/member/viewer).
    // This drives UI-side permission gating; the backend is still the source of truth.
    use_effect(move || {
        let gate = session_probe();
        let base = api_base();
        let t = token();
        let cid = case_id();
        let mut case_role_sig = case_role;
        let mut case_role_loading_sig = case_role_loading;
        let session_ready = matches!(gate, Some(Ok(Some(_))));
        if !session_ready || t.trim().is_empty() || cid.trim().len() < 32 {
            case_role_sig.set(None);
            case_role_loading_sig.set(false);
            return;
        }

        spawn(async move {
            case_role_loading_sig.set(true);
            match api::get_case_member_me(&base, t.trim(), cid.trim()).await {
                Ok(me) => case_role_sig.set(Some(me.role_in_case)),
                Err(_) => case_role_sig.set(None),
            }
            case_role_loading_sig.set(false);
        });
    });

    let mut auth_mode = use_signal(|| AuthMode::Login);

    use_effect(move || {
        if matches!(session_probe(), Some(Ok(Some(_)))) {
            tab.set(Tab::Brief);
            show_devtools.set(false);
            auth_mode.set(AuthMode::Login);
        }
    });

    let mut login_username = use_signal(String::new);
    let mut login_password = use_signal(String::new);

    let on_login = move |_| {
        let base = api_base();
        let username = login_username();
        let password = login_password();
        let mut status = status;
        let mut token = token;
        let mut token_draft = token_draft;
        let mut user = user;
        let mut session_refresh_tick = session_refresh_tick;
        let mut tab = tab;
        spawn(async move {
            status.set(Some("Logging in...".to_string()));
            match api::post_login(&base, &username, &password).await {
                Ok(data) => {
                    token.set(data.access_token.clone());
                    token_draft.set(data.access_token);
                    user.set(None);
                    session_refresh_tick.set(session_refresh_tick() + 1);
                    tab.set(Tab::Brief);
                    status.set(Some("Login succeeded. Verifying session...".to_string()));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    let mut reg_username = use_signal(String::new);
    let mut reg_email = use_signal(String::new);
    let mut reg_password = use_signal(String::new);
    let mut reg_real_name = use_signal(String::new);

    let on_register = move |_| {
        let base = api_base();
        let username = reg_username();
        let email = reg_email();
        let password = reg_password();
        let real_name = reg_real_name();
        let mut status = status;
        let mut token = token;
        let mut token_draft = token_draft;
        let mut user = user;
        let mut session_refresh_tick = session_refresh_tick;
        let mut auth_mode = auth_mode;
        let mut tab = tab;
        spawn(async move {
            if username.trim().is_empty() || email.trim().is_empty() {
                status.set(Some("username/email required".to_string()));
                return;
            }
            status.set(Some("Registering...".to_string()));
            let real_name = if real_name.trim().is_empty() {
                None
            } else {
                Some(real_name.trim())
            };
            match api::post_register(&base, username.trim(), email.trim(), &password, real_name)
                .await
            {
                Ok(data) => {
                    token.set(data.access_token.clone());
                    token_draft.set(data.access_token);
                    user.set(None);
                    session_refresh_tick.set(session_refresh_tick() + 1);
                    auth_mode.set(AuthMode::Login);
                    tab.set(Tab::Brief);
                    status.set(Some("Account created. Verifying session...".to_string()));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    let on_apply_token = move |_| {
        let mut status = status;
        let mut user = user;
        let mut token = token;
        let draft = token_draft();
        let mut session_refresh_tick = session_refresh_tick;
        if draft.trim().is_empty() {
            token.set(String::new());
            user.set(None);
            status.set(Some("Cleared active access token".to_string()));
            return;
        }
        user.set(None);
        token.set(draft.trim().to_string());
        session_refresh_tick.set(session_refresh_tick() + 1);
        status.set(Some("Verifying session via /auth/me...".to_string()));
    };

    let on_me = move |_| {
        if token().trim().is_empty() {
            status.set(Some("Missing active access token".to_string()));
            return;
        }
        session_refresh_tick.set(session_refresh_tick() + 1);
        status.set(Some("Revalidating session via /auth/me...".to_string()));
    };

    let on_clear_connection = move |_| {
        token.set(String::new());
        token_draft.set(String::new());
        user.set(None);
        case_id.set(String::new());
        session_refresh_tick.set(session_refresh_tick() + 1);
        status.set(Some("Cleared connection state".to_string()));
    };

    let on_logout = move |_| {
        token.set(String::new());
        token_draft.set(String::new());
        user.set(None);
        case_id.set(String::new());
        session_refresh_tick.set(session_refresh_tick() + 1);
        tab.set(Tab::Brief);
        show_devtools.set(false);
        auth_mode.set(AuthMode::Login);
        status.set(Some("Logged out (local)".to_string()));
    };

    let active_token = token();
    let session_probe_state = session_probe();
    let session_verifying = !active_token.trim().is_empty() && session_probe_state.is_none();
    let session_verified = matches!(session_probe_state, Some(Ok(Some(_))));
    let auth_gate = derive_auth_gate(&active_token, session_verifying, session_verified);
    let workspace_tab = workspace_tab_for_auth_gate(auth_gate);
    let connected = workspace_tab.is_some();
    let selected_case_id = case_id();
    let current_role = case_role();
    let active_tab = tab();
    let shell_state = shell::ShellState {
        signed_in: connected,
        has_case: !selected_case_id.trim().is_empty(),
        role_in_case: current_role.clone(),
        active_tab,
        username: user().as_ref().map(|u| u.username.clone()),
    };
    let brief = build_shell_brief(&shell_state);
    let action_queue = build_action_queue(&shell_state);
    let session_line = ctx_user_line(user()).unwrap_or_else(|| match auth_gate {
        AuthGate::Ready => "Signed in with verified session".to_string(),
        AuthGate::Verifying => "Waiting for /auth/me verification".to_string(),
        AuthGate::SignedOut => "No active session".to_string(),
    });
    let (role_badge_text, role_badge_class) =
        case_role_badge(current_role.as_deref(), case_role_loading());
    let case_label_text = current_case_label(&selected_case_id);
    let operator_panel = build_operator_panel(&shell_state, auth_gate, &case_label_text);
    let operator_avatar = operator_initials(&operator_panel.title);
    let (auth_gate_badge_text, auth_gate_badge_class) = match auth_gate {
        AuthGate::SignedOut => (auth_gate_label(auth_gate), "badge"),
        AuthGate::Verifying => (auth_gate_label(auth_gate), "badge badge--run"),
        AuthGate::Ready => (auth_gate_label(auth_gate), "badge badge--ok"),
    };

    rsx! {
        style { {APP_CSS} }
        if connected {
            workspace::frame::WorkspaceFrame {
                view: workspace::frame::WorkspaceFrameViewData {
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
                },
                bindings: workspace::frame::WorkspaceFrameBindings {
                    session_verifying,
                    api_base,
                    token_draft,
                    case_id,
                    tab,
                    show_devtools,
                    status,
                },
                on_me,
                on_logout,
                on_apply_token,
                on_clear_connection,
            }
        } else {
            div { class: "auth-shell",
                div { class: "auth-shell__grid",
                    section { class: "card auth-shell__hero",
                        div { class: "shell__brand shell__brand--auth",
                            div { class: "logo", "LM" }
                            div {
                                h1 { "LegalMinds" }
                                p { "AI-led legal command center" }
                            }
                        }
                        span { class: "eyebrow", "Single-case AI cockpit" }
                        h2 { "登录后进入 AI 主驾驶工作台" }
                        p { class: "muted", "首屏会先给出案件简报、建议动作和执行边界，再进入时间轴、证据、人物和导出视图。" }
                        ul { class: "command-deck__insights",
                            li { "AI 主驾驶，但所有高风险写操作仍保留人工确认。" }
                            li { "连接设置被收进开发者面板，不再压住主工作流。" }
                            li { "进入案件后，所有建议都会绑定到单案件上下文。" }
                        }
                    }

                    section { class: "card auth-shell__card",
                        div { class: "card__topline",
                            h3 { if auth_mode() == AuthMode::Login { "Sign In" } else { "Create Account" } }
                            p { class: "muted", "Use an account for the full flow, or paste a token in the connection panel." }
                        }

                        div { class: "auth__mode",
                            button {
                                class: if auth_mode() == AuthMode::Login { "tab tab--active" } else { "tab" },
                                onclick: move |_| auth_mode.set(AuthMode::Login),
                                "Login"
                            }
                            button {
                                class: if auth_mode() == AuthMode::Register { "tab tab--active" } else { "tab" },
                                onclick: move |_| auth_mode.set(AuthMode::Register),
                                "Register"
                            }
                        }

                        match auth_mode() {
                            AuthMode::Login => rsx!{
                                label { "Username"
                                    input {
                                        value: login_username(),
                                        placeholder: "username",
                                        oninput: move |e| login_username.set(e.value()),
                                    }
                                }
                                label { "Password"
                                    input {
                                        r#type: "password",
                                        value: login_password(),
                                        placeholder: "password",
                                        oninput: move |e| login_password.set(e.value()),
                                    }
                                }
                                div { class: "actions",
                                    button { class: "btn btn--accent", onclick: on_login, "Login" }
                                }
                            },
                            AuthMode::Register => rsx!{
                                label { "Username"
                                    input {
                                        value: reg_username(),
                                        placeholder: "username",
                                        oninput: move |e| reg_username.set(e.value()),
                                    }
                                }
                                label { "Email"
                                    input {
                                        value: reg_email(),
                                        placeholder: "email",
                                        oninput: move |e| reg_email.set(e.value()),
                                    }
                                }
                                label { "Password"
                                    input {
                                        r#type: "password",
                                        value: reg_password(),
                                        placeholder: "Password (min 8, upper/lower/digit)",
                                        oninput: move |e| reg_password.set(e.value()),
                                    }
                                }
                                label { "Real name"
                                    input {
                                        value: reg_real_name(),
                                        placeholder: "optional",
                                        oninput: move |e| reg_real_name.set(e.value()),
                                    }
                                }
                                div { class: "actions",
                                    button { class: "btn btn--accent", onclick: on_register, "Register" }
                                    button { class: "btn btn--ghost", onclick: move |_| auth_mode.set(AuthMode::Login), "Back" }
                                }
                            },
                        }
                    }

                    section { class: "card auth-shell__card auth-shell__card--dev",
                        div { class: "card__topline",
                            div {
                                h3 { "Connection" }
                                p { class: "muted", "Paste a token here to jump straight into the command center." }
                            }
                            span { class: auth_gate_badge_class, "{auth_gate_badge_text}" }
                        }
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
                                rows: 4,
                            }
                        }
                        div { class: "actions",
                            button {
                                class: "btn btn--accent",
                                disabled: session_verifying,
                                onclick: on_apply_token,
                                if session_verifying { "Verifying..." } else { "Verify Session" }
                            }
                            button { class: "btn btn--ghost", onclick: on_clear_connection, "Clear" }
                        }
                    }
                }

                if let Some(msg) = &status() {
                    div { class: "status status--auth", "{msg}" }
                }
            }
        }
    }
}

fn current_case_label(case_id: &str) -> String {
    let trimmed = case_id.trim();
    if trimmed.is_empty() {
        "No case selected".to_string()
    } else if trimmed.len() > 16 {
        format!("Case {}", &trimmed[..12])
    } else {
        format!("Case {trimmed}")
    }
}

fn operator_initials(title: &str) -> String {
    let mut chars = title.chars().filter(|c| c.is_alphanumeric());
    let first = chars.next().unwrap_or('L');
    let second = chars.next().unwrap_or(first);
    format!(
        "{}{}",
        first.to_ascii_uppercase(),
        second.to_ascii_uppercase()
    )
}

fn case_role_badge(role: Option<&str>, loading: bool) -> (&'static str, &'static str) {
    if loading {
        ("Loading role", "badge badge--run")
    } else {
        match role {
            Some("owner") => ("Owner", "badge badge--ok"),
            Some("member") => ("Member", "badge badge--run"),
            Some("viewer") => ("Viewer", "badge badge--warn"),
            Some(_) => ("Case role", "badge"),
            None => ("No case role", "badge"),
        }
    }
}

fn ctx_user_line(u: Option<UserInfo>) -> Option<String> {
    let u = u?;
    let roles = if u.roles.is_empty() {
        String::new()
    } else {
        format!(" [{}]", u.roles.join(","))
    };
    Some(format!("Signed in: {}{}", u.username, roles))
}

const APP_CSS: &str = r#"
:root{
  --bg:#f6f3ec;
  --bg-alt:#ebe4d7;
  --paper:rgba(255,255,255,0.92);
  --paper-strong:#ffffff;
  --ink:#14202b;
  --muted:#5b6773;
  --line:rgba(20,32,43,0.12);
  --accent:#24364a;
  --accent-strong:#1b2b3c;
  --accent2:#a06a2c;
  --nav:#1c2a39;
  --shadow:0 24px 48px rgba(20,32,43,0.10);
}

*{box-sizing:border-box;}
html,body{min-height:100%;}
body{
  margin:0;
  font-family:"Noto Sans SC","IBM Plex Sans","PingFang SC","Microsoft YaHei",sans-serif;
  background:
    radial-gradient(1200px 720px at 12% 4%, rgba(255,255,255,0.95) 0%, rgba(255,255,255,0) 58%),
    linear-gradient(180deg, var(--bg) 0%, #f0ebdf 100%);
  color:var(--ink);
}
body:before{
  content:"";
  position:fixed;inset:0;
  background:
    linear-gradient(transparent 0, transparent 23px, rgba(20,32,43,0.025) 24px),
    linear-gradient(90deg, transparent 0, transparent 23px, rgba(20,32,43,0.02) 24px);
  background-size:24px 24px;
  pointer-events:none;
  opacity:0.28;
}

.app{position:relative;min-height:100vh;}
.shell{
  min-height:100vh;
  display:grid;
  grid-template-columns:112px minmax(0,1fr);
}
.shell__sidebar{
  position:sticky;
  top:0;
  height:100vh;
  padding:22px 14px 18px;
  background:linear-gradient(180deg, var(--nav) 0%, #16212d 100%);
  color:#ecf2f8;
  border-right:1px solid rgba(255,255,255,0.08);
  display:flex;
  flex-direction:column;
  gap:18px;
}
.shell__sidebar .muted{color:rgba(236,242,248,0.72);}
.shell__brand{
  display:flex;
  gap:12px;
  align-items:center;
}
.shell__brand h1{
  margin:0;
  font-size:18px;
  line-height:1.08;
}
.shell__brand p{
  margin:2px 0 0 0;
  font-size:12px;
  color:rgba(236,242,248,0.72);
}
.shell__brand--auth h1{color:var(--ink);}
.shell__brand--auth p{color:var(--muted);}
.shell__sidebar-card{
  display:grid;
  gap:8px;
  padding:12px;
  border:1px solid rgba(255,255,255,0.08);
  border-radius:18px;
  background:rgba(255,255,255,0.04);
}
.shell__sidebar-card strong{font-size:13px;line-height:1.35;}
.shell__sidebar-card p{margin:0;}
.shell__sidebar-card--footer{margin-top:auto;}
.navrail{
  display:flex;
  flex-direction:column;
  gap:8px;
}
.navrail__btn{
  appearance:none;
  border:1px solid transparent;
  background:rgba(255,255,255,0.04);
  color:#dce7f3;
  border-radius:16px;
  padding:12px 10px;
  text-align:left;
  cursor:pointer;
  font-weight:800;
  letter-spacing:0.01em;
}
.navrail__btn:hover{
  border-color:rgba(255,255,255,0.10);
  background:rgba(255,255,255,0.08);
}
.navrail__btn--active{
  color:#fff;
  border-color:rgba(160,106,44,0.40);
  background:linear-gradient(135deg, rgba(160,106,44,0.22), rgba(255,255,255,0.08));
  box-shadow:inset 0 1px 0 rgba(255,255,255,0.06);
}
.navrail__label{
  font-size:12px;
  line-height:1.18;
}
.shell__stage{
  min-width:0;
  display:flex;
  flex-direction:column;
  gap:16px;
  padding:22px 24px 28px;
}
.shell__topbar{
  display:flex;
  align-items:flex-start;
  justify-content:space-between;
  gap:18px;
}
.shell__heading h1{
  margin:6px 0 8px 0;
  font-size:32px;
  line-height:1.02;
  font-weight:600;
}
.shell__heading p{
  margin:0;
  max-width:720px;
  color:var(--muted);
  font-size:14px;
  line-height:1.55;
}
.shell__topbar-right{
  display:grid;
  gap:10px;
  justify-items:end;
}
.shell__operator{
  width:min(360px, 100%);
  display:grid;
  gap:12px;
  padding:14px 16px;
  border-radius:22px;
  border:1px solid rgba(20,32,43,0.10);
  background:linear-gradient(180deg, rgba(255,255,255,0.94), rgba(246,248,250,0.90));
  box-shadow:var(--shadow);
}
.shell__operator-head{
  display:flex;
  align-items:center;
  gap:12px;
}
.shell__avatar{
  width:42px;
  height:42px;
  border-radius:14px;
  display:flex;
  align-items:center;
  justify-content:center;
  font-size:14px;
  font-weight:900;
  letter-spacing:0.08em;
  color:#fff;
  background:linear-gradient(135deg, var(--accent), var(--accent2));
  box-shadow:0 12px 24px rgba(36,54,74,0.20);
}
.shell__operator-copy strong{
  display:block;
  font-size:15px;
  line-height:1.15;
}
.shell__operator-copy p{
  margin:4px 0 0 0;
  color:var(--muted);
  font-size:12px;
}
.shell__operator-meta{
  display:grid;
  grid-template-columns:1fr 1fr;
  gap:12px;
}
.shell__operator-meta p{
  margin:4px 0 0 0;
  font-size:13px;
  line-height:1.4;
}
.shell__operator-summary{
  margin:0;
  font-size:12px;
  line-height:1.55;
}
.shell__pillbar{
  display:flex;
  flex-wrap:wrap;
  gap:8px;
  justify-content:flex-end;
}
.shell__actions{
  margin-top:0;
  justify-content:flex-end;
}
.shell__content{
  display:flex;
  flex-direction:column;
  gap:18px;
}
.command-deck{
  display:grid;
  grid-template-columns:minmax(0,1.45fr) minmax(320px,1fr) 320px;
  gap:16px;
  align-items:start;
}
.command-deck__lead h2{
  margin:6px 0 10px 0;
  font-size:26px;
  line-height:1.12;
}
.command-deck__summary{
  margin:0 0 14px 0;
  font-size:14px;
  line-height:1.6;
}
.command-deck__insights{
  margin:0;
  padding-left:18px;
  display:grid;
  gap:8px;
  color:var(--muted);
  font-size:13px;
  line-height:1.55;
}
.card__topline{
  display:flex;
  align-items:flex-start;
  justify-content:space-between;
  gap:12px;
  margin-bottom:10px;
}
.card__topline h3{
  margin:0;
  font-size:16px;
}
.card__topline p{
  margin:0;
  max-width:340px;
}
.queue{
  display:flex;
  flex-direction:column;
  gap:12px;
}
.queue__item{
  display:grid;
  gap:10px;
  padding:14px;
  border-radius:16px;
  border:1px solid rgba(20,32,43,0.10);
  background:rgba(246,248,250,0.82);
}
.queue__meta{
  display:flex;
  align-items:center;
  justify-content:space-between;
  gap:10px;
}
.queue__meta .muted{
  font-size:12px;
  text-align:right;
}
.queue__body{
  display:flex;
  align-items:center;
  justify-content:space-between;
  gap:14px;
}
.queue__body strong{
  font-size:14px;
  line-height:1.4;
}
.command-deck__context p{margin:4px 0 0 0;line-height:1.55;}
.context-grid{
  display:grid;
  gap:12px;
}
.context-grid__facts{
  display:grid;
  gap:12px;
}
.workspace{
  display:flex;
  flex-direction:column;
  gap:18px;
}
.workspace__surface{
  display:flex;
  flex-direction:column;
  gap:18px;
}
.brief-grid{
  display:grid;
  grid-template-columns:repeat(3, minmax(0, 1fr));
  gap:16px;
}
.stack-list{
  display:grid;
  gap:14px;
}
.eyebrow{
  display:inline-flex;
  align-items:center;
  gap:6px;
  font-size:11px;
  font-weight:800;
  letter-spacing:0.08em;
  text-transform:uppercase;
  color:var(--accent2);
}
.dev-panel{
  background:rgba(255,255,255,0.95);
}
.dev-panel__head h3{margin:0;font-size:16px;}
.dev-panel__head p{margin:4px 0 0 0;}
.dev-panel__grid{
  display:grid;
  grid-template-columns:1fr 1.35fr;
  gap:14px;
}
.auth-shell{
  min-height:100vh;
  display:flex;
  flex-direction:column;
  justify-content:center;
  padding:32px;
}
.auth-shell__grid{
  width:100%;
  max-width:1480px;
  margin:0 auto;
  display:grid;
  grid-template-columns:1.15fr 0.95fr 0.9fr;
  gap:18px;
  align-items:start;
}
.auth-shell__hero{
  min-height:420px;
  display:flex;
  flex-direction:column;
  justify-content:space-between;
  gap:16px;
}
.auth-shell__hero h2{
  margin:0;
  font-size:32px;
  line-height:1.08;
}
.auth-shell__card{
  min-height:420px;
  display:grid;
  gap:12px;
  align-content:start;
}
.auth-shell__card--dev{
  background:linear-gradient(180deg, rgba(238,242,246,0.96), rgba(255,255,255,0.92));
}
.status--shell{margin:0;}
.status--auth{
  width:100%;
  max-width:1480px;
  margin:18px auto 0;
}

.topbar{
  position:sticky;top:0;z-index:10;
  display:grid;
  grid-template-columns: 1.2fr 0.7fr 2fr 1.2fr;
  gap:14px;
  padding:16px 18px;
  border-bottom:1px solid var(--line);
  background:linear-gradient(180deg, rgba(255,255,255,0.82), rgba(255,255,255,0.66));
  backdrop-filter: blur(10px);
}

.brand{display:flex;gap:12px;align-items:center;}
.logo{
  width:44px;height:44px;border-radius:12px;
  background:linear-gradient(135deg, var(--accent), #1e293b);
  color:white;font-weight:800;display:flex;align-items:center;justify-content:center;
  box-shadow:0 10px 22px rgba(47,93,138,0.25);
  letter-spacing:0.5px;
}
.brand h1{font-size:18px;margin:0;line-height:1.1;}
.brand p{margin:2px 0 0 0;font-size:12px;color:var(--muted);}

.tabs{display:flex;align-items:center;gap:10px;flex-wrap:wrap;}
.tab{
  appearance:none;border:1px solid var(--line);
  background:rgba(255,255,255,0.6);
  padding:10px 12px;border-radius:999px;
  font-weight:700;color:#1f2937;
  cursor:pointer;
}
.tab--active{
  background:linear-gradient(180deg, rgba(47,93,138,0.20), rgba(47,93,138,0.08));
  border-color:rgba(47,93,138,0.45);
  color:#0f172a;
}

.conn,.auth{display:grid;gap:10px;align-content:start;}
.who{font-size:12px;color:var(--muted);font-weight:800;padding-left:2px;}
.auth__mode{display:flex;gap:10px;align-items:center;flex-wrap:wrap;}
label{display:grid;gap:6px;font-size:11px;color:var(--muted);font-weight:700;}
input,textarea{
  width:100%;
  border:1px solid var(--line);
  border-radius:10px;
  padding:10px 12px;
  background:rgba(255,255,255,0.75);
  color:var(--ink);
  font-size:13px;
  outline:none;
}
textarea{resize:vertical;}
input:focus,textarea:focus{border-color:rgba(47,93,138,0.55);box-shadow:0 0 0 4px rgba(47,93,138,0.12);}

.btn{
  appearance:none;
  border:1px solid var(--line);
  background:rgba(255,255,255,0.72);
  padding:10px 12px;
  border-radius:12px;
  cursor:pointer;
  font-weight:800;
  color:var(--ink);
}
.btn:hover{transform:translateY(-1px);box-shadow:0 10px 22px rgba(15,23,42,0.10);}
.btn:disabled{opacity:0.5;cursor:not-allowed;transform:none;box-shadow:none;}
.btn--accent{
  background:linear-gradient(135deg, rgba(47,93,138,0.92), rgba(30,41,59,0.92));
  color:white;
  border-color:rgba(47,93,138,0.65);
}
.btn--ghost{background:transparent;}
.btn--small{padding:8px 10px;border-radius:10px;font-weight:800;}

.status{
  margin:12px 18px 0 18px;
  padding:10px 12px;
  border:1px dashed rgba(180,83,9,0.35);
  border-radius:14px;
  background:rgba(255,255,255,0.70);
  color:#7c2d12;
}

.content{padding:18px;display:flex;flex-direction:column;gap:16px;}

.panel{display:flex;flex-direction:column;gap:14px;animation:fadeIn 220ms ease-out;}
.panel__head h2{margin:0;font-size:16px;}
.panel__head p{margin:4px 0 0 0;color:var(--muted);font-size:12px;}

.grid{display:grid;grid-template-columns: 1fr 1fr;gap:16px;}
.card{
  background:var(--paper);
  border:1px solid var(--line);
  border-radius:18px;
  box-shadow:var(--shadow);
  padding:14px;
}
.card--full{margin-top:14px;}
.card h3{margin:0 0 10px 0;font-size:13px;color:#111827;}
.row{display:grid;grid-template-columns: 1fr 1fr;gap:10px;}
.actions{display:flex;flex-wrap:wrap;gap:10px;margin-top:10px;}
.muted{color:var(--muted);}
.error{color:#b91c1c;font-weight:800;}
.badge{
  display:inline-flex;align-items:center;justify-content:center;
  border:1px solid rgba(47,93,138,0.35);
  background:rgba(47,93,138,0.10);
  color:#0f172a;
  padding:3px 8px;border-radius:999px;
  font-weight:800;font-size:11px;
}
.badge--ok{border-color:rgba(56,161,105,0.35);background:rgba(56,161,105,0.12);}
.badge--warn{border-color:rgba(180,83,9,0.35);background:rgba(180,83,9,0.12);}
.badge--bad{border-color:rgba(185,28,28,0.35);background:rgba(185,28,28,0.12);}
.badge--run{border-color:rgba(49,130,206,0.35);background:rgba(49,130,206,0.12);}

.table{display:grid;gap:8px;}
.table__head,.table__row{
  display:grid;
  grid-template-columns: 120px 1.4fr 170px 110px 120px;
  gap:10px;
  align-items:center;
  padding:10px 10px;
  border:1px solid rgba(15,23,42,0.10);
  border-radius:14px;
  background:rgba(255,255,255,0.70);
}
.table__head{font-weight:900;color:#111827;background:rgba(255,255,255,0.88);}
.table--logs .table__head,.table--logs .table__row{
  grid-template-columns: 180px 140px 110px 110px 1.6fr 120px;
}
.table--files .table__head,.table--files .table__row{
  grid-template-columns: 1.8fr 220px 110px 120px 170px 260px;
}
.table--timeline .table__head,.table--timeline .table__row{
  grid-template-columns: 120px 1.6fr 110px 1fr 220px;
}
.timeline-canvas{
  overflow:hidden;
}
.timeline-canvas__toolbar{
  display:flex;
  flex-wrap:wrap;
  justify-content:space-between;
  gap:12px;
  align-items:flex-start;
  margin-bottom:12px;
}
.timeline-canvas__toolbar h3{
  margin:0;
}
.timeline-canvas__meta{
  margin:0 0 10px 0;
}
.timeline-canvas__frame{
  overflow:auto;
  border:1px solid rgba(15,23,42,0.10);
  border-radius:20px;
  background:
    radial-gradient(circle at top left, rgba(255,255,255,0.92), rgba(247,242,230,0.96)),
    linear-gradient(180deg, rgba(47,93,138,0.04), rgba(180,83,9,0.04));
  box-shadow: inset 0 1px 0 rgba(255,255,255,0.8);
}
.timeline-canvas__svg{
  display:block;
  min-width:1200px;
}
.timeline-canvas__bg{
  fill:rgba(0,0,0,0);
}
.timeline-canvas__axis{
  stroke:rgba(15,23,42,0.38);
  stroke-width:2;
}
.timeline-canvas__tick-line{
  stroke:rgba(15,23,42,0.16);
  stroke-width:1.5;
}
.timeline-canvas__tick{
  fill:#475569;
  font-size:12px;
  font-weight:700;
}
.timeline-canvas__stem{
  stroke:rgba(47,93,138,0.24);
  stroke-width:2;
}
.timeline-canvas__stem--selected{
  stroke:rgba(180,83,9,0.75);
}
.timeline-canvas__card{
  fill:rgba(255,255,255,0.92);
  stroke:rgba(47,93,138,0.20);
  stroke-width:1.5;
  filter:drop-shadow(0 12px 16px rgba(15,23,42,0.10));
}
.timeline-canvas__card--selected{
  fill:rgba(255,247,237,0.98);
  stroke:rgba(180,83,9,0.70);
  stroke-width:2.4;
}
.timeline-canvas__card--dragging{
  fill:rgba(238,246,255,0.98);
  stroke:rgba(47,93,138,0.75);
  stroke-width:2.4;
}
.timeline-canvas__card--flash{
  animation: canvasFlash 1.25s ease;
}
.timeline-canvas__title{
  fill:#0f172a;
  font-size:15px;
  font-weight:800;
}
.timeline-canvas__desc{
  fill:#475569;
  font-size:12px;
}
.timeline-canvas__meta-text{
  fill:#64748b;
  font-size:11px;
  font-weight:700;
}
.timeline-canvas__hint-line{
  stroke:rgba(180,83,9,0.45);
  stroke-width:2;
  stroke_dasharray:8 6;
}
.timeline-canvas__hint{
  fill:#9a3412;
  font-size:12px;
  font-weight:800;
}
.timeline-canvas__hotspot{
  fill:rgba(47,93,138,0.14);
  stroke:rgba(47,93,138,0.42);
  stroke-width:1.6;
  cursor:pointer;
}
.timeline-canvas__hotspot--dense{
  fill:rgba(180,83,9,0.14);
  stroke:rgba(180,83,9,0.55);
}
.timeline-canvas__hotspot-text{
  fill:#0f172a;
  font-size:11px;
  font-weight:900;
  pointer-events:none;
}

/* Case-local person relationship graph (SVG) */
.person-graph__frame{
  overflow:auto;
  border:1px solid rgba(15,23,42,0.10);
  border-radius:18px;
  background:rgba(255,255,255,0.72);
}
.person-graph__svg{display:block;min-width:620px;}
.person-graph__col-label{
  fill:#334155;
  font-size:12px;
  font-weight:900;
}
.person-graph__edge{
  stroke:rgba(47,93,138,0.35);
  stroke-width:2.0;
}
.person-graph__edge--active{
  stroke:rgba(180,83,9,0.80);
  stroke-width:3.0;
}
.person-graph__arrow{fill:rgba(47,93,138,0.60);}
.person-graph__node{
  fill:rgba(255,255,255,0.94);
  stroke:rgba(47,93,138,0.20);
  stroke-width:1.6;
  filter:drop-shadow(0 10px 14px rgba(15,23,42,0.10));
  cursor:pointer;
}
.person-graph__node--selected{
  fill:rgba(255,247,237,0.98);
  stroke:rgba(180,83,9,0.70);
  stroke-width:2.4;
}
.person-graph__node-title{
  fill:#0f172a;
  font-size:13px;
  font-weight:900;
  pointer-events:none;
}
.person-graph__node-meta{
  fill:#64748b;
  font-size:11px;
  font-weight:700;
  pointer-events:none;
}

/* Office layout: timeline + chat + context bar */
.office-context{padding:12px 14px;}
.office-context__row{
  display:grid;
  grid-template-columns: 1.6fr 1fr 1.4fr;
  gap:14px;
  align-items:center;
}
.office-context__left{display:flex;gap:12px;align-items:flex-end;flex-wrap:wrap;}
.office-context__left label{min-width:220px;}
.office-context__range{display:flex;gap:10px;align-items:flex-end;flex-wrap:wrap;}
.office-context__range label{min-width:140px;}
.office-context__mid{display:flex;gap:10px;align-items:center;justify-content:center;flex-wrap:wrap;}
.office-context__item{display:flex;gap:8px;align-items:center;flex-wrap:wrap;}
.office-context__focus{display:flex;gap:8px;align-items:baseline;flex-wrap:wrap;}
.office-context__right{display:flex;gap:12px;align-items:center;justify-content:flex-end;flex-wrap:wrap;}
.office-context__modes{display:flex;gap:8px;align-items:center;flex-wrap:wrap;}

.office-body{
  display:flex;
  gap:14px;
  align-items:stretch;
}
.office-left{flex:1 1 auto;min-width:520px;display:flex;flex-direction:column;gap:16px;}
.office-splitter{
  flex:0 0 12px;
  border-radius:999px;
  cursor:col-resize;
  position:relative;
  background:rgba(15,23,42,0.04);
  border:1px solid rgba(15,23,42,0.08);
}
.office-splitter:before{
  content:"";
  position:absolute;
  top:16px;
  bottom:16px;
  left:50%;
  width:3px;
  transform:translateX(-50%);
  border-radius:999px;
  background:rgba(15,23,42,0.18);
}
.office-splitter:hover{background:rgba(47,93,138,0.06);border-color:rgba(47,93,138,0.18);}
.office-splitter:hover:before{background:rgba(47,93,138,0.55);}
.office-splitter--dragging{background:rgba(180,83,9,0.06);border-color:rgba(180,83,9,0.22);}
.office-splitter--dragging:before{background:rgba(180,83,9,0.75);}
.office--resizing{user-select:none;}
.office-chat{
  flex:0 0 auto;
  padding:0;
  overflow:hidden;
  display:flex;
  flex-direction:column;
  min-height:620px;
}
.office-legacy{margin-top:10px;}
.office-legacy summary{
  cursor:pointer;
  padding:10px 12px;
  border-radius:14px;
  background:rgba(255,255,255,0.70);
  border:1px solid rgba(15,23,42,0.10);
  font-weight:900;
  color:#111827;
}
.office-legacy summary:hover{border-color:rgba(47,93,138,0.40);}

.chat__head{
  padding:14px;
  border-bottom:1px solid var(--line);
  background:linear-gradient(180deg, rgba(255,255,255,0.88), rgba(255,255,255,0.70));
  display:flex;
  align-items:flex-start;
  justify-content:space-between;
  gap:12px;
}
.chat__title h3{margin:0;font-size:14px;}
.chat__title p{margin:4px 0 0 0;font-size:12px;color:var(--muted);}
.chat__tools{display:flex;gap:8px;align-items:center;flex-wrap:wrap;}
.chat__scroll{flex:1;overflow:auto;padding:14px;display:flex;flex-direction:column;gap:12px;}
.chat__log{display:flex;flex-direction:column;gap:10px;}
.chat__msg{
  max-width:92%;
  border:1px solid rgba(15,23,42,0.12);
  border-radius:16px;
  padding:10px 12px;
  background:rgba(255,255,255,0.78);
}
.chat__msg p{margin:0;font-size:13px;line-height:1.45;}
.chat__msg--user{
  align-self:flex-end;
  background:linear-gradient(180deg, rgba(47,93,138,0.18), rgba(255,255,255,0.80));
  border-color:rgba(47,93,138,0.30);
}
.chat__msg--assistant{align-self:flex-start;}
.chat__msg--system{
  align-self:center;
  max-width:100%;
  background:rgba(15,23,42,0.06);
  border-style:dashed;
  color:var(--muted);
}
.chat__preview{
  border:1px solid rgba(180,83,9,0.30);
  background:rgba(255,247,237,0.72);
  border-radius:18px;
  padding:12px;
}
.chat__undo{
  border:1px solid rgba(47,93,138,0.26);
  background:rgba(238,246,255,0.72);
  border-radius:18px;
  padding:12px;
}
.chat__undo p{margin:0 0 8px 0;font-size:13px;}
.chat__preview h4{margin:0 0 8px 0;font-size:13px;}
.chat__preview p{margin:6px 0;font-size:13px;}
.chat__card{
  border:1px solid rgba(15,23,42,0.10);
  background:rgba(255,255,255,0.70);
  border-radius:18px;
  padding:12px;
}
.chat__card h4{margin:0 0 10px 0;font-size:13px;color:#111827;}
.chat__composer{
  border-top:1px solid var(--line);
  padding:12px 14px;
  background:linear-gradient(180deg, rgba(255,255,255,0.74), rgba(255,255,255,0.64));
  display:flex;
  gap:10px;
  align-items:flex-end;
}
.chat__composer textarea{
  min-height:56px;
  resize:none;
}
.chat__composer button{white-space:nowrap;}

.table--persons .table__head,.table--persons .table__row{
  grid-template-columns: 1.5fr 1fr 1fr 1fr 120px;
}
.table--search .table__head,.table--search .table__row{
  grid-template-columns: 120px 1.6fr 1fr 170px 110px;
}
.table--search-history .table__head,.table--search-history .table__row{
  grid-template-columns: 1.2fr 1fr 1.2fr 110px 170px;
}
.cell{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
.pager{display:flex;align-items:center;justify-content:space-between;margin-top:10px;}

.history{border:1px solid rgba(15,23,42,0.10);border-radius:16px;background:rgba(255,255,255,0.72);padding:12px;margin-top:10px;}
.history__meta{display:flex;gap:10px;align-items:center;margin-bottom:8px;}
.history__changes{max-height:240px;overflow:auto;margin:0;background:rgba(15,23,42,0.92);color:#e2e8f0;padding:10px;border-radius:12px;}

.workspace-shell{
  min-height:100vh;
  display:grid;
  grid-template-columns:300px minmax(0, 1fr) 320px;
  gap:14px;
  padding:14px;
}
.left-rail{
  border:1px solid var(--line);
  background:var(--paper);
  border-radius:20px;
  padding:14px;
  display:grid;
  gap:12px;
  align-content:start;
}
.left-rail__brand{display:flex;gap:10px;align-items:center;}
.left-rail__card{
  border:1px solid var(--line);
  border-radius:14px;
  padding:10px;
  background:rgba(255,255,255,0.7);
}
.left-rail__scope{display:flex;justify-content:space-between;gap:10px;align-items:center;}
.left-rail__scope-actions{display:flex;gap:8px;flex-wrap:wrap;margin:8px 0;}
.left-rail__history{display:grid;gap:8px;}
.left-rail__history-item{
  border:1px solid var(--line);
  border-radius:12px;
  padding:8px;
}
.conversation-pane{
  display:grid;
  gap:12px;
  align-content:start;
}
.conversation-pane__header{display:flex;justify-content:space-between;gap:10px;align-items:flex-start;}
.quick-entry-grid{
  display:grid;
  grid-template-columns:repeat(4, minmax(0, 1fr));
  gap:10px;
}
.quick-entry-card{
  border:1px solid var(--line);
  border-radius:12px;
  padding:12px;
  background:rgba(255,255,255,0.78);
  text-align:center;
}
.context-block{
  border:1px solid var(--line);
  border-radius:12px;
  padding:10px;
  background:rgba(255,255,255,0.68);
}
.context-block__meta{display:flex;justify-content:space-between;gap:10px;align-items:center;margin-bottom:6px;}
.workspace-placeholder{
  min-height:180px;
  display:grid;
  align-content:start;
}

@media (max-width: 1360px){
  .workspace-shell{grid-template-columns:280px minmax(0, 1fr);}
  .workspace-placeholder{grid-column:1 / -1;}
  .command-deck{grid-template-columns:1fr 1fr;}
  .command-deck__context{grid-column:1 / -1;}
  .brief-grid{grid-template-columns:1fr;}
  .auth-shell__grid{grid-template-columns:1fr 1fr;}
  .auth-shell__hero{grid-column:1 / -1;min-height:auto;}
}

@media (max-width: 1100px){
  .workspace-shell{grid-template-columns:1fr;}
  .quick-entry-grid{grid-template-columns:repeat(2, minmax(0, 1fr));}
  .shell{grid-template-columns:1fr;}
  .shell__sidebar{
    position:static;
    height:auto;
  }
  .navrail{
    display:grid;
    grid-template-columns:repeat(4, minmax(0, 1fr));
  }
  .shell__stage{padding:18px;}
  .shell__topbar{flex-direction:column;}
  .shell__topbar-right{justify-items:start;}
  .shell__operator{width:100%;}
  .shell__pillbar{justify-content:flex-start;}
  .command-deck{grid-template-columns:1fr;}
  .brief-grid{grid-template-columns:1fr;}
  .dev-panel__grid{grid-template-columns:1fr;}
  .auth-shell{padding:18px;}
  .auth-shell__grid{grid-template-columns:1fr;}
  .auth-shell__card,.auth-shell__hero{min-height:auto;}
  .topbar{grid-template-columns: 1fr;position:static;}
  .grid{grid-template-columns:1fr;}
  .office-body{flex-direction:column;}
  .office-left{min-width:0;}
  .office-splitter{display:none;}
  .office-context__row{grid-template-columns:1fr;}
  .office-context__mid{justify-content:flex-start;}
  .office-context__right{justify-content:flex-start;}
  .office-chat{min-height:520px;width:auto !important;}
}

@keyframes fadeIn{
  from{opacity:0;transform:translateY(6px);}
  to{opacity:1;transform:translateY(0);}
}

@keyframes canvasFlash{
  0%{filter:drop-shadow(0 18px 20px rgba(180,83,9,0.22));}
  45%{filter:drop-shadow(0 22px 24px rgba(180,83,9,0.28));}
  100%{filter:drop-shadow(0 12px 16px rgba(15,23,42,0.10));}
}
"#;
