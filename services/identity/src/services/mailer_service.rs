use crate::repositories::mailer::{Email, EmailContent, EmailSender, EmailSenderError};
use ring::digest;
use tera::Tera;

#[derive(Clone)]
pub struct MailerService<'a, E: EmailSender> {
    pub mailer: &'a E,
    pub tera: &'a Tera,
}

impl<'a, E: EmailSender> MailerService<'a, E> {
    pub fn new(mailer: &'a E, tera: &'a Tera) -> Self {
        Self { mailer, tera }
    }

    pub async fn send_confirmation_email(&self, to: &str, token: &str) -> Result<(), EmailSenderError> {
        self.mailer
            .send(
                "no-replay",
                to,
                Email {
                    subject: "Confirm your email".to_string(),
                    body: EmailContent::Text(format!(
                        "Click the link below to confirm your email address:\n\nhttps://example.com/auth/email/confirm?token={token}"
                    )),
                },
            )
            .await?;
        Ok(())
    }
}

/// Generate a (crypto) hashed version of an email.
pub fn hash_email(email_address: &str) -> String {
    let hash = digest::digest(&digest::SHA256, email_address.as_bytes());
    let hash = hex::encode(hash);
    log::debug!("Hashing email: {email_address:?} -> [{hash}]");
    hash
}
