//! Durable job queue (P1): PostgreSQL-native workers.
//!
//! Jobs are claimed with `SELECT ... FOR UPDATE SKIP LOCKED` inside a
//! transaction, so multiple worker processes/instances can safely compete.
//! Leases expire on crash; `reset_stale_jobs` returns expired jobs to
//! `pending` without burning retry budget. Failures back off exponentially
//! (with jitter) and move to `dead` after `max_attempts`.
use chrono::{Duration, Utc};
use serde::Serialize;
use sqlx::PgPool;
use std::time::Duration as StdDuration;

const LEASE_SECS: i64 = 5 * 60;
pub const HEARTBEAT_INTERVAL: StdDuration = StdDuration::from_secs(30);
const MAX_BACKOFF_SECS: i64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum JobKind {
    Parse,
    Translate,
}

impl JobKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobKind::Parse => "parse",
            JobKind::Translate => "translate",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub kind: JobKind,
    pub case_id: Option<String>,
    pub ref_id: String,
    pub payload: serde_json::Value,
    pub attempts: i64,
    pub max_attempts: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct JobRow {
    id: String,
    kind: String,
    case_id: Option<String>,
    ref_id: String,
    payload: String,
    attempts: i64,
    max_attempts: i64,
}

fn parse_kind(raw: &str) -> anyhow::Result<JobKind> {
    match raw {
        "parse" => Ok(JobKind::Parse),
        "translate" => Ok(JobKind::Translate),
        other => anyhow::bail!("unknown job kind: {other}"),
    }
}

fn now_plus_seconds(secs: i64) -> String {
    (Utc::now() + Duration::seconds(secs))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

fn backoff_seconds(attempt: i64) -> i64 {
    let base = 5_i64.saturating_mul(1_i64 << attempt.min(8));
    let base = base.min(MAX_BACKOFF_SECS);
    // jitter: +0..30%
    let jitter = (base / 3).max(1);
    base + (fast_rand() % jitter)
}

fn fast_rand() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0) as i64;
    (nanos ^ (nanos.rotate_left(17))) & 0x7fff_ffff
}

// ---------- Enqueue ----------

/// Queues a job unless one is already active for the same (kind, ref_id).
/// Returns `true` when a new job was created.
pub async fn enqueue_job(
    pool: &PgPool,
    kind: JobKind,
    case_id: Option<&str>,
    ref_id: &str,
    payload: serde_json::Value,
) -> anyhow::Result<bool> {
    let res = sqlx::query(
        r#"
        INSERT INTO jobs (id, kind, case_id, ref_id, payload, status, next_run_at)
        VALUES ($1, $2, $3, $4, $5, 'pending', utc_text())
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(kind.as_str())
    .bind(case_id)
    .bind(ref_id)
    .bind(payload.to_string())
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

// ---------- Claim ----------

/// Atomically claims the oldest due pending job (SKIP LOCKED).
pub async fn claim_next_job(pool: &PgPool) -> anyhow::Result<Option<Job>> {
    let mut tx = pool.begin().await?;

    let row: Option<JobRow> = sqlx::query_as(
        r#"
        SELECT id, kind, case_id, ref_id, payload, attempts, max_attempts
        FROM jobs
        WHERE status = 'pending' AND next_run_at <= utc_text()
        ORDER BY created_at ASC, id ASC
        LIMIT 1
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };

    let updated = sqlx::query(
        r#"
        UPDATE jobs
        SET status = 'claimed', lease_until = $1, updated_at = utc_text()
        WHERE id = $2 AND status = 'pending'
        "#,
    )
    .bind(now_plus_seconds(LEASE_SECS))
    .bind(&row.id)
    .execute(&mut *tx)
    .await?;

    if updated.rows_affected() == 0 {
        tx.commit().await?;
        return Ok(None);
    }

    tx.commit().await?;

    Ok(Some(Job {
        id: row.id,
        kind: parse_kind(&row.kind)?,
        case_id: row.case_id,
        ref_id: row.ref_id,
        payload: serde_json::from_str(&row.payload).unwrap_or(serde_json::Value::Null),
        attempts: row.attempts,
        max_attempts: row.max_attempts,
    }))
}

// ---------- Lifecycle ----------

pub async fn mark_running(pool: &PgPool, job_id: &str) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE jobs
        SET status = 'processing', lease_until = $1, updated_at = utc_text()
        WHERE id = $2 AND status = 'claimed'
        "#,
    )
    .bind(now_plus_seconds(LEASE_SECS))
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn complete_job(pool: &PgPool, job_id: &str) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE jobs
        SET status = 'succeeded', lease_until = NULL, last_error = NULL, updated_at = utc_text()
        WHERE id = $1 AND status IN ('claimed', 'processing')
        "#,
    )
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Marks the job failed: exponential backoff with jitter, or `dead` once the
/// retry budget is exhausted.
pub async fn fail_job(pool: &PgPool, job_id: &str, error: &str) -> anyhow::Result<()> {
    let row: Option<(i64, i64)> =
        sqlx::query_as("SELECT attempts, max_attempts FROM jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(pool)
            .await?;

    let Some((attempts, max_attempts)) = row else {
        return Ok(());
    };

    let next_attempts = attempts + 1;
    if next_attempts >= max_attempts {
        sqlx::query(
            r#"
            UPDATE jobs
            SET status = 'dead', lease_until = NULL, last_error = $1,
                attempts = $2, updated_at = utc_text()
            WHERE id = $3
            "#,
        )
        .bind(truncate_error(error))
        .bind(next_attempts)
        .bind(job_id)
        .execute(pool)
        .await?;
    } else {
        let delay = backoff_seconds(next_attempts);
        sqlx::query(
            r#"
            UPDATE jobs
            SET status = 'pending', lease_until = NULL, last_error = $1,
                attempts = $2, next_run_at = $3, updated_at = utc_text()
            WHERE id = $4
            "#,
        )
        .bind(truncate_error(error))
        .bind(next_attempts)
        .bind(now_plus_seconds(delay))
        .bind(job_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

fn truncate_error(error: &str) -> String {
    let mut s = error.to_string();
    s.truncate(2000);
    s
}

/// Returns jobs whose lease expired (crashed workers) to `pending` so another
/// worker can retake them. Does not consume retry budget.
pub async fn reset_stale_jobs(pool: &PgPool) -> anyhow::Result<i64> {
    let res = sqlx::query(
        r#"
        UPDATE jobs
        SET status = 'pending', lease_until = NULL, updated_at = utc_text()
        WHERE status IN ('claimed', 'processing')
          AND lease_until IS NOT NULL
          AND lease_until < utc_text()
        "#,
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() as i64)
}

/// Renews the lease of a running job.
pub async fn heartbeat_job(pool: &PgPool, job_id: &str) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE jobs
        SET lease_until = $1, updated_at = utc_text()
        WHERE id = $2 AND status = 'processing'
        "#,
    )
    .bind(now_plus_seconds(LEASE_SECS))
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------- Events ----------

pub async fn record_job_event(
    pool: &PgPool,
    job_id: &str,
    kind: JobKind,
    event: &str,
    detail: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO job_events (job_id, kind, event, detail)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(job_id)
    .bind(kind.as_str())
    .bind(event)
    .bind(detail)
    .execute(pool)
    .await?;
    Ok(())
}
