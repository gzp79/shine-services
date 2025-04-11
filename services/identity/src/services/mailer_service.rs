use crate::{
    repositories::mailer::{Email, EmailContent, EmailSender, EmailSenderError},
    services::SettingsService,
};
use ring::digest;
use shine_infra::language::Language;
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

    pub async fn send_email_confirmation(
        &self,
        to: &str,
        token: &str,
        lang: Option<Language>,
        user_name: &str,
    ) -> Result<(), EmailSenderError> {
        let mut context = tera::Context::new();

        let lang = lang.unwrap_or(Language::En);
        let mut redirect_url = self
            .settings
            .link_url
            .join("email-verify")
            .map_err(|err| EmailSenderError::SendFailed(err.to_string()))?;
        redirect_url.set_query(Some(&format!("token={}&hint=email-confirm", token)));

        context.insert("user", user_name);
        context.insert("link", redirect_url.as_str());
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

    pub async fn send_email_change(
        &self,
        to: &str,
        token: &str,
        lang: Option<Language>,
        user_name: &str,
    ) -> Result<(), EmailSenderError> {
        let mut context = tera::Context::new();

        let lang = lang.unwrap_or(Language::En);
        let mut redirect_url = self
            .settings
            .link_url
            .join("email-verify")
            .map_err(|err| EmailSenderError::SendFailed(err.to_string()))?;
        redirect_url.set_query(Some(&format!("token={}&hint=email-change", token)));

        context.insert("user", user_name);
        context.insert("link", redirect_url.as_str());
        context.insert("app", self.settings.app_name.as_str());

        let html = self
            .tera
            .render(&format!("mail/{}/change.html", lang), &context)
            .expect("Failed to generate change email html");

        self.mailer
            .send(
                "no-replay",
                to,
                Email {
                    subject: "Switch your email address".to_string(),
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
