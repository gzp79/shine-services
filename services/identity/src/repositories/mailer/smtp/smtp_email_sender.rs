use crate::repositories::mailer::{Email, EmailContent, EmailSender, EmailSenderError};
use lettre::{
    address::AddressError,
    error::Error as EmailError,
    message::header::ContentType,
    transport::smtp::{self, authentication::Credentials, client::Tls, SUBMISSIONS_PORT},
    Message, SmtpTransport, Transport,
};
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum SmtpEmailSenderBuildError {
    #[error("Invalid SMTP URL")]
    InvalidSmtpUrl,
}

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
    pub fn new(
        email_domain: &str,
        smtp_url: &str,
        use_tls: bool,
        smtp_username: &str,
        smtp_password: &str,
    ) -> Result<Self, SmtpEmailSenderBuildError> {
        let (relay, port) = {
            let mut iter = smtp_url.splitn(2, ':');
            let relay = iter.next().ok_or(SmtpEmailSenderBuildError::InvalidSmtpUrl)?;
            let port = iter.next().map_or(Ok(SUBMISSIONS_PORT), |p| {
                p.parse().map_err(|err| {
                    log::error!("Failed to parse SMTP port: {err}");
                    SmtpEmailSenderBuildError::InvalidSmtpUrl
                })
            })?;
            (relay, port)
        };

        let credentials = Credentials::new(smtp_username.to_owned(), smtp_password.to_owned());
        let mut mailer_builder = SmtpTransport::relay(relay)
            .map_err(|err| {
                log::error!("Failed to create SMTP transport: {err}");
                SmtpEmailSenderBuildError::InvalidSmtpUrl
            })?
            .port(port)
            .credentials(credentials);

        if !use_tls {
            log::warn!("TLS is disabled for SMTP transport");
            mailer_builder = mailer_builder.tls(Tls::None);
        }

        let mailer = mailer_builder.build();

        Ok(Self {
            email_domain: email_domain.to_owned(),
            mailer,
        })
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
            //EmailContent::Text(text) => builder.header(ContentType::TEXT_PLAIN).body(text)?,
        };

        self.mailer.send(&message)?;

        Ok(())
    }
}
