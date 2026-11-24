use crate::sync::{KeyedRateLimiter, RateLimit};
use std::{hash::Hash, sync::Arc, time::Duration};
use thiserror::Error as ThisError;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// A place among those an [`Admission`] hands out. Whoever is admitted holds it; dropping it frees the place.
#[derive(Debug)]
pub struct AdmissionPermit(#[allow(dead_code)] OwnedSemaphorePermit);

/// Why somebody was not admitted.
#[derive(Debug, PartialEq, Eq, ThisError)]
pub enum AdmissionError {
    /// The key asked too often.
    #[error("too many attempts, retry after {} s", .retry_after.as_secs())]
    RateLimited { retry_after: Duration },
    /// There is no place left.
    #[error("no place left")]
    Full,
}

impl AdmissionError {
    /// A stable name, for metric labels.
    pub fn as_str(&self) -> &'static str {
        match self {
            AdmissionError::RateLimited { .. } => "rate_limited",
            AdmissionError::Full => "capacity",
        }
    }
}

/// Decides who is let in: a key has to be within its rate and there has to be a place left.
///
/// Every attempt costs a token of the key, a refused one too, so the budget of a key is not in what it was
/// admitted to and survives it. The places are taken at the admission, not when the admitted starts to work, so
/// whatever waits to start is counted too.
pub struct Admission<K> {
    limiter: KeyedRateLimiter<K>,
    retry_after: Duration,
    places: Arc<Semaphore>,
}

impl<K: Eq + Hash> Admission<K> {
    pub fn new(rate: RateLimit, places: usize) -> Self {
        Self {
            limiter: KeyedRateLimiter::new(rate),
            retry_after: Duration::from_secs(rate.interval().as_secs_f64().ceil() as u64),
            places: Arc::new(Semaphore::new(places)),
        }
    }

    pub fn admit(&self, key: K) -> Result<AdmissionPermit, AdmissionError> {
        if !self.limiter.check(key) {
            return Err(AdmissionError::RateLimited { retry_after: self.retry_after });
        }
        self.places
            .clone()
            .try_acquire_owned()
            .map(AdmissionPermit)
            .map_err(|_| AdmissionError::Full)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// A rate that does not refill within a test.
    const NO_REFILL: f64 = 0.001;

    #[test]
    fn the_budget_of_a_key_outlives_its_permits() {
        let admission = Admission::new(RateLimit::new(2, NO_REFILL), 10);

        // The admitted are gone at once, the budget is not.
        drop(admission.admit(1).unwrap());
        drop(admission.admit(1).unwrap());
        assert_eq!(
            admission.admit(1).err(),
            Some(AdmissionError::RateLimited {
                retry_after: Duration::from_secs(1000)
            })
        );
        assert!(admission.admit(2).is_ok(), "the budget is per key");
    }

    #[test]
    fn the_places_are_limited_and_a_dropped_permit_frees_its_place() {
        let admission = Admission::new(RateLimit::new(100, 100.0), 2);
        let first = admission.admit(1).unwrap();
        let _second = admission.admit(2).unwrap();

        assert_eq!(admission.admit(3).err(), Some(AdmissionError::Full));
        drop(first);
        assert!(admission.admit(3).is_ok());
    }

    #[test]
    fn an_attempt_refused_for_the_places_still_costs_a_token() {
        let admission = Admission::new(RateLimit::new(2, NO_REFILL), 1);
        let holder = admission.admit(1).unwrap();

        assert_eq!(admission.admit(2).err(), Some(AdmissionError::Full));
        drop(holder);
        drop(admission.admit(2).unwrap());
        assert!(matches!(
            admission.admit(2).err(),
            Some(AdmissionError::RateLimited { .. })
        ));
    }
}
