use tokio::time::{sleep_until, Duration, Instant};

/// A conditional countdown that preserves its deadline until disarmed.
pub struct GraceTimer {
    timeout: Duration,
    deadline: Option<Instant>,
}

impl GraceTimer {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout, deadline: None }
    }

    /// Starts or preserves the countdown when true; cancels it when false.
    pub fn arm_if(&mut self, condition: bool) {
        if condition {
            self.deadline.get_or_insert_with(|| Instant::now() + self.timeout);
        } else {
            self.deadline = None;
        }
    }

    /// Waits for the deadline, or indefinitely while disarmed.
    /// Cancelling this future does not reset the countdown.
    pub async fn wait(&self) {
        match self.deadline {
            Some(deadline) => sleep_until(deadline).await,
            None => std::future::pending().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arming_preserves_the_deadline_and_disarming_allows_a_fresh_countdown() {
        let timeout = Duration::from_secs(10);
        let mut timer = GraceTimer::new(timeout);
        timer.arm_if(true);
        let deadline = timer.deadline.unwrap();
        timer.arm_if(true);
        assert_eq!(timer.deadline, Some(deadline));
        timer.arm_if(false);
        assert_eq!(timer.deadline, None);
        let started = Instant::now();
        timer.arm_if(true);
        assert!(timer.deadline.unwrap() >= started + timeout);
    }

    #[shine_test::test(flavor = "current_thread")]
    async fn wait_is_pending_when_disarmed_and_preserves_the_deadline_when_cancelled() {
        let mut timer = GraceTimer::new(Duration::from_millis(40));
        assert!(tokio::time::timeout(Duration::from_millis(10), timer.wait())
            .await
            .is_err());
        timer.arm_if(true);
        let deadline = timer.deadline.unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(10), timer.wait())
            .await
            .is_err());
        timer.arm_if(true);
        assert_eq!(timer.deadline, Some(deadline));
        tokio::time::timeout(Duration::from_secs(2), timer.wait())
            .await
            .unwrap();
        assert!(Instant::now() >= deadline);
    }
}
