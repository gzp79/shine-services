use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error as ThisError;
use uuid::Uuid;

const CAPTCHA_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

#[derive(Debug, ThisError, Serialize)]
pub enum CaptchaError {
    #[error("Request failed with {0}")]
    Request(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct TurnstileValidationRequest<'a> {
    #[serde(rename = "secret")]
    pub secret: &'a str,

    #[serde(rename = "response")]
    pub response: &'a str,

    #[serde(rename = "remoteip")]
    pub remote_ip: Option<&'a str>,

    #[serde(rename = "idempotency_key")]
    pub idempotency_key: Option<&'a str>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TurnstileValidationResponse {
    #[serde(rename = "success")]
    pub success: bool,

    #[serde(rename = "challenge_ts")]
    pub challenge_ts: Option<DateTime<Utc>>,

    #[serde(rename = "hostname")]
    pub hostname: Option<String>,

    #[serde(rename = "error-codes")]
    pub error_codes: Vec<String>,

    #[serde(rename = "action")]
    pub action: Option<String>,

    #[serde(rename = "cdata")]
    pub cdata: Option<String>,
}

struct Inner {
    secret: String,
}

#[derive(Clone)]
pub struct CaptchaValidator(Arc<Inner>);

impl CaptchaValidator {
    pub fn new(secret: String) -> Self {
        Self(Arc::new(Inner { secret }))
    }

    pub async fn validate(
        &self,
        token: &str,
        remote_ip: Option<&str>,
    ) -> Result<TurnstileValidationResponse, CaptchaError> {
        let idempotency_key = Uuid::new_v4().to_string();

        let secret = &self.0.secret;
        let secret = if secret == "1x0000000000000000000000000000000AA" {
            // with the testing secret allow all the test token to pass and fail everything else
            let test_tokens = [
                "1x00000000000000000000AA",
                "2x00000000000000000000AB",
                "1x00000000000000000000BB",
                "2x00000000000000000000BB",
                "3x00000000000000000000FF",
            ];
            if test_tokens.contains(&token) {
                "1x0000000000000000000000000000000AA"
            } else {
                "2x0000000000000000000000000000000AA"
            }
        } else {
            &self.0.secret
        };

        let request = TurnstileValidationRequest {
            response: token,
            remote_ip,
            secret,
            idempotency_key: Some(&idempotency_key),
        };

        let client = reqwest::Client::new();
        let response = client
            .post(CAPTCHA_URL)
            .form(&request)
            .send()
            .await
            .map_err(|err| CaptchaError::Request(format!("{:?}", err)))?
            .json::<TurnstileValidationResponse>()
            .await
            .map_err(|err| CaptchaError::Request(format!("{:?}", err)))?;
        Ok(response)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use shine_test::test;

    #[test]
    async fn test_captcha_validator_test_tokens_pass() {
        let validator = CaptchaValidator::new("1x0000000000000000000000000000000AA".into());
        let token = "token";
        let response = validator
            .validate(token, None)
            .await
            .expect("Validation request failed");
        log::info!("response: {:?}", response);
        assert!(response.success);
    }

    #[test]
    async fn test_captcha_validator_test_tokens_fail() {
        let validator = CaptchaValidator::new("2x0000000000000000000000000000000AA".into());
        let token = "token";
        let response = validator
            .validate(token, None)
            .await
            .expect("Validation request failed");
        log::info!("response: {:?}", response);
        assert!(!response.success);
        assert_eq!(response.error_codes, vec!["invalid-input-response"]);
    }

    #[test]
    async fn test_captcha_validator_test_tokens_expired() {
        let validator = CaptchaValidator::new("3x0000000000000000000000000000000AA".into());
        let token = "token";
        let response = validator
            .validate(token, None)
            .await
            .expect("Validation request failed");
        log::info!("response: {:?}", response);
        assert!(!response.success);
        assert_eq!(response.error_codes, vec!["timeout-or-duplicate"]);
    }

    #[test]
    async fn test_captcha_validator_invalid_token() {
        if let Ok(secret) = std::env::var("CF_CAPTCHA_SECRET") {
            let validator = CaptchaValidator::new(secret);

            let token = "invalid";
            let response = validator
                .validate(token, None)
                .await
                .expect("Validation request failed");
            log::info!("response: {:?}", response);
            assert!(!response.success);
            assert_eq!(response.error_codes, vec!["invalid-input-response"]);
        } else {
            log::warn!("CF_CAPTCHA_SECRET not set, skipping test");
        }
    }

    #[test]
    async fn test_captcha_validator_expired_token() {
        if let Ok(secret) = std::env::var("CF_CAPTCHA_SECRET") {
            let validator = CaptchaValidator::new(secret);

            let token = "0.xVJwSGxsgonZ5dcUUCehmsoDATadFvQcJHYJ2T77vggEXA0EfzqtYQKk8dRdGgdZQieN1Cdh9TR1BCd3jU80Tkq_wBt5jdhvvMeGQNDtNRbkyj4W_Tp2_kEFRfQRWmnNA56MC2jpaNbi74OD3Ixz52koRwBkbaKWukRnHyxtQ80gkm2Uv_rnJsxFbsQurrs1JBy2azoc5zdW7esOi9gZEhwBhhXbnyj7u3Pu0Ui2ywe7ehfuU1-1dtzEMM9Gt2jSm8qYSD2AvYr2-CIUj8kIXbi5K9Z8tibclvQgePsdWo7mgMkQkpDzUKZwLpUUkqBSgP-wvcsdRS_El487aHUBjrIhVCqtaca_mCi7vIQNDSXFjzhn7_ffhzxcGZUeCj13vDjkCOcHZdtx9pJWd_G6Ir9pul0XXo60QEJJkzgxKUY3cYPaxsAhpPLvq3yfRvP7.tJWm1L0wA8I5zg2c1vPVTg.3125c6192bcc80a18596acdb789c53362fd48b71cde2ceaa0206b1e44c22f2e8";
            let response = validator
                .validate(token, None)
                .await
                .expect("Validation request failed");
            log::info!("response: {:?}", response);
            assert!(!response.success);
            assert_eq!(response.error_codes, vec!["timeout-or-duplicate"]);
        } else {
            log::warn!("CF_CAPTCHA_SECRET not set, skipping test");
        }
    }
}
