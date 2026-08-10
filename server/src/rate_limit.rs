use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Debug, Default)]
pub struct RateLimiter {
    attempts: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `(allowed, retry_after_secs)`.
    ///
    /// When `allowed` is `false`, `retry_after_secs` is the remaining seconds
    /// of the sliding window the caller must wait before retrying, so the
    /// server can emit a `Retry-After` header.
    pub async fn check_and_record(
        &self,
        key: &str,
        max_attempts: usize,
        window: Duration,
    ) -> (bool, u64) {
        let mut attempts = self.attempts.lock().await;
        let now = Instant::now();

        let user_attempts = attempts.entry(key.to_string()).or_default();
        user_attempts.retain(|t| now.duration_since(*t) < window);

        if user_attempts.len() >= max_attempts {
            // Oldest record is the first to expire; that is when a slot frees up.
            let retry_after = user_attempts
                .first()
                .map(|oldest| {
                    let remaining = window.saturating_sub(now.duration_since(*oldest));
                    remaining.as_secs().saturating_add(1)
                })
                .unwrap_or(0);
            return (false, retry_after);
        }

        user_attempts.push(now);
        (true, 0)
    }

    pub async fn reset(&self, key: &str) {
        let mut attempts = self.attempts.lock().await;
        attempts.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;

    #[tokio::test]
    async fn sliding_window_blocks_after_limit() {
        let limiter = RateLimiter::new();
        let window = StdDuration::from_secs(60);

        for i in 0..5 {
            let (allowed, _) = limiter.check_and_record("user-1", 5, window).await;
            assert!(allowed, "attempt {i} should be allowed");
        }

        let (allowed, retry_after) = limiter.check_and_record("user-1", 5, window).await;
        assert!(!allowed, "6th attempt must be limited");
        assert!(
            retry_after >= 1,
            "retry_after should be >= 1s, got {retry_after}"
        );
    }

    #[tokio::test]
    async fn window_opens_after_oldest_expires() {
        let limiter = RateLimiter::new();
        let window = StdDuration::from_millis(80);

        for _ in 0..2 {
            let (allowed, _) = limiter.check_and_record("user-2", 2, window).await;
            assert!(allowed);
        }
        let (allowed, _) = limiter.check_and_record("user-2", 2, window).await;
        assert!(!allowed);

        tokio::time::sleep(StdDuration::from_millis(100)).await;
        let (allowed, _) = limiter.check_and_record("user-2", 2, window).await;
        assert!(allowed, "expired window slot must free up");
    }

    #[tokio::test]
    async fn keys_are_isolated() {
        let limiter = RateLimiter::new();
        for _ in 0..2 {
            let (allowed, _) = limiter
                .check_and_record("user-a", 2, StdDuration::from_secs(60))
                .await;
            assert!(allowed);
        }
        let (allowed, _) = limiter
            .check_and_record("user-a", 2, StdDuration::from_secs(60))
            .await;
        assert!(!allowed, "user-a must be limited after its own quota");

        let (allowed, _) = limiter
            .check_and_record("user-b", 2, StdDuration::from_secs(60))
            .await;
        assert!(allowed, "different keys must not share the limit");
    }
}
