/* spell-checker: disable */
use crate::repositories::mailer::{Email, EmailContent, EmailSender, EmailSenderError};
use lettre::{
    address::AddressError,
    error::Error as EmailError,
    message::header::ContentType,
    transport::smtp::{self, authentication::Credentials},
    Message, SmtpTransport, Transport,
};
/* spell-checker: enable */

impl From<AddressError> for EmailSenderError {
    fn from(err: AddressError) -> Self {
        log::warn!("Invalid email address, {err}");
        EmailSenderError::InvalidEmailAddress
    }
}

impl From<EmailError> for EmailSenderError {
    fn from(err: EmailError) -> Self {
        log::warn!("Invalid email content, {err}");
        EmailSenderError::InvalidContent
    }
}

impl From<smtp::Error> for EmailSenderError {
    fn from(err: smtp::Error) -> Self {
        log::warn!("Email send failed: {err}");
        EmailSenderError::SendFailed(err.to_string())
    }
}

#[derive(Debug)]
pub struct SmtpEmailSender {
    mailer: SmtpTransport,
    email_domain: String,
}

impl SmtpEmailSender {
    pub fn new(email_domain: &str, relay_server: &str, smtp_username: &str, smtp_password: &str) -> Self {
        let credentials = Credentials::new(smtp_username.to_owned(), smtp_password.to_owned());
        let mailer = SmtpTransport::relay(relay_server)
            .unwrap()
            .credentials(credentials)
            .build();

        Self {
            email_domain: email_domain.to_owned(),
            mailer,
        }
    }
}

impl EmailSender for SmtpEmailSender {
    async fn send(&self, from_name: &str, to: &str, content: Email) -> Result<(), EmailSenderError> {
        log::info!(
            "Sending email from {} to {} with subject {}...",
            to,
            from_name,
            content.subject
        );

        let builder = Message::builder()
            .from(format!("{}@{}", from_name, self.email_domain).parse()?)
            .to(to.parse()?)
            .subject(content.subject);

        let message = match content.body {
            EmailContent::Html(html) => builder.header(ContentType::TEXT_HTML).body(html)?,
            EmailContent::Text(text) => builder.header(ContentType::TEXT_PLAIN).body(text)?,
        };

        self.mailer.send(&message)?;

        Ok(())
    }
}
