use dashmap::DashMap;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const MAX_TRACKED_KEYS: usize = 4_096;
const MAX_ATTEMPTS: usize = 5;
const ATTEMPT_WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
pub struct AuthRateLimiter {
    attempts: DashMap<String, VecDeque<Instant>>,
}

impl AuthRateLimiter {
    pub fn check(&self, key: &str) -> bool {
        self.check_with_limit(key, MAX_ATTEMPTS)
    }

    pub fn check_with_limit(&self, key: &str, limit: usize) -> bool {
        self.check_at(key, limit, Instant::now())
    }

    fn check_at(&self, key: &str, limit: usize, now: Instant) -> bool {
        if limit == 0 {
            return false;
        }
        if self.attempts.len() >= MAX_TRACKED_KEYS && !self.attempts.contains_key(key) {
            self.attempts.retain(|_, attempts| {
                attempts
                    .back()
                    .is_some_and(|attempt| now.saturating_duration_since(*attempt) < ATTEMPT_WINDOW)
            });
            if self.attempts.len() >= MAX_TRACKED_KEYS {
                return false;
            }
        }

        let mut attempts = self.attempts.entry(key.to_owned()).or_default();
        while attempts
            .front()
            .is_some_and(|attempt| now.duration_since(*attempt) >= ATTEMPT_WINDOW)
        {
            attempts.pop_front();
        }
        if attempts.len() >= limit {
            return false;
        }
        attempts.push_back(now);
        true
    }

    pub fn clear(&self, key: &str) {
        self.attempts.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_attempts_and_recovers_after_the_window() {
        let limiter = AuthRateLimiter::default();
        let start = Instant::now();
        for _ in 0..MAX_ATTEMPTS {
            assert!(limiter.check_at("login:player", MAX_ATTEMPTS, start));
        }
        assert!(!limiter.check_at("login:player", MAX_ATTEMPTS, start));
        assert!(limiter.check_at("login:player", MAX_ATTEMPTS, start + ATTEMPT_WINDOW));
    }

    #[test]
    fn clearing_a_successful_key_resets_its_budget() {
        let limiter = AuthRateLimiter::default();
        for _ in 0..MAX_ATTEMPTS {
            assert!(limiter.check("register:player"));
        }
        limiter.clear("register:player");
        assert!(limiter.check("register:player"));
    }

    #[test]
    fn expired_keys_do_not_permanently_exhaust_the_registry() {
        let limiter = AuthRateLimiter::default();
        let start = Instant::now();
        for key in 0..MAX_TRACKED_KEYS {
            assert!(limiter.check_at(&format!("key:{key}"), 1, start));
        }
        assert!(!limiter.check_at("new-key", 1, start));
        assert!(limiter.check_at("new-key", 1, start + ATTEMPT_WINDOW));
    }
}
