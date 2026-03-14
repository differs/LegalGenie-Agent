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

    /// Returns `true` if allowed, `false` if limited.
    pub async fn check_and_record(&self, key: &str, max_attempts: usize, window: Duration) -> bool {
        let mut attempts = self.attempts.lock().await;
        let now = Instant::now();

        let user_attempts = attempts.entry(key.to_string()).or_default();
        user_attempts.retain(|t| now.duration_since(*t) < window);

        if user_attempts.len() >= max_attempts {
            return false;
        }

        user_attempts.push(now);
        true
    }
}
