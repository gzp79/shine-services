/// ```
/// # use shine_infra::telemetry::{Flap, FlappingTrace};
/// let mut guard = FlappingTrace::default();
/// assert_eq!(guard.record(true), Flap::Entered);
/// assert_eq!(guard.record(true), Flap::Repeated);
/// assert_eq!(guard.record(false), Flap::Recovered);
/// assert_eq!(guard.record(false), Flap::Steady);
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct FlappingTrace {
    failing: bool,
}

/// What changed, compared to the previous [`FlappingTrace::record`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flap {
    /// The first failure after a success (or since the guard was created).
    Entered,
    /// Still failing, same as last time.
    Repeated,
    /// The first success after a failure.
    Recovered,
    /// Still succeeding, same as last time.
    Steady,
}

impl FlappingTrace {
    pub fn record(&mut self, failed: bool) -> Flap {
        let flap = match (self.failing, failed) {
            (false, true) => Flap::Entered,
            (true, true) => Flap::Repeated,
            (true, false) => Flap::Recovered,
            (false, false) => Flap::Steady,
        };
        self.failing = failed;
        flap
    }
}

/// Records a failed attempt on `$guard` and logs it at `$level` the first time (entering
/// failure), falling back to `debug!` on every repeat. Called through
/// `flap_trace_failed::{warn,error,info}`, not directly.
#[doc(hidden)]
#[macro_export]
macro_rules! __flap_trace_failed {
    ($level:ident, $guard:expr, $($msg:tt)*) => {
        match $guard.record(true) {
            $crate::telemetry::Flap::Entered => ::tracing::$level!($($msg)*),
            _ => ::tracing::debug!($($msg)*),
        }
    };
}

/// Records a successful attempt on `$guard` and logs it at `$level` only if it just recovered
/// from a failure; silent while steady. Called through `flap_trace_recovered::{warn,error,info}`,
/// not directly.
#[doc(hidden)]
#[macro_export]
macro_rules! __flap_trace_recovered {
    ($level:ident, $guard:expr, $($msg:tt)*) => {
        if $guard.record(false) == $crate::telemetry::Flap::Recovered {
            ::tracing::$level!($($msg)*);
        }
    };
}

/// `flap_trace_failed::warn!(guard, msg...)` (also `error!`/`info!`): logs entering failure at
/// the chosen level, `debug!` on a repeat — so an outage logs once instead of once per attempt.
pub mod flap_trace_failed {
    #[macro_export]
    macro_rules! flap_trace_failed_warn {
        ($guard:expr, $($msg:tt)*) => {
            $crate::__flap_trace_failed!(warn, $guard, $($msg)*)
        };
    }
    #[macro_export]
    macro_rules! flap_trace_failed_error {
        ($guard:expr, $($msg:tt)*) => {
            $crate::__flap_trace_failed!(error, $guard, $($msg)*)
        };
    }
    #[macro_export]
    macro_rules! flap_trace_failed_info {
        ($guard:expr, $($msg:tt)*) => {
            $crate::__flap_trace_failed!(info, $guard, $($msg)*)
        };
    }
    pub use crate::{flap_trace_failed_error as error, flap_trace_failed_info as info, flap_trace_failed_warn as warn};
}

/// `flap_trace_recovered::warn!(guard, msg...)` (also `error!`/`info!`): logs recovery at the
/// chosen level once, nothing while steady.
pub mod flap_trace_recovered {
    #[macro_export]
    macro_rules! flap_trace_recovered_warn {
        ($guard:expr, $($msg:tt)*) => {
            $crate::__flap_trace_recovered!(warn, $guard, $($msg)*)
        };
    }
    #[macro_export]
    macro_rules! flap_trace_recovered_error {
        ($guard:expr, $($msg:tt)*) => {
            $crate::__flap_trace_recovered!(error, $guard, $($msg)*)
        };
    }
    #[macro_export]
    macro_rules! flap_trace_recovered_info {
        ($guard:expr, $($msg:tt)*) => {
            $crate::__flap_trace_recovered!(info, $guard, $($msg)*)
        };
    }
    pub use crate::{
        flap_trace_recovered_error as error, flap_trace_recovered_info as info, flap_trace_recovered_warn as warn,
    };
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_fresh_guard_starts_steady() {
        let mut guard = FlappingTrace::default();
        assert_eq!(guard.record(false), Flap::Steady);
    }

    #[test]
    fn failures_transition_once_on_entry_and_repeat_after() {
        let mut guard = FlappingTrace::default();
        assert_eq!(guard.record(true), Flap::Entered);
        assert_eq!(guard.record(true), Flap::Repeated);
        assert_eq!(guard.record(true), Flap::Repeated);
    }

    #[test]
    fn a_recovery_is_reported_once_then_steady() {
        let mut guard = FlappingTrace::default();
        guard.record(true);
        assert_eq!(guard.record(false), Flap::Recovered);
        assert_eq!(guard.record(false), Flap::Steady);
    }

    #[test]
    fn flapping_back_and_forth_reports_every_transition() {
        let mut guard = FlappingTrace::default();
        assert_eq!(guard.record(true), Flap::Entered);
        assert_eq!(guard.record(false), Flap::Recovered);
        assert_eq!(guard.record(true), Flap::Entered);
    }
}
