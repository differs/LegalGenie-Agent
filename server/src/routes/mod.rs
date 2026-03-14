use crate::config::CorsOrigins;
use crate::state::AppState;
use axum::{routing::get, Router};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

mod auth;
mod case_exports;
mod case_files;
mod case_members;
mod case_persons;
mod case_timeline;
mod cases;
mod files;
mod health;
mod logs;
mod persons;
mod search;
mod timeline;

pub fn router(state: AppState) -> Router {
    let api = Router::<AppState>::new()
        .route("/health", get(health::health))
        .nest("/auth", auth::router())
        .nest("/cases", cases::router())
        .nest("/files", files::router())
        .nest("/persons", persons::router())
        .nest("/search", search::router())
        .nest("/timeline", timeline::router())
        .nest("/logs", logs::router());

    Router::new()
        .nest("/api/v1", api)
        .with_state(state.clone())
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer(&state))
}

fn cors_layer(state: &AppState) -> CorsLayer {
    match &state.config.cors_origins {
        CorsOrigins::Any => CorsLayer::permissive(),
        CorsOrigins::AllowList(origins) => {
            let allow_origin = origins
                .iter()
                .filter_map(|o| o.parse().ok())
                .collect::<Vec<_>>();

            CorsLayer::new()
                .allow_origin(allow_origin)
                .allow_methods(Any)
                .allow_headers(Any)
        }
    }
}
