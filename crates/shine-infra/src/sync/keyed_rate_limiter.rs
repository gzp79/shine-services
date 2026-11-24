use crate::sync::{RateLimit, TokenBucket};
use std::{
    collections::HashMap,
    hash::Hash,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// How often the buckets that have refilled are forgotten.
const SWEEP_PERIOD: Duration = Duration::from_secs(60);

struct State<K> {
    limit: RateLimit,
    buckets: HashMap<K, TokenBucket>,
    last_sweep: Instant,
}

/// A [`TokenBucket`] per key, all with the same rate. Cheap to clone; clones share the buckets.
///
/// A bucket that has refilled is the same as no bucket, so it is dropped by a periodic sweep: the memory follows
/// the keys that were active within one refill period, not every key ever seen.
pub struct KeyedRateLimiter<K> {
    state: Arc<Mutex<State<K>>>,
}

impl<K> Clone for KeyedRateLimiter<K> {
    fn clone(&self) -> Self {
        Self { state: self.state.clone() }
    }
}

impl<K: Eq + Hash> KeyedRateLimiter<K> {
    pub fn new(limit: RateLimit) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                limit,
                buckets: HashMap::new(),
                last_sweep: Instant::now(),
            })),
        }
    }

    /// Takes a token of the key's bucket; `false` if it is empty.
    pub fn check(&self, key: K) -> bool {
        self.check_at(key, Instant::now())
    }

    /// Like [`check`](Self::check) with the time given, for tests.
    pub fn check_at(&self, key: K, now: Instant) -> bool {
        let mut state = self.state.lock().unwrap();
        if now.saturating_duration_since(state.last_sweep) >= SWEEP_PERIOD {
            state.buckets.retain(|_, bucket| !bucket.is_full_at(now));
            state.last_sweep = now;
        }
        let limit = state.limit;
        state
            .buckets
            .entry(key)
            .or_insert_with(|| TokenBucket::new(limit, now))
            .try_acquire_at(now)
    }

    /// Number of keys that have a bucket.
    pub fn len(&self) -> usize {
        self.state.lock().unwrap().buckets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn t0() -> Instant {
        Instant::now() + Duration::from_secs(10)
    }

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    #[test]
    fn keys_have_independent_budgets() {
        let t0 = t0();
        let limiter = KeyedRateLimiter::new(RateLimit::new(1, 0.001));
        assert!(limiter.check_at("a", t0));
        assert!(!limiter.check_at("a", t0));
        assert!(limiter.check_at("b", t0));
    }

    #[test]
    fn a_new_key_starts_with_a_full_burst() {
        let t0 = t0();
        let limiter = KeyedRateLimiter::new(RateLimit::new(3, 0.001));
        let granted = (0..5).filter(|_| limiter.check_at(7u32, t0)).count();
        assert_eq!(granted, 3);
    }

    #[test]
    fn a_key_earns_its_tokens_back() {
        let t0 = t0();
        let limiter = KeyedRateLimiter::new(RateLimit::new(1, 1.0));
        assert!(limiter.check_at("a", t0));
        assert!(!limiter.check_at("a", t0));
        assert!(limiter.check_at("a", t0 + secs(1)));
    }

    #[test]
    fn a_clone_shares_the_state() {
        let t0 = t0();
        let limiter = KeyedRateLimiter::new(RateLimit::new(1, 0.001));
        let clone = limiter.clone();
        assert!(limiter.check_at("a", t0));
        assert!(!clone.check_at("a", t0));
    }

    #[test]
    fn idle_keys_are_swept() {
        let t0 = t0();
        let limiter = KeyedRateLimiter::new(RateLimit::new(1, 1.0));
        for user in 0..1000u32 {
            assert!(limiter.check_at(user, t0));
        }
        assert_eq!(limiter.len(), 1000);

        // Every bucket is full again and a sweep period has passed.
        assert!(limiter.check_at(5000, t0 + secs(120)));
        assert_eq!(limiter.len(), 1);
    }

    #[test]
    fn a_key_that_is_not_full_is_kept() {
        let t0 = t0();
        // A token takes 100 s to come back, so a spent burst takes 200 s.
        let limiter = KeyedRateLimiter::new(RateLimit::new(2, 0.01));
        assert!(limiter.check_at("a", t0));
        assert!(limiter.check_at("a", t0));

        assert!(limiter.check_at("b", t0 + secs(120)));
        assert_eq!(limiter.len(), 2, "the spent key stays until it has refilled");
    }
}
