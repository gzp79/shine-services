use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use validator::{Validate, ValidationError};

/// A rate: up to `burst` events at once, then `per_second` events a second.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RateLimit {
    #[validate(range(min = 1))]
    pub burst: u32,
    #[validate(custom(function = "positive_rate"))]
    pub per_second: f64,
}

fn positive_rate(value: f64) -> Result<(), ValidationError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(ValidationError::new("rate_must_be_positive"))
    }
}

impl RateLimit {
    pub const fn new(burst: u32, per_second: f64) -> Self {
        Self { burst, per_second }
    }

    /// Time it takes to earn one event back.
    pub fn interval(&self) -> Duration {
        Duration::from_secs_f64(1.0 / self.per_second)
    }
}

/// Token bucket: `try_acquire` takes one token, tokens come back at the rate of the limit up to the burst.
/// It is refilled lazily on each call, so it needs no timer and no lock; share it with `&mut`.
#[derive(Clone, Debug)]
pub struct TokenBucket {
    limit: RateLimit,
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    /// A bucket that is full at `now`.
    pub fn new(limit: RateLimit, now: Instant) -> Self {
        Self {
            limit,
            tokens: f64::from(limit.burst),
            last_refill: now,
        }
    }

    pub fn try_acquire(&mut self) -> bool {
        self.try_acquire_at(Instant::now())
    }

    /// Like [`try_acquire`](Self::try_acquire) with the time given, for tests.
    pub fn try_acquire_at(&mut self, now: Instant) -> bool {
        self.refill(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Whether every token is back, which makes the bucket indistinguishable from a new one.
    pub fn is_full_at(&mut self, now: Instant) -> bool {
        self.refill(now);
        self.tokens >= f64::from(self.limit.burst)
    }

    fn refill(&mut self, now: Instant) {
        // A time before the last refill adds nothing and does not move the clock back.
        if now <= self.last_refill {
            return;
        }
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.limit.per_second).min(f64::from(self.limit.burst));
        self.last_refill = now;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn t0() -> Instant {
        // Far enough from the process start that the tests can step back from it.
        Instant::now() + Duration::from_secs(10)
    }

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    #[test]
    fn grants_the_burst_then_denies() {
        let t0 = t0();
        let mut bucket = TokenBucket::new(RateLimit::new(3, 1.0), t0);
        assert!(bucket.try_acquire_at(t0));
        assert!(bucket.try_acquire_at(t0));
        assert!(bucket.try_acquire_at(t0));
        assert!(!bucket.try_acquire_at(t0));
    }

    #[test]
    fn refills_over_time() {
        let t0 = t0();
        let mut bucket = TokenBucket::new(RateLimit::new(3, 1.0), t0);
        for _ in 0..3 {
            assert!(bucket.try_acquire_at(t0));
        }

        assert!(
            !bucket.try_acquire_at(t0 + Duration::from_millis(500)),
            "half a token is not enough"
        );
        assert!(bucket.try_acquire_at(t0 + secs(1)));
        assert!(!bucket.try_acquire_at(t0 + secs(1)));
        assert!(bucket.try_acquire_at(t0 + secs(2)));
    }

    #[test]
    fn never_exceeds_the_capacity() {
        let t0 = t0();
        let mut bucket = TokenBucket::new(RateLimit::new(3, 1.0), t0);
        let later = t0 + secs(3600);
        let granted = (0..10).filter(|_| bucket.try_acquire_at(later)).count();
        assert_eq!(granted, 3);
    }

    #[test]
    fn a_time_going_backwards_does_not_refill() {
        let t0 = t0();
        let mut bucket = TokenBucket::new(RateLimit::new(1, 1.0), t0);
        assert!(bucket.try_acquire_at(t0));
        assert!(!bucket.try_acquire_at(t0 - secs(1)));
        assert!(!bucket.try_acquire_at(t0));
        assert!(bucket.try_acquire_at(t0 + secs(1)));
    }

    #[test]
    fn is_full_after_the_refill() {
        let t0 = t0();
        let mut bucket = TokenBucket::new(RateLimit::new(2, 1.0), t0);
        assert!(bucket.is_full_at(t0));
        assert!(bucket.try_acquire_at(t0));
        assert!(!bucket.is_full_at(t0));
        assert!(!bucket.is_full_at(t0 + Duration::from_millis(500)));
        assert!(bucket.is_full_at(t0 + secs(1)));
    }

    #[test]
    fn try_acquire_uses_the_clock() {
        let mut bucket = TokenBucket::new(RateLimit::new(1, 0.001), Instant::now());
        assert!(bucket.try_acquire());
        assert!(!bucket.try_acquire());
    }

    #[test]
    fn rate_limit_validation() {
        assert!(RateLimit::new(1, 0.1).validate().is_ok());
        assert!(RateLimit::new(0, 1.0).validate().is_err(), "no burst");
        assert!(RateLimit::new(1, 0.0).validate().is_err(), "no refill");
        assert!(RateLimit::new(1, -1.0).validate().is_err());
        assert!(RateLimit::new(1, f64::NAN).validate().is_err());
        assert!(RateLimit::new(1, f64::INFINITY).validate().is_err());
    }

    #[test]
    fn interval_is_the_inverse_of_the_rate() {
        assert_eq!(RateLimit::new(5, 0.1).interval(), secs(10));
        assert_eq!(RateLimit::new(5, 4.0).interval(), Duration::from_millis(250));
    }
}
