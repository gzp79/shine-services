use serde::{Deserialize, Serialize};
use shine_infra::web::responses::Problem;
use std::sync::Arc;
use thiserror::Error as ThisError;
use uuid::Uuid;

const CAPTCHA_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

const CAPTCHA_FAILED: &str = "captcha-failed-validation";
const CAPTCHA_MISSING: &str = "captcha-not-provided";

/// Sentinel captcha secret enabling test mode. It is deliberately NOT a valid Cloudflare
/// secret; when configured, the response is driven by the token instead of Cloudflare's own
/// test keys, so tests can select an outcome deterministically (and mostly offline).
const TEST_SECRET: &str = "0000000000000000000000000000000000";
/// Cloudflare test secret that always passes (used to exercise a real siteverify round-trip).
const CF_TEST_SECRET_PASS: &str = "1x0000000000000000000000000000000AA";
/// Cloudflare test secret that always blocks (used to exercise a real siteverify round-trip).
const CF_TEST_SECRET_BLOCK: &str = "2x0000000000000000000000000000000AA";
/// Dummy response token forwarded to Cloudflare when the test secret drives the outcome.
const CF_DUMMY_TOKEN: &str = "XXXX.DUMMY.TOKEN.XXXX";

/// In test mode (secret == [`TEST_SECRET`]) the incoming token selects the behaviour:
/// - [`TEST_TOKEN_PASS`]  -> real Cloudflare call with the always-passing test secret
/// - [`TEST_TOKEN_BLOCK`] -> real Cloudflare call with the always-blocking test secret
/// - [`TEST_TOKEN_SKIP`]  -> synthetic success, no Cloudflare call (offline)
/// - anything else        -> synthetic `invalid-input-response` failure, no Cloudflare call
const TEST_TOKEN_PASS: &str = "pass";
const TEST_TOKEN_BLOCK: &str = "block";
const TEST_TOKEN_SKIP: &str = "skip";

#[derive(Debug, ThisError)]
pub enum CaptchaError {
    #[error("Request failed with")]
    Request(String),
    #[error("Captcha validation failed")]
    FailedValidation(String),
    #[error("Missing captcha token")]
    MissingCaptcha,
}

impl From<CaptchaError> for Problem {
    fn from(value: CaptchaError) -> Self {
        let detail = value.to_string();

        match value {
            CaptchaError::FailedValidation(err) => Problem::bad_request(CAPTCHA_FAILED)
                .with_detail(detail)
                .with_sensitive_dbg(err),
            CaptchaError::MissingCaptcha => Problem::bad_request(CAPTCHA_MISSING).with_detail(detail),

            _ => Problem::internal_error().with_detail(detail).with_sensitive_dbg(value),
        }
    }
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
    //#[serde(rename = "challenge_ts")]
    //pub challenge_ts: Option<DateTime<Utc>>,
    //#[serde(rename = "hostname")]
    //pub hostname: Option<String>,
    #[serde(rename = "error-codes")]
    pub error_codes: Vec<String>,
    //#[serde(rename = "action")]
    //pub action: Option<String>,
    //#[serde(rename = "cdata")]
    //pub cdata: Option<String>,
}

struct Inner {
    secret: String,
}

#[derive(Clone)]
pub struct CaptchaValidator(Arc<Inner>);

impl CaptchaValidator {
    pub fn new<S: ToString>(secret: S) -> Self {
        Self(Arc::new(Inner { secret: secret.to_string() }))
    }

