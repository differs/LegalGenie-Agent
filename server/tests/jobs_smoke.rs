// P1b smoke tests: durable job queue semantics on PostgreSQL.
mod test_support;

use legalminds_server::jobs::{
    claim_next_job, complete_job, enqueue_job, fail_job, mark_running, reset_stale_jobs, JobKind,
};
use serde_json::json;
use std::time::Duration;

async fn new_pool() -> sqlx::PgPool {
    let (pool, _tmp) = test_support::test_pool_and_tmp().await;
    pool
}

#[tokio::test]
async fn enqueue_then_claim_then_complete() {
    let pool = new_pool().await;

    assert!(enqueue_job(
        &pool,
        JobKind::Parse,
        None,
        "file-1",
        json!({"force": true})
    )
    .await
    .unwrap());
    // Duplicate enqueue for the same active (kind, ref_id) is a no-op.
    assert!(
        !enqueue_job(&pool, JobKind::Parse, None, "file-1", json!({}))
            .await
            .unwrap()
    );

    let job = claim_next_job(&pool).await.unwrap().expect("job claimed");
    assert_eq!(job.kind, JobKind::Parse);
    assert_eq!(job.ref_id, "file-1");

    mark_running(&pool, &job.id).await.unwrap();
    complete_job(&pool, &job.id).await.unwrap();

    assert!(
        claim_next_job(&pool).await.unwrap().is_none(),
        "no jobs left"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_claims_are_atomic() {
    let pool = new_pool().await;
    for i in 0..5 {
        enqueue_job(&pool, JobKind::Parse, None, &format!("f-{i}"), json!({}))
            .await
            .unwrap();
    }

    let mut handles = Vec::new();
    for _ in 0..8 {
        let pool = pool.clone();
        handles.push(tokio::spawn(
            async move { claim_next_job(&pool).await.unwrap() },
        ));
    }

    let mut claimed = 0;
    for h in handles {
        if h.await.unwrap().is_some() {
            claimed += 1;
        }
    }
    assert_eq!(
        claimed, 5,
        "each job claimed exactly once across 8 concurrent claimers"
    );
}

#[tokio::test]
async fn expired_lease_is_recovered_without_burning_attempts() {
    let pool = new_pool().await;
    enqueue_job(&pool, JobKind::Translate, None, "f-stale", json!({}))
        .await
        .unwrap();

    let job = claim_next_job(&pool).await.unwrap().expect("claimed");
    mark_running(&pool, &job.id).await.unwrap();

    // Simulate a crash: lease expires.
    sqlx::query(
        "UPDATE jobs SET lease_until = to_char(now() AT TIME ZONE 'UTC' - interval '1 minute', 'YYYY-MM-DD HH24:MI:SS') WHERE id = $1",
    )
    .bind(&job.id)
    .execute(&pool)
    .await
    .unwrap();

    let recovered = reset_stale_jobs(&pool).await.unwrap();
    assert_eq!(recovered, 1);

    // Attempts must NOT be consumed by crash recovery.
    let attempts: i64 = sqlx::query_scalar("SELECT attempts FROM jobs WHERE id = $1")
        .bind(&job.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempts, 0);

    // The job is claimable again.
    let again = claim_next_job(&pool).await.unwrap().expect("re-claimable");
    assert_eq!(again.id, job.id);
}

#[tokio::test]
async fn failure_backs_off_then_dead_letters() {
    let pool = new_pool().await;
    enqueue_job(&pool, JobKind::Parse, None, "f-fail", json!({}))
        .await
        .unwrap();

    // max_attempts=2 → two failures, then dead.
    sqlx::query("UPDATE jobs SET max_attempts = 2 WHERE kind = 'parse' AND ref_id = 'f-fail'")
        .execute(&pool)
        .await
        .unwrap();

    // Fail #1
    let job = claim_next_job(&pool).await.unwrap().expect("claimed 1");
    fail_job(&pool, &job.id, "boom one").await.unwrap();

    let (status1, attempts1, next_run_at1): (String, i64, Option<String>) =
        sqlx::query_as("SELECT status, attempts, next_run_at FROM jobs WHERE id = $1")
            .bind(&job.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status1, "pending");
    assert_eq!(attempts1, 1);
    let due1 = next_run_at1.expect("next_run_at set after failure");
    assert!(due1 > utc_now_str(), "backoff delays retry: {due1}");

    // Force due + fail #2 → dead
    sqlx::query("UPDATE jobs SET next_run_at = utc_text() WHERE id = $1")
        .bind(&job.id)
        .execute(&pool)
        .await
        .unwrap();
    let job2 = claim_next_job(&pool).await.unwrap().expect("claimed 2");
    assert_eq!(job2.id, job.id);
    fail_job(&pool, &job2.id, "boom two").await.unwrap();

    let (status2, attempts2): (String, i64) =
        sqlx::query_as("SELECT status, attempts FROM jobs WHERE id = $1")
            .bind(&job.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status2, "dead", "retry budget exhausted → dead letter");
    assert_eq!(attempts2, 2);

    assert!(
        claim_next_job(&pool).await.unwrap().is_none(),
        "dead jobs are not claimable"
    );
}

#[tokio::test]
async fn translate_kind_roundtrip() {
    let pool = new_pool().await;
    enqueue_job(&pool, JobKind::Translate, Some("case-x"), "f-tr", json!({}))
        .await
        .unwrap();
    let job = claim_next_job(&pool).await.unwrap().expect("claimed");
    assert_eq!(job.kind, JobKind::Translate);
    assert_eq!(job.case_id.as_deref(), Some("case-x"));
    mark_running(&pool, &job.id).await.unwrap();
    complete_job(&pool, &job.id).await.unwrap();
}

fn utc_now_str() -> String {
    use chrono::Utc;
    Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

// Keep the pool alive long enough for background tasks in multi-thread tests.
#[allow(dead_code)]
fn _touch(_: &sqlx::PgPool) {
    let _ = Duration::from_secs(0);
}
