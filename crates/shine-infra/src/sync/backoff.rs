use rand::{rngs::SysRng, TryRng};
use tokio::time::{sleep, Duration};

/// "Equal jitter": delay drawn from `[base/2, base]`.
const DEFAULT_JITTER: f64 = 0.5;

/// Doubles the base delay on every attempt by default.
const DEFAULT_GROWTH: f64 = 2.0;

/// Exponential backoff with jitter for retry loops (e.g. connection keep-alive tasks).
#[derive(Clone, Debug)]
pub struct ExponentialBackoff {
    min: Duration,
    max: Duration,
    factor: f64,
    growth: f64,
    base: Duration,
}

impl ExponentialBackoff {
    /// Backoff from `min` to `max` (clamped to `min >= 1µs`, `max >= min`) with default jitter and
    /// growth (doubling).
    pub fn new(min: Duration, max: Duration) -> Self {
        let min = min.max(Duration::from_micros(1));
        let max = max.max(min);
        Self {
            min,
            max,
            factor: DEFAULT_JITTER,
            growth: DEFAULT_GROWTH,
            base: min,
        }
    }

    /// Sets the jitter fraction, clamped to `[0, 1]` (`0` disables, `1` is full jitter).
    pub fn with_jitter(mut self, factor: f64) -> Self {
        self.factor = factor.clamp(0.0, 1.0);
        self
    }

    /// Sets the per-attempt growth multiplier, clamped to at least `1.0` (`1.0` disables growth:
    /// every attempt waits `min`, jittered). Default is `2.0` (doubling); a milder ramp (e.g. `1.5`)
    /// retries sooner under light contention while still backing off under sustained contention.
    pub fn with_growth(mut self, growth: f64) -> Self {
        self.growth = growth.max(1.0);
        self
    }

    /// Returns the base to `min`; call after a stable/successful operation.
    pub fn reset(&mut self) {
        self.base = self.min;
    }

    /// Sleeps the current (jittered) delay, then grows the base for the next call, capped at `max`.
    /// Jitter spreads the delay over `[base * (1 - factor), base]` so instances retrying against the
    /// same resource don't reconnect in lockstep.
    pub async fn delay(&mut self) {
        sleep(self.next_delay()).await;
    }

    fn next_delay(&mut self) -> Duration {
        let delay = Self::jitter(self.base, self.factor);
        self.base = self.base.mul_f64(self.growth).min(self.max);
        delay
    }

    /// Draws a delay from `[base * (1 - factor), base]`.
    fn jitter(base: Duration, factor: f64) -> Duration {
        if factor <= 0.0 {
            return base;
        }
        let base = base.as_secs_f64();
        let fixed = base * (1.0 - factor);
        Duration::from_secs_f64(fixed + random_fraction() * base * factor)
    }
}

/// A uniform `f64` in `[0, 1)`, falling back to `0.0` if the system RNG is unavailable — jitter is a
/// fairness optimization, never a correctness requirement.
fn random_fraction() -> f64 {
    let mut bytes = [0u8; 8];
    if SysRng.try_fill_bytes(&mut bytes).is_err() {
        return 0.0;
    }
    // Top 53 bits give a uniform double in [0, 1).
    (u64::from_le_bytes(bytes) >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn doubles_and_caps_at_max() {
        let mut backoff =
            ExponentialBackoff::new(Duration::from_millis(100), Duration::from_millis(800)).with_jitter(0.0);
        assert_eq!(backoff.next_delay(), Duration::from_millis(100));
        assert_eq!(backoff.next_delay(), Duration::from_millis(200));
        assert_eq!(backoff.next_delay(), Duration::from_millis(400));
        assert_eq!(backoff.next_delay(), Duration::from_millis(800));
        assert_eq!(backoff.next_delay(), Duration::from_millis(800));
    }

    #[test]
    fn reset_returns_to_min() {
        let mut backoff =
            ExponentialBackoff::new(Duration::from_millis(100), Duration::from_millis(800)).with_jitter(0.0);
        backoff.next_delay();
        backoff.next_delay();
        backoff.reset();
        assert_eq!(backoff.next_delay(), Duration::from_millis(100));
    }

    #[test]
    fn jitter_stays_within_bounds() {
        // min == max fixes the base so only jitter varies the delay.
        let base = Duration::from_millis(1000);
        let mut backoff = ExponentialBackoff::new(base, base).with_jitter(0.5);
        for _ in 0..1000 {
            let delay = backoff.next_delay();
            assert!(
                delay >= Duration::from_millis(500) && delay <= base,
                "delay {delay:?} out of [500ms, 1000ms]"
            );
        }
    }

    #[test]
    fn max_is_clamped_to_at_least_min() {
        let mut backoff =
            ExponentialBackoff::new(Duration::from_millis(500), Duration::from_millis(100)).with_jitter(0.0);
        assert_eq!(backoff.next_delay(), Duration::from_millis(500));
        assert_eq!(backoff.next_delay(), Duration::from_millis(500));
    }
}
