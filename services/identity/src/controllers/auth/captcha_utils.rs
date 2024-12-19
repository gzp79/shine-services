use crate::{
    controllers::{auth::AuthError, AppState},
    repositories::CaptchaValidator,
};

pub struct CaptchaUtils<'a> {
    captcha_validator: &'a CaptchaValidator,
}

impl<'a> CaptchaUtils<'a> {
    pub fn new(app_state: &'a AppState) -> Self {
        Self {
            captcha_validator: app_state.captcha_validator(),
        }
    }

    pub async fn validate(&self, token: Option<&str>) -> Result<(), AuthError> {
        if let Some(token) = token {
            match self.captcha_validator.validate(token, None).await {
                Ok(result) => {
                    if !result.success {
                        Err(AuthError::Captcha(result.error_codes.join(", ")))
                    } else {
                        Ok(())
                    }
                }
                Err(err) => Err(AuthError::CaptchaServiceError(format!("{err}"))),
            }
        } else {
            Err(AuthError::Captcha("missing".to_string()))
        }
    }
}
