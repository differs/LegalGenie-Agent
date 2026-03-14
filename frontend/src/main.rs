mod api;
mod models;
mod pages;

use dioxus::prelude::*;
use models::UserInfo;

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
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Cases,
    Exports,
    Logs,
}

#[derive(Clone, Copy, PartialEq)]
enum AuthMode {
    Login,
    Register,
}

#[component]
fn App() -> Element {
    let mut api_base = use_signal(|| "http://127.0.0.1:8000".to_string());
    let mut token = use_signal(String::new);
    let mut case_id = use_signal(String::new);
    let mut user = use_signal(|| None::<UserInfo>);
    let mut status = use_signal(|| None::<String>);
    let mut tab = use_signal(|| Tab::Exports);

    provide_context(AppCtx {
        api_base,
        token,
        case_id,
        user,
        status,
    });

    let mut auth_mode = use_signal(|| AuthMode::Login);

    let mut login_username = use_signal(String::new);
    let mut login_password = use_signal(String::new);

    let on_login = move |_| {
        let base = api_base();
        let username = login_username();
        let password = login_password();
        let mut status = status;
        let mut token = token;
        let mut user = user;
        spawn(async move {
            status.set(Some("Logging in...".to_string()));
            match api::post_login(&base, &username, &password).await {
                Ok(data) => {
                    token.set(data.access_token);
                    user.set(Some(data.user));
                    status.set(Some("Logged in".to_string()));
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
        let mut user = user;
        let mut auth_mode = auth_mode;
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
                    token.set(data.access_token);
                    user.set(Some(data.user));
                    auth_mode.set(AuthMode::Login);
                    status.set(Some("Registered and logged in".to_string()));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    let on_me = move |_| {
        let base = api_base();
        let t = token();
        let mut status = status;
        let mut user = user;
        spawn(async move {
            if t.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                return;
            }
            status.set(Some("Loading /me...".to_string()));
            match api::get_me(&base, t.trim()).await {
                Ok(u) => {
                    user.set(Some(u.clone()));
                    status.set(Some(format!("Me: {} ({})", u.username, u.roles.join(","))));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    let on_logout = move |_| {
        token.set(String::new());
        user.set(None);
        case_id.set(String::new());
        status.set(Some("Logged out (local)".to_string()));
    };

    rsx! {
        style { {APP_CSS} }
        div { class: "app",
            header { class: "topbar",
                div { class: "brand",
                    div { class: "logo", "LM" }
                    div {
                        h1 { "LegalMinds" }
                        p { "Exports and audit logs" }
                    }
                }

                nav { class: "tabs",
                    button {
                        class: if tab() == Tab::Cases { "tab tab--active" } else { "tab" },
                        onclick: move |_| tab.set(Tab::Cases),
                        "Cases"
                    }
                    button {
                        class: if tab() == Tab::Exports { "tab tab--active" } else { "tab" },
                        onclick: move |_| tab.set(Tab::Exports),
                        "Exports"
                    }
                    button {
                        class: if tab() == Tab::Logs { "tab tab--active" } else { "tab" },
                        onclick: move |_| tab.set(Tab::Logs),
                        "Logs"
                    }
                }

                div { class: "conn",
                    label { "API Base"
                        input {
                            value: api_base(),
                            placeholder: "http://127.0.0.1:8000",
                            oninput: move |e| api_base.set(e.value()),
                        }
                    }
                    label { "Access Token"
                        textarea {
                            value: token(),
                            placeholder: "Paste Bearer token here...",
                            oninput: move |e| token.set(e.value()),
                            rows: 2,
                        }
                    }
                    if let Some(u) = ctx_user_line(user()) {
                        div { class: "who", "{u}" }
                    }
                }

                div { class: "auth",
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
                                button { class: "btn btn--ghost", onclick: on_me, "Me" }
                                button { class: "btn btn--ghost", onclick: on_logout, "Logout" }
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
            }

            if let Some(msg) = &status() {
                div { class: "status", "{msg}" }
            }

            main { class: "content",
                match tab() {
                    Tab::Cases => rsx! { pages::CasesPage {} },
                    Tab::Exports => rsx! { pages::ExportsPage {} },
                    Tab::Logs => rsx! { pages::LogsPage {} },
                }
            }
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
  --bg:#fbf6ea;
  --paper:rgba(255,255,255,0.86);
  --ink:#0f172a;
  --muted:#475569;
  --line:rgba(15,23,42,0.12);
  --accent:#2f5d8a;
  --accent2:#b45309;
  --shadow:0 18px 40px rgba(15,23,42,0.10);
}

*{box-sizing:border-box;}
body{margin:0;background:radial-gradient(900px 600px at 18% 8%, #fff 0%, var(--bg) 55%, #f6f0df 100%);color:var(--ink);}
body:before{
  content:"";
  position:fixed;inset:0;
  background:
    linear-gradient(transparent 0, transparent 23px, rgba(15,23,42,0.04) 24px),
    linear-gradient(90deg, transparent 0, transparent 23px, rgba(15,23,42,0.03) 24px);
  background-size:24px 24px;
  pointer-events:none;
  opacity:0.35;
}

.app{position:relative;min-height:100vh;}

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

.tabs{display:flex;align-items:center;gap:10px;}
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
.cell{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
.pager{display:flex;align-items:center;justify-content:space-between;margin-top:10px;}

.history{border:1px solid rgba(15,23,42,0.10);border-radius:16px;background:rgba(255,255,255,0.72);padding:12px;margin-top:10px;}
.history__meta{display:flex;gap:10px;align-items:center;margin-bottom:8px;}
.history__changes{max-height:240px;overflow:auto;margin:0;background:rgba(15,23,42,0.92);color:#e2e8f0;padding:10px;border-radius:12px;}

@media (max-width: 1100px){
  .topbar{grid-template-columns: 1fr;position:static;}
  .grid{grid-template-columns:1fr;}
}

@keyframes fadeIn{
  from{opacity:0;transform:translateY(6px);}
  to{opacity:1;transform:translateY(0);}
}
"#;
