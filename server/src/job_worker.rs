//! Job workers (P1): background loop that claims and executes durable jobs.
//!
//! Spawn one or more workers (`worker_count`); each runs an independent loop:
//! reset stale leases occasionally, claim the next due job with SKIP LOCKED,
//! execute it (with a lease heartbeat), then record success/failure.
//! Workers stop claiming on shutdown and let in-flight jobs finish.
use crate::jobs::{self, Job, JobKind};
use crate::state::AppState;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const STALE_SWEEP_EVERY: u32 = 15;

/// Spawns `worker_count` worker tasks. Returns a handle that resolves when
/// all workers have exited.
pub fn spawn_job_workers(
    state: AppState,
    shutdown: CancellationToken,
    worker_count: usize,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut handles = Vec::with_capacity(worker_count);
        for id in 0..worker_count {
            let state = state.clone();
            let shutdown = shutdown.clone();
            handles.push(tokio::spawn(async move {
                worker_loop(state, shutdown, id).await;
            }));
        }
        for h in handles {
            let _ = h.await;
        }
    })
}

async fn worker_loop(state: AppState, shutdown: CancellationToken, worker_id: usize) {
    let mut cycle: u32 = 0;
    loop {
        if shutdown.is_cancelled() {
            tracing::info!(worker_id, "worker stopping (shutdown requested)");
            break;
        }

        cycle += 1;
        if cycle % STALE_SWEEP_EVERY == 0 {
            match jobs::reset_stale_jobs(&state.pool).await {
                Ok(recovered) => {
                    if recovered > 0 {
                        tracing::warn!(worker_id, recovered, "recovered stale jobs");
                    }
                }
                Err(err) => {
                    tracing::error!(worker_id, error = %err, "stale job sweep failed");
                }
            }
        }

        match jobs::claim_next_job(&state.pool).await {
            Ok(Some(job)) => {
                if let Err(err) = run_job(&state, job.clone(), shutdown.clone()).await {
                    tracing::error!(
                        worker_id,
                        job_id = %job.id,
                        kind = %job.kind.as_str(),
                        error = %err,
                        "job execution failed"
                    );
                }
            }
            Ok(None) => {
                tokio::time::sleep(POLL_INTERVAL).await;
            }
            Err(err) => {
                tracing::error!(worker_id, error = %err, "job claim failed");
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }
    }
}

async fn run_job(state: &AppState, job: Job, shutdown: CancellationToken) -> anyhow::Result<()> {
    jobs::mark_running(&state.pool, &job.id).await?;
    jobs::record_job_event(&state.pool, &job.id, job.kind, "started", None).await?;

    // Heartbeat keeps the lease alive while the job runs; it stops renewing
    // on shutdown so a graceful drain lets the lease expire (another instance
    // can retake the job if we cannot finish).
    let heartbeat = {
        let pool = state.pool.clone();
        let job_id = job.id.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(jobs::HEARTBEAT_INTERVAL) => {
                        if let Err(err) = jobs::heartbeat_job(&pool, &job_id).await {
                            tracing::warn!(job_id = %job_id, error = %err, "job heartbeat failed");
                        }
                    }
                    _ = shutdown.cancelled() => break,
                }
            }
        })
    };

    let result = match job.kind {
        JobKind::Parse => crate::parser::run_parse_job(state, &job.ref_id).await,
        JobKind::Translate => {
            crate::translation::run_translate_job(state.clone(), &job.ref_id).await
        }
    };

    heartbeat.abort();

    match result {
        Ok(()) => {
            jobs::complete_job(&state.pool, &job.id).await?;
            jobs::record_job_event(&state.pool, &job.id, job.kind, "succeeded", None).await?;
            tracing::info!(job_id = %job.id, kind = %job.kind.as_str(), "job succeeded");
        }
        Err(err) => {
            let message = format!("{err:#}");
            jobs::fail_job(&state.pool, &job.id, &message).await?;
            jobs::record_job_event(
                &state.pool,
                &job.id,
                job.kind,
                "failed",
                Some(&message[..message.len().min(500)]),
            )
            .await?;
            tracing::warn!(job_id = %job.id, kind = %job.kind.as_str(), error = %message, "job failed");
        }
    }
    Ok(())
}
