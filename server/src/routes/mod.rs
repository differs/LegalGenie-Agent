use crate::config::CorsOrigins;
use crate::errors::AppError;
use crate::state::AppState;
use axum::{
    body::Body,
    extract::State,
    http::{header::HeaderName, HeaderValue, Request},
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
    // Allow multipart requests up to MAX_FILE_SIZE (+ small overhead).
    let max_upload_bytes =
        (state.config.max_file_size.min(usize::MAX as u64) as usize).saturating_add(1_048_576);

    let api = Router::<AppState>::new()
        .route("/health", get(health::health))
        .nest("/auth", auth::router())
        .nest("/cases", cases::router(max_upload_bytes))
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
        .layer(from_fn_with_state(state.clone(), security_headers))
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
    if forwarded_proto_https(&req) {
        return next.run(req).await;
    }

    AppError::bad_request("https required").into_response()
}

async fn security_headers(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let is_https = forwarded_proto_https(&req);
    let mut resp = next.run(req).await;

    let headers = resp.headers_mut();
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    );
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
    );
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(
            "default-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; img-src data:; style-src 'unsafe-inline'",
        ),
    );

    if state.config.force_https && is_https {
        headers.insert(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }

    resp
}

fn forwarded_proto_https(req: &Request<Body>) -> bool {
    let xf_proto = req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let xf_proto = xf_proto.split(',').next().unwrap_or("").trim();
    if xf_proto.eq_ignore_ascii_case("https") {
        return true;
    }

    let forwarded = req
        .headers()
        .get("forwarded")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    forwarded.to_ascii_lowercase().contains("proto=https")
}
