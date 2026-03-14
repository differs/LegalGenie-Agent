mod api;
mod models;
mod pages;

use dioxus::prelude::*;

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
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Cases,
    Exports,
    Logs,
}

#[component]
fn App() -> Element {
    let mut api_base = use_signal(|| "http://127.0.0.1:8000".to_string());
    let mut token = use_signal(String::new);
    let case_id = use_signal(String::new);
    let status = use_signal(|| None::<String>);
    let mut tab = use_signal(|| Tab::Exports);

    provide_context(AppCtx {
        api_base,
        token,
        case_id,
        status,
    });

    let mut login_username = use_signal(String::new);
    let mut login_password = use_signal(String::new);

    let on_login = move |_| {
        let base = api_base();
        let username = login_username();
        let password = login_password();
        let mut status = status;
        let mut token = token;
        spawn(async move {
            status.set(Some("Logging in...".to_string()));
            match api::post_login(&base, &username, &password).await {
                Ok(data) => {
                    token.set(data.access_token);
                    status.set(Some("Logged in".to_string()));
                }
                Err(e) => status.set(Some(e)),
            }
        });
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
                }

                div { class: "auth",
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
                    button { class: "btn btn--accent", onclick: on_login, "Login" }
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
