use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use dashmap::DashMap;

/// Sliding window in-memory rate limiter (pod local).
#[derive(Clone)]
pub struct InMemoryRateLimiter {
    store: Arc<DashMap<String, VecDeque<Instant>>>,
    pub enabled: bool,
}

impl InMemoryRateLimiter {
    pub fn new(enabled: bool) -> Self {
        Self { store: Arc::new(DashMap::new()), enabled }
    }

    /// Returns true if allowed, false if limited.
    pub fn check(&self, key: &str, limit: usize, window: Duration) -> bool {
        if !self.enabled { return true; }
        let now = Instant::now();
        let mut entry = self.store.entry(key.to_string()).or_default();
        while let Some(front) = entry.front() {
            if now.duration_since(*front) >= window { entry.pop_front(); } else { break; }
        }
        if entry.len() < limit {
            entry.push_back(now);
            true
        } else {
            false
        }
    }
}

/// Convenience wrapper holding per-action config derived from env.
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    pub thread_limit: usize,
    pub thread_window: Duration,
    pub reply_limit: usize,
    pub reply_window: Duration,
    pub image_limit: usize,
    pub image_window: Duration,
}

impl RateLimitConfig {
    pub fn from_env() -> Self {
        fn usize_env(name: &str, default: usize) -> usize { std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default) }
        fn dur_env(name: &str, default: u64) -> Duration { Duration::from_secs(std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)) }
        Self {
            thread_limit: usize_env("RL_THREAD_LIMIT", 1),
            thread_window: dur_env("RL_THREAD_WINDOW", 300),
            reply_limit: usize_env("RL_REPLY_LIMIT", 10),
            reply_window: dur_env("RL_REPLY_WINDOW", 60),
            image_limit: usize_env("RL_IMAGE_LIMIT", 5),
            image_window: dur_env("RL_IMAGE_WINDOW", 3600),
        }
    }
}

/// High level guard used by handlers.
#[derive(Clone)]
pub struct RateLimiterFacade {
    pub limiter: InMemoryRateLimiter,
    pub cfg: RateLimitConfig,
}

impl RateLimiterFacade {
    pub fn new(limiter: InMemoryRateLimiter, cfg: RateLimitConfig) -> Self { Self { limiter, cfg } }
    pub fn allow_thread(&self, ip: &str) -> bool { self.limiter.check(&format!("thread:{ip}"), self.cfg.thread_limit, self.cfg.thread_window) }
    pub fn allow_reply(&self, ip: &str) -> bool { self.limiter.check(&format!("reply:{ip}"), self.cfg.reply_limit, self.cfg.reply_window) }
    pub fn allow_image(&self, ip: &str) -> bool { self.limiter.check(&format!("image:{ip}"), self.cfg.image_limit, self.cfg.image_window) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sliding_window_basic() {
        let rl = InMemoryRateLimiter::new(true);
        let window = Duration::from_millis(50);
        for _ in 0..3 { assert!(rl.check("k", 3, window)); }
        assert!(!rl.check("k", 3, window));
    }
}
