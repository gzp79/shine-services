use crate::{
    repositories::mailer::{Email, EmailContent, EmailSender, EmailSenderError},
    services::SettingsService,
};
use ring::digest;
use shine_core::consts::Language;
use tera::Tera;

#[derive(Clone)]
pub struct MailerService<'a, E: EmailSender> {
    pub settings: &'a SettingsService,
    pub mailer: &'a E,
    pub tera: &'a Tera,
}

impl<'a, E: EmailSender> MailerService<'a, E> {
    pub fn new(settings: &'a SettingsService, mailer: &'a E, tera: &'a Tera) -> Self {
        Self { settings, mailer, tera }
    }

    pub async fn send_confirmation_email(
        &self,
        to: &str,
        token: &str,
        lang: Option<Language>,
        user_name: &str,
    ) -> Result<(), EmailSenderError> {
        let mut context = tera::Context::new();

        let lang = lang.unwrap_or(Language::En);
        let to_hash = hash_email(to);
        let url = format!(
            "{}token/login?token={}&emailHash={}",
            self.settings.auth_base_url, token, to_hash
        );

        context.insert("user", user_name);
        context.insert("link", url.as_str());
        context.insert("app", self.settings.app_name.as_str());

        let html = self
            .tera
            .render(&format!("mail/{}/confirm.html", lang), &context)
            .expect("Failed to generate confirm html");

        self.mailer
            .send(
                "no-replay",
                to,
                Email {
                    subject: "Confirm your email".to_string(),
                    body: EmailContent::Html(html),
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
