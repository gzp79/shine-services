use crate::routes::ws::ws_config::SocketLimits;
use shine_infra::sync::TokenBucket;
use std::time::Instant;

/// What a frame of the client costs from the budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetKind {
    /// A frame token, cheap and the bulk of the traffic.
    Command,
    /// An expensive operation that consumes a subscribe token.
    Subscribe,
}

/// What to do with a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Handle it.
    Allow,
    /// Refuse it, the socket stays.
    Deny,
    /// Refuse it and drop the socket: too many frames in a row were denied.
    Close,
}

/// The budget of one socket for the frames its client sends. Owned by the receive task of the socket.
pub struct SocketBudget {
    frames: TokenBucket,
    subscribes: TokenBucket,
    violations: u32,
    violation_limit: u32,
}

impl SocketBudget {
    pub fn new(limits: &SocketLimits) -> Self {
        Self::new_at(limits, Instant::now())
    }

    pub fn new_at(limits: &SocketLimits, now: Instant) -> Self {
        Self {
            frames: TokenBucket::new(limits.frames, now),
            subscribes: TokenBucket::new(limits.subscribes, now),
            violations: 0,
            violation_limit: limits.violation_limit,
        }
    }

    pub fn check(&mut self, kind: BudgetKind) -> Verdict {
        self.check_at(Instant::now(), kind)
    }

    /// Like [`check`](Self::check) with the time given, for tests.
    pub fn check_at(&mut self, now: Instant, kind: BudgetKind) -> Verdict {
        let bucket = match kind {
            BudgetKind::Command => &mut self.frames,
            BudgetKind::Subscribe => &mut self.subscribes,
        };
        if bucket.try_acquire_at(now) {
            // A grant forgives one denial: a flood that now and then gets a frame through is still closed, a
            // client that now and then runs into its limit is not.
            self.violations = self.violations.saturating_sub(1);
            return Verdict::Allow;
        }
        self.violations += 1;
        if self.violations >= self.violation_limit {
            Verdict::Close
        } else {
            Verdict::Deny
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use shine_infra::sync::RateLimit;
    use shine_test::test;
    use std::time::Duration;

    /// A rate that does not refill within a test.
    const NO_REFILL: f64 = 0.001;

    fn t0() -> Instant {
        Instant::now() + Duration::from_secs(10)
    }

    fn budget(frames: RateLimit, subscribes: RateLimit, violation_limit: u32, now: Instant) -> SocketBudget {
        SocketBudget::new_at(
            &SocketLimits {
                frames,
                subscribes,
                violation_limit,
            },
            now,
        )
    }

    #[test]
    fn frames_are_allowed_up_to_the_burst() {
        let t0 = t0();
        let mut budget = budget(RateLimit::new(3, NO_REFILL), RateLimit::new(1, NO_REFILL), 100, t0);
        let verdicts: Vec<_> = (0..4).map(|_| budget.check_at(t0, BudgetKind::Command)).collect();
        assert_eq!(
            verdicts,
            [Verdict::Allow, Verdict::Allow, Verdict::Allow, Verdict::Deny]
        );
    }

    #[test]
    fn a_subscribe_over_its_budget_is_denied_while_other_frames_still_pass() {
        let t0 = t0();
        let mut budget = budget(RateLimit::new(10, NO_REFILL), RateLimit::new(1, NO_REFILL), 100, t0);
        assert_eq!(budget.check_at(t0, BudgetKind::Subscribe), Verdict::Allow);
        assert_eq!(budget.check_at(t0, BudgetKind::Subscribe), Verdict::Deny);
        assert_eq!(budget.check_at(t0, BudgetKind::Command), Verdict::Allow);
    }

    #[test]
    fn a_subscribe_and_the_other_frames_draw_on_their_own_buckets() {
        let t0 = t0();
        let mut budget = budget(RateLimit::new(1, NO_REFILL), RateLimit::new(1, NO_REFILL), 100, t0);
        let verdicts = [
            budget.check_at(t0, BudgetKind::Command),
            budget.check_at(t0, BudgetKind::Command),
            budget.check_at(t0, BudgetKind::Subscribe),
            budget.check_at(t0, BudgetKind::Subscribe),
        ];
        // The empty frame bucket does not stop a subscribe, and the spent subscribe bucket does not stop a frame.
        assert_eq!(verdicts, [Verdict::Allow, Verdict::Deny, Verdict::Allow, Verdict::Deny]);
        assert_eq!(budget.check_at(t0, BudgetKind::Command), Verdict::Deny);
    }

    #[test]
    fn the_socket_is_closed_at_the_violation_limit() {
        let t0 = t0();
        let mut budget = budget(RateLimit::new(1, NO_REFILL), RateLimit::new(1, NO_REFILL), 3, t0);
        let verdicts: Vec<_> = (0..4).map(|_| budget.check_at(t0, BudgetKind::Command)).collect();
        assert_eq!(verdicts, [Verdict::Allow, Verdict::Deny, Verdict::Deny, Verdict::Close]);
    }

    #[test]
    fn a_granted_frame_forgives_a_denial() {
        let t0 = t0();
        let mut budget = budget(RateLimit::new(1, 1.0), RateLimit::new(1, NO_REFILL), 2, t0);
        // An occasional denial between grants never adds up to a close.
        for second in 0..10 {
            let now = t0 + Duration::from_secs(second);
            assert_eq!(budget.check_at(now, BudgetKind::Command), Verdict::Allow);
            assert_eq!(budget.check_at(now, BudgetKind::Command), Verdict::Deny);
        }
    }

    #[test]
    fn a_flood_that_gets_a_grant_now_and_then_is_closed_all_the_same() {
        let t0 = t0();
        // Four frames for every token that comes back: three denials for each grant.
        let mut budget = budget(RateLimit::new(1, 1.0), RateLimit::new(1, NO_REFILL), 5, t0);
        let closed = (0..40)
            .map(|n| budget.check_at(t0 + Duration::from_millis(250) * n, BudgetKind::Command))
            .any(|verdict| verdict == Verdict::Close);
        assert!(closed);
    }
}
