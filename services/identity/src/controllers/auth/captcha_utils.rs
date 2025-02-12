use crate::{app_state::AppState, controllers::auth::AuthError, repositories::CaptchaValidator};

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
                        Err(AuthError::Captcha {
                            error: result.error_codes.join(", "),
                        })
                    } else {
                        Ok(())
                    }
                }
                Err(err) => Err(AuthError::CaptchaServiceError {
                    error: format!("{err:#?}"),
                }),
            }
        } else {
            Err(AuthError::Captcha {
                error: "missing captcha".to_string(),
            })
        }
    }
}
