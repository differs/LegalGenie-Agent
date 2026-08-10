-- Reliable task execution (P1): durable job queue for parse/translate/export.
-- Workers claim jobs with `SELECT ... FOR UPDATE SKIP LOCKED` inside a
-- transaction; leases expire on crash and a sweeper retakes them.
CREATE TABLE IF NOT EXISTS jobs (
    id           TEXT PRIMARY KEY,
    kind         TEXT NOT NULL,                -- parse | translate
    case_id      TEXT,
    ref_id       TEXT NOT NULL,                -- evidence file id
    payload      TEXT NOT NULL DEFAULT '{}',   -- json
    status       TEXT NOT NULL DEFAULT 'pending', -- pending|claimed|processing|succeeded|failed|dead
    attempts     BIGINT NOT NULL DEFAULT 0,
    max_attempts BIGINT NOT NULL DEFAULT 5,
    lease_until  TEXT,
    next_run_at  TEXT NOT NULL,
    last_error   TEXT,
    created_at   TEXT NOT NULL DEFAULT utc_text(),
    updated_at   TEXT NOT NULL DEFAULT utc_text()
);

CREATE INDEX IF NOT EXISTS idx_jobs_claim
    ON jobs (status, next_run_at);
CREATE INDEX IF NOT EXISTS idx_jobs_ref
    ON jobs (kind, ref_id);

-- De-duplicate enqueues: only one active job per (kind, ref_id).
CREATE UNIQUE INDEX IF NOT EXISTS idx_jobs_active_unique
    ON jobs (kind, ref_id)
    WHERE status IN ('pending', 'claimed', 'processing');

-- Append-only event trail for job lifecycle.
CREATE TABLE IF NOT EXISTS job_events (
    id         BIGSERIAL PRIMARY KEY,
    job_id     TEXT NOT NULL,
    kind       TEXT NOT NULL,
    event      TEXT NOT NULL,
    detail     TEXT,
    created_at TEXT NOT NULL DEFAULT utc_text()
);

CREATE INDEX IF NOT EXISTS idx_job_events_job
    ON job_events (job_id, created_at);
