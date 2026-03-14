use crate::config::CorsOrigins;
use crate::errors::AppError;
use crate::state::AppState;
use axum::{
    body::Body,
    extract::State,
    http::Request,
    middleware::{from_fn_with_state, Next},
    response::IntoResponse,
    response::Response,
    routing::get,
    Router,
};
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
        .layer(from_fn_with_state(state.clone(), enforce_https))
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

async fn enforce_https(State(state): State<AppState>, req: Request<Body>, next: Next) -> Response {
    if !state.config.force_https {
        return next.run(req).await;
    }

    // Trust `x-forwarded-proto` / `forwarded` headers set by a reverse proxy.
    // If they are missing or not https, reject the request.
    let xf_proto = req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let xf_proto = xf_proto.split(',').next().unwrap_or("").trim();
    if xf_proto.eq_ignore_ascii_case("https") {
        return next.run(req).await;
    }

    let forwarded = req
        .headers()
        .get("forwarded")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if forwarded.to_ascii_lowercase().contains("proto=https") {
        return next.run(req).await;
    }

    AppError::bad_request("https required").into_response()
}
