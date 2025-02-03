use super::EmailSenderError;
use std::future::Future;

pub enum EmailContent {
    Html(String),
    Text(String),
}

pub struct Email {
    pub subject: String,
    pub body: EmailContent,
}

pub trait EmailSender {
    /// Send an email from a given name form with the default email server domain to a given email address.
    fn send(
        &self,
        from_name: &str,
        to: &str,
        email: Email,
    ) -> impl Future<Output = Result<(), EmailSenderError>> + Send + Sync;
}
