use std::time::Instant;

/// Token bucket: a burst of `capacity` actions, refilled at `refill_per_sec`.
pub struct RateLimiter {
    capacity: f64,
    refill_per_sec: f64,
    tokens: f64,
    last: Instant,
}

impl RateLimiter {
    pub fn new(capacity: u32, refill_per_sec: f64, now: Instant) -> Self {
        Self {
            capacity: capacity as f64,
            refill_per_sec,
            tokens: capacity as f64,
            last: now,
        }
    }

    /// Takes a token when one is available.
    pub fn allow(&mut self, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last);
        self.last = now;
        self.tokens = (self.tokens + elapsed.as_secs_f64() * self.refill_per_sec).min(self.capacity);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use shine_test::test;

    #[test]
    fn a_burst_is_allowed_then_refused_until_refilled() {
        let start = Instant::now();
        let mut limiter = RateLimiter::new(3, 1.0, start);

        assert!((0..3).all(|_| limiter.allow(start)));
        assert!(!limiter.allow(start));
        assert!(!limiter.allow(start + Duration::from_millis(500)));
        assert!(limiter.allow(start + Duration::from_millis(1000)));
        assert!(!limiter.allow(start + Duration::from_millis(1000)));
    }

    #[test]
    fn idle_time_refills_up_to_the_capacity_only() {
        let start = Instant::now();
        let mut limiter = RateLimiter::new(2, 1.0, start);

        let later = start + Duration::from_secs(3600);
        assert!(limiter.allow(later));
        assert!(limiter.allow(later));
        assert!(!limiter.allow(later), "a long pause must not bank more than the burst");
    }

    #[test]
    fn a_clock_going_backwards_is_harmless() {
        let start = Instant::now() + Duration::from_secs(10);
        let mut limiter = RateLimiter::new(1, 1.0, start);
        assert!(limiter.allow(start));
        assert!(!limiter.allow(start - Duration::from_secs(5)));
    }
}
