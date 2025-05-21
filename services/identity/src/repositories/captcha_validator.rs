use serde::{Deserialize, Serialize};
use shine_infra::web::responses::Problem;
use std::sync::Arc;
use thiserror::Error as ThisError;
use uuid::Uuid;

const CAPTCHA_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

const CAPTCHA_FAILED: &str = "captcha-failed-validation";
const CAPTCHA_MISSING: &str = "captcha-not-provided";

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

        let secret = &self.0.secret;
        let (secret, token) = if secret == "1x0000000000000000000000000000000AA" {
            log::warn!("Using test-secret for captcha validation for token {token}");
            // When a test-secret is used, the token is used as a site-key to emulate a passing or failing response
            let test_site_keys = [
                "1x00000000000000000000AA", // Always passes
                //"2x00000000000000000000AB", // Always blocks
                "1x00000000000000000000BB", // Always passes
                //"2x00000000000000000000BB", // Always blocks
                "3x00000000000000000000FF", // Forces an interactive challenge
            ];
            if test_site_keys.contains(&token) {
                log::info!("Using an always passing secret");
                ("1x0000000000000000000000000000000AA", "XXXX.DUMMY.TOKEN.XXXX")
            } else {
                log::info!("Using an always failing secret");
                ("2x0000000000000000000000000000000AA", "XXXX.DUMMY.TOKEN.XXXX")
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
            .map_err(|err| CaptchaError::Request(format!("{:?}", err)))?
            .json::<TurnstileValidationResponse>()
            .await
            .map_err(|err| CaptchaError::Request(format!("{:?}", err)))?;
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
        let validator = CaptchaValidator::new("1x0000000000000000000000000000000AA");
        let token = "1x00000000000000000000AA";
        let response = validator
            .validate_request(token, None)
            .await
            .expect("Validation request failed");
        log::info!("response: {:?}", response);
        assert!(response.success);
    }

    #[test]
    async fn test_captcha_validator_test_token_invalid() {
        let validator = CaptchaValidator::new("2x0000000000000000000000000000000AA");
        let token = "token";
        let response = validator
            .validate_request(token, None)
            .await
            .expect("Validation request failed");
        log::info!("response: {:?}", response);
        assert!(!response.success);
        assert_eq!(response.error_codes, vec!["invalid-input-response"]);
    }

    #[test]
    async fn test_captcha_validator_test_token_expired() {
        let validator = CaptchaValidator::new("3x0000000000000000000000000000000AA");
        let token = "token";
        let response = validator
            .validate_request(token, None)
            .await
            .expect("Validation request failed");
        log::info!("response: {:?}", response);
        assert!(!response.success);
        assert_eq!(response.error_codes, vec!["timeout-or-duplicate"]);
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
            log::info!("response: {:?}", response);
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
            log::info!("response: {:?}", response);
            assert!(!response.success);
            assert_eq!(response.error_codes, vec!["timeout-or-duplicate"]);
        } else {
            log::warn!("CF_CAPTCHA_SECRET not set, skipping test");
        }
    }
}