    pub async fn validate_request(
        &self,
        token: &str,
        remote_ip: Option<&str>,
    ) -> Result<TurnstileValidationResponse, CaptchaError> {
        let idempotency_key = Uuid::new_v4().to_string();

        let (secret, token) = if self.0.secret == TEST_SECRET {
            log::warn!("Using test-secret for captcha validation for token {token}");
            match token {
                TEST_TOKEN_PASS => {
                    log::info!("Test token 'pass': calling Cloudflare with the always-passing test secret");
                    (CF_TEST_SECRET_PASS, CF_DUMMY_TOKEN)
                }
                TEST_TOKEN_BLOCK => {
                    log::info!("Test token 'block': calling Cloudflare with the always-blocking test secret");
                    (CF_TEST_SECRET_BLOCK, CF_DUMMY_TOKEN)
                }
                TEST_TOKEN_SKIP => {
                    log::info!("Test token 'skip': resolving as success without contacting Cloudflare");
                    return Ok(TurnstileValidationResponse {
                        success: true,
                        error_codes: vec![],
                    });
                }
                _ => {
                    log::info!("Unrecognized test token: resolving as failure without contacting Cloudflare");
                    return Ok(TurnstileValidationResponse {
                        success: false,
                        error_codes: vec!["invalid-input-response".to_string()],
                    });
                }
            }
        } else {
            (self.0.secret.as_str(), token)
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
            .map_err(|err| CaptchaError::Request(format!("{err:?}")))?
            .json::<TurnstileValidationResponse>()
            .await
            .map_err(|err| CaptchaError::Request(format!("{err:?}")))?;
        Ok(response)
    }

    pub async fn validate(&self, token: Option<&str>) -> Result<(), CaptchaError> {
        if let Some(token) = token {
            match self.validate_request(token, None).await {
                Ok(result) => {
                    if !result.success {
                        Err(CaptchaError::FailedValidation(result.error_codes.join(", ")))
                    } else {
                        Ok(())
                    }
                }
                Err(err) => Err(CaptchaError::Request(format!("{err:#?}"))),
            }
        } else {
            Err(CaptchaError::MissingCaptcha)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use shine_test::test;

    #[test]
    async fn test_captcha_validator_test_token_pass() {
        let validator = CaptchaValidator::new(TEST_SECRET);
        let response = validator
            .validate_request(TEST_TOKEN_PASS, None)
            .await
            .expect("Validation request failed");
        log::info!("response: {response:?}");
        assert!(response.success);
    }

    #[test]
    async fn test_captcha_validator_test_token_block() {
        let validator = CaptchaValidator::new(TEST_SECRET);
        let response = validator
            .validate_request(TEST_TOKEN_BLOCK, None)
            .await
            .expect("Validation request failed");
        log::info!("response: {response:?}");
        assert!(!response.success);
    }

    #[test]
    async fn test_captcha_validator_test_token_skip() {
        let validator = CaptchaValidator::new(TEST_SECRET);
        let response = validator
            .validate_request(TEST_TOKEN_SKIP, None)
            .await
            .expect("Validation request failed");
        log::info!("response: {response:?}");
        assert!(response.success);
        assert!(response.error_codes.is_empty());
    }

    #[test]
    async fn test_captcha_validator_test_token_invalid() {
        let validator = CaptchaValidator::new(TEST_SECRET);
        let response = validator
            .validate_request("token", None)
            .await
            .expect("Validation request failed");
        log::info!("response: {response:?}");
        assert!(!response.success);
        assert_eq!(response.error_codes, vec!["invalid-input-response"]);
    }

    #[test]
    async fn test_captcha_validator_real_token_invalid() {
        if let Ok(secret) = std::env::var("CF_CAPTCHA_SECRET") {
            let validator = CaptchaValidator::new(secret);

            let token = "invalid";
            let response = validator
                .validate_request(token, None)
                .await
                .expect("Validation request failed");
            log::info!("response: {response:?}");
            assert!(!response.success);
            assert_eq!(response.error_codes, vec!["invalid-input-response"]);
        } else {
            log::warn!("CF_CAPTCHA_SECRET not set, skipping test");
        }
    }

    #[test]
    async fn test_captcha_validator_real_token_expired() {
        if let Ok(secret) = std::env::var("CF_CAPTCHA_SECRET") {
            let validator = CaptchaValidator::new(secret);

            let token = "0.xVJwSGxsgonZ5dcUUCehmsoDATadFvQcJHYJ2T77vggEXA0EfzqtYQKk8dRdGgdZQieN1Cdh9TR1BCd3jU80Tkq_wBt5jdhvvMeGQNDtNRbkyj4W_Tp2_kEFRfQRWmnNA56MC2jpaNbi74OD3Ixz52koRwBkbaKWukRnHyxtQ80gkm2Uv_rnJsxFbsQurrs1JBy2azoc5zdW7esOi9gZEhwBhhXbnyj7u3Pu0Ui2ywe7ehfuU1-1dtzEMM9Gt2jSm8qYSD2AvYr2-CIUj8kIXbi5K9Z8tibclvQgePsdWo7mgMkQkpDzUKZwLpUUkqBSgP-wvcsdRS_El487aHUBjrIhVCqtaca_mCi7vIQNDSXFjzhn7_ffhzxcGZUeCj13vDjkCOcHZdtx9pJWd_G6Ir9pul0XXo60QEJJkzgxKUY3cYPaxsAhpPLvq3yfRvP7.tJWm1L0wA8I5zg2c1vPVTg.3125c6192bcc80a18596acdb789c53362fd48b71cde2ceaa0206b1e44c22f2e8";
            let response = validator
                .validate_request(token, None)
                .await
                .expect("Validation request failed");
            log::info!("response: {response:?}");
            assert!(!response.success);
            assert_eq!(response.error_codes, vec!["timeout-or-duplicate"]);
        } else {
            log::warn!("CF_CAPTCHA_SECRET not set, skipping test");
        }
    }
}
