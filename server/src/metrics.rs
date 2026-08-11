//! Observability (P4): Prometheus metrics, health probes.
//!
//! - `GET /metrics`     – Prometheus text exposition (internal endpoint)
//! - `GET /health/live` – process liveness (always 200 when serving)
//! - `GET /health/ready`– dependency probe: DB, jobs table, object store
//!
//! Job metrics are DB-driven (accurate across multiple worker instances);
//! HTTP metrics come from a middleware; worker liveness is an atomic gauge.
use crate::state::AppState;
use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use prometheus::{
    Encoder, Gauge, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, Opts, Registry,
    TextEncoder,
};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub struct Metrics {
    pub registry: Registry,
    pub http_requests: IntCounterVec,
    pub http_duration: HistogramVec,
    pub workers_active: IntGauge,
    pub worker_slots: IntGauge,
    pub jobs_succeeded: Gauge,
    pub jobs_failed: Gauge,
    pub jobs_dead: Gauge,
    pub jobs_pending: Gauge,
    pub jobs_due: Gauge,
}

static ACTIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);

impl Metrics {
    pub fn new() -> Arc<Self> {
        let registry = Registry::new();
        let http_requests = IntCounterVec::new(
            Opts::new("legalgenie_http_requests_total", "HTTP requests processed"),
            &["method", "path", "status"],
        )
        .expect("metric");
        let http_duration = HistogramVec::new(
            HistogramOpts::new(
                "legalgenie_http_request_duration_seconds",
                "HTTP request latency",
            )
            .buckets(vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
            &["method", "path"],
        )
        .expect("metric");
        let workers_active = IntGauge::new(
            "legalgenie_workers_active",
            "Workers currently claiming jobs",
        )
        .expect("metric");
        let worker_slots =
            IntGauge::new("legalgenie_workers_slots", "Configured worker slots").expect("metric");

        registry
            .register(Box::new(http_requests.clone()))
            .expect("register");
        registry
            .register(Box::new(http_duration.clone()))
            .expect("register");
        registry
            .register(Box::new(workers_active.clone()))
            .expect("register");
        registry
            .register(Box::new(worker_slots.clone()))
            .expect("register");

        let mk = |name: &str| {
            let g = Gauge::with_opts(Opts::new(name, name)).expect("metric");
            registry.register(Box::new(g.clone())).expect("register");
            g
        };
        let jobs_succeeded = mk("legalgenie_jobs_succeeded_total");
        let jobs_failed = mk("legalgenie_jobs_failed_total");
        let jobs_dead = mk("legalgenie_jobs_dead_total");
        let jobs_pending = mk("legalgenie_jobs_pending_total");
        let jobs_due = mk("legalgenie_jobs_due_total");

        Arc::new(Metrics {
            registry,
            http_requests,
            http_duration,
            workers_active,
            worker_slots,
            jobs_succeeded,
            jobs_failed,
            jobs_dead,
            jobs_pending,
            jobs_due,
        })
    }

    pub fn worker_started(&self) {
        ACTIVE_WORKERS.fetch_add(1, Ordering::Relaxed);
        self.workers_active
            .set(ACTIVE_WORKERS.load(Ordering::Relaxed) as i64);
    }

    pub fn worker_stopped(&self) {
        ACTIVE_WORKERS.fetch_sub(1, Ordering::Relaxed);
        self.workers_active
            .set(ACTIVE_WORKERS.load(Ordering::Relaxed) as i64);
    }
}

pub fn metrics_router(state: AppState) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health/live", get(live_handler))
        .route("/health/ready", get(ready_handler))
        .with_state(state)
}

pub async fn metrics_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().clone();
    let start = Instant::now();
    let response = next.run(req).await;
    let status = response.status().as_u16();
    let elapsed = start.elapsed().as_secs_f64();

    state
        .metrics
        .http_requests
        .with_label_values(&[method.as_str(), &path, &status.to_string()])
        .inc();
    state
        .metrics
        .http_duration
        .with_label_values(&[method.as_str(), &path])
        .observe(elapsed);
    response
}

async fn metrics_handler(State(state): State<AppState>) -> Response {
    refresh_job_metrics(&state).await;
    let encoder = TextEncoder::new();
    let metric_families = state.metrics.registry.gather();
    let mut buffer = Vec::new();
    match encoder.encode(&metric_families, &mut buffer) {
        Ok(()) => (
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "text/plain; version=0.0.4",
            )],
            String::from_utf8_lossy(&buffer).into_owned(),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "code": 500, "message": format!("encode metrics: {err}") })),
        )
            .into_response(),
    }
}

/// Refreshes DB-driven job gauges on each scrape so multi-instance state is
/// always reflected.
async fn refresh_job_metrics(state: &AppState) {
    let pool = &state.pool;
    let (succeeded, failed, dead, pending, due): (i64, i64, i64, i64, i64) = match sqlx::query_as(
        r#"
        SELECT
            COALESCE(SUM(CASE WHEN status = 'succeeded' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN status = 'dead' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN status = 'pending' AND next_run_at <= utc_text() THEN 1 ELSE 0 END), 0)
        FROM jobs
        "#,
    )
    .fetch_one(pool)
    .await
    {
        Ok(row) => row,
        Err(_) => return,
    };

    state.metrics.jobs_succeeded.set(succeeded as f64);
    state.metrics.jobs_failed.set(failed as f64);
    state.metrics.jobs_dead.set(dead as f64);
    state.metrics.jobs_pending.set(pending as f64);
    state.metrics.jobs_due.set(due as f64);
}

async fn live_handler() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

async fn ready_handler(State(state): State<AppState>) -> Response {
    // DB probe
    if sqlx::query_scalar::<_, i64>("SELECT 1::bigint")
        .fetch_one(&state.pool)
        .await
        .is_err()
    {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "unavailable", "reason": "database unreachable" })),
        )
            .into_response();
    }
    // Jobs table probe (queue ready)
    if sqlx::query_scalar::<_, i64>("SELECT 1::bigint FROM jobs LIMIT 1")
        .fetch_optional(&state.pool)
        .await
        .is_err()
    {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "unavailable", "reason": "jobs table unavailable" })),
        )
            .into_response();
    }
    // Object store probe (writable)
    if state
        .store
        .put_bytes("health/probe.tmp", b"ok")
        .await
        .is_err()
        || state.store.delete("health/probe.tmp").await.is_err()
    {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "unavailable", "reason": "object store unavailable" })),
        )
            .into_response();
    }
    (
        StatusCode::OK,
        Json(json!({ "status": "ok", "dependencies": ["postgres", "object_store", "jobs"] })),
    )
        .into_response()
}
