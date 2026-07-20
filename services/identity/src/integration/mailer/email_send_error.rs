use shine_infra::web::responses::Problem;
use thiserror::Error as ThisError;

const INVALID_EMAIL: &str = "email-invalid-address";
const INVALID_CONTENT: &str = "email-invalid-content";

#[derive(ThisError, Debug)]
pub enum EmailSenderError {
    #[error("Invalid email address")]
    InvalidEmailAddress,
    #[error("Invalid content")]
    InvalidContent,
    #[error("Email send failed")]
    SendFailed(String),
}

impl From<EmailSenderError> for Problem {
    fn from(err: EmailSenderError) -> Self {
        match err {
            EmailSenderError::InvalidEmailAddress => Problem::bad_request(INVALID_EMAIL).with_detail(err.to_string()),
            EmailSenderError::InvalidContent => Problem::bad_request(INVALID_CONTENT).with_detail(err.to_string()),

            err => Problem::internal_error()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
        }
    }
}
