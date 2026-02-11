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
