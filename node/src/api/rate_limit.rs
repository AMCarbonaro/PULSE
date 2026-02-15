//! Simple in-memory rate limiter for API endpoints.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Rate limiter configuration
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Time window
    pub window: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 60,
            window: Duration::from_secs(60),
        }
    }
}

/// Per-key rate limit state
struct RateState {
    count: u32,
    window_start: Instant,
}

/// Thread-safe rate limiter
#[derive(Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    state: Arc<Mutex<HashMap<String, RateState>>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if a request from `key` is allowed. Returns true if allowed.
    pub async fn check(&self, key: &str) -> bool {
        let mut state = self.state.lock().await;
        let now = Instant::now();

        let entry = state.entry(key.to_string()).or_insert(RateState {
            count: 0,
            window_start: now,
        });

        // Reset window if expired
        if now.duration_since(entry.window_start) > self.config.window {
            entry.count = 0;
            entry.window_start = now;
        }

        if entry.count >= self.config.max_requests {
            false
        } else {
            entry.count += 1;
            true
        }
    }

    /// Periodically clean up expired entries (call from a background task)
    pub async fn cleanup(&self) {
        let mut state = self.state.lock().await;
        let now = Instant::now();
        state.retain(|_, v| now.duration_since(v.window_start) <= self.config.window * 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_allows_under_limit() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 5,
            window: Duration::from_secs(60),
        });
        for _ in 0..5 {
            assert!(limiter.check("user1").await);
        }
    }

    #[tokio::test]
    async fn test_blocks_over_limit() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 3,
            window: Duration::from_secs(60),
        });
        assert!(limiter.check("user1").await);
        assert!(limiter.check("user1").await);
        assert!(limiter.check("user1").await);
        assert!(!limiter.check("user1").await);
    }

    #[tokio::test]
    async fn test_separate_keys() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 1,
            window: Duration::from_secs(60),
        });
        assert!(limiter.check("a").await);
        assert!(limiter.check("b").await);
        assert!(!limiter.check("a").await);
        assert!(!limiter.check("b").await);
    }

    #[tokio::test]
    async fn test_window_reset() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 1,
            window: Duration::from_millis(50),
        });
        assert!(limiter.check("k").await);
        assert!(!limiter.check("k").await);
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(limiter.check("k").await);
    }

    #[tokio::test]
    async fn test_cleanup_removes_expired() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 10,
            window: Duration::from_millis(10),
        });
        limiter.check("x").await;
        tokio::time::sleep(Duration::from_millis(30)).await;
        limiter.cleanup().await;
        // After cleanup, entry should be gone; new check starts fresh
        assert!(limiter.check("x").await);
    }

    #[test]
    fn test_default_config() {
        let cfg = RateLimitConfig::default();
        assert_eq!(cfg.max_requests, 60);
        assert_eq!(cfg.window, Duration::from_secs(60));
    }
}
