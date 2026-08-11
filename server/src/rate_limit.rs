//! Shared rate limiting (P3): PostgreSQL-backed counters.
//!
//! Replaces the in-memory limiter so multiple server instances enforce a
//! single limit. The counter uses a fixed window refreshed atomically:
//! `INSERT ... ON CONFLICT DO UPDATE` resets the window when it has expired
//! and increments otherwise, in one statement — safe under concurrency.
use chrono::{Duration, Utc};
use sqlx::PgPool;
use std::time::Duration as StdDuration;

#[derive(Clone)]
pub struct RateLimiter {
    pool: PgPool,
}

impl RateLimiter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Returns `(allowed, retry_after_secs)` with the same contract as the
    /// previous in-memory limiter.
    pub async fn check_and_record(
        &self,
        key: &str,
        max_attempts: usize,
        window: StdDuration,
    ) -> (bool, u64) {
        let window_start_ts = (Utc::now() - Duration::from_std(window).unwrap_or_default())
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let now_ts = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let row = sqlx::query_as::<_, (i64, String)>(
            r#"
            INSERT INTO rate_limit_counters (key, count, window_start)
            VALUES ($1, 1, $2)
            ON CONFLICT (key) DO UPDATE SET
                count = CASE
                    WHEN rate_limit_counters.window_start < $3 THEN 1
                    ELSE rate_limit_counters.count + 1
                END,
                window_start = CASE
                    WHEN rate_limit_counters.window_start < $3 THEN $2
                    ELSE rate_limit_counters.window_start
                END
            RETURNING count, window_start
            "#,
        )
        .bind(key)
        .bind(&now_ts)
        .bind(&window_start_ts)
        .fetch_one(&self.pool)
        .await;

        match row {
            Ok((count, window_start)) => {
                if (count as usize) > max_attempts {
                    // Remaining window seconds from window_start to now.
                    let retry_after = parse_ts(&window_start)
                        .and_then(|start| {
                            let now_naive = Utc::now().naive_utc();
                            let elapsed = (now_naive - start).to_std().ok()?;
                            Some(window.saturating_sub(elapsed).as_secs().saturating_add(1))
                        })
                        .unwrap_or(window.as_secs().saturating_add(1));
                    (false, retry_after)
                } else {
                    (true, 0)
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "rate limiter db error; allowing request");
                (true, 0)
            }
        }
    }

    pub async fn reset(&self, key: &str) {
        let _ = sqlx::query("DELETE FROM rate_limit_counters WHERE key = $1")
            .bind(key)
            .execute(&self.pool)
            .await;
    }
}

fn parse_ts(raw: &str) -> Option<chrono::NaiveDateTime> {
    chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ts_accepts_utc_format() {
        assert!(parse_ts("2026-08-10 12:00:00").is_some());
        assert!(parse_ts("garbage").is_none());
    }
}
