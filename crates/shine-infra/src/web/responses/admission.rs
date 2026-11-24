use crate::{sync::AdmissionError, web::responses::Problem};
use axum::http::StatusCode;
use serde_json::json;

/// 429 with the seconds to wait in `retryAfter` when the key asked too often, 503 when there is no place left.
impl From<AdmissionError> for Problem {
    fn from(error: AdmissionError) -> Self {
        match error {
            AdmissionError::RateLimited { retry_after } => {
                Problem::new(StatusCode::TOO_MANY_REQUESTS, "too-many-requests")
                    .with_detail("too many attempts")
                    .with_extension(json!({ "retryAfter": retry_after.as_secs() }))
            }
            AdmissionError::Full => Problem::service_unavailable().with_detail("no place left"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_rate_limited_key_is_a_429_that_tells_when_to_come_back() {
        let problem = Problem::from(AdmissionError::RateLimited {
            retry_after: Duration::from_secs(10),
        });
        assert_eq!(problem.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(problem.extension, json!({ "retryAfter": 10 }));
    }

    #[test]
    fn a_full_admission_is_a_503() {
        assert_eq!(
            Problem::from(AdmissionError::Full).status,
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}
