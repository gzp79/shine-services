use crate::{
    repositories::mailer::{Email, EmailContent, EmailSender, EmailSenderError},
    services::SettingsService,
};
use ring::digest;
use shine_infra::language::Language;
use tera::Tera;
use url::Url;

#[derive(Clone)]
pub struct MailerService<'a, E: EmailSender> {
    pub settings: &'a SettingsService,
    pub mailer: &'a E,
    pub tera: &'a Tera,
}

impl<'a, E: EmailSender> MailerService<'a, E> {
    pub fn new(settings: &'a SettingsService, mailer: &'a E, tera: &'a Tera) -> Self {
        Self {
            settings,
            mailer,
            tera,
        }
    }

    fn find_subject(&self, html: &str) -> Option<String> {
        //extract the title text from the html
        let start = html.find("<title>")? + "<title>".len();
        let end = html.find("</title>")?;
        let subject = &html[start..end];
        let subject = subject.trim();
        if subject.is_empty() {
            None
        } else {
            Some(subject.to_string())
        }
    }

    async fn send_email(
        &self,
        to: &str,
        link: Url,
        user_name: &str,
        lang: Option<Language>,
        template: &str,
    ) -> Result<(), EmailSenderError> {
        let mut context = tera::Context::new();

        context.insert("user", user_name);
        context.insert("link", link.as_str());
        context.insert("app", self.settings.app_name.as_str());

        let lang = lang.unwrap_or(Language::En);
        let html = self
            .tera
            .render(&format!("mail/{}/{}", lang, template), &context)
            .expect("Failed to generate email html");

        self.mailer
            .send(
                "no-replay",
                to,
                Email {
                    subject: self.find_subject(&html).expect("Failed to extract subject"),
                    body: EmailContent::Html(html),
                },
            )
            .await?;
        Ok(())
    }

    pub async fn send_email_confirmation(
        &self,
        to: &str,
        token: &str,
        user_name: &str,
        lang: Option<Language>,
    ) -> Result<(), EmailSenderError> {
        let mut redirect_url = self
            .settings
            .link_url
            .join("email-verify")
            .map_err(|err| EmailSenderError::SendFailed(err.to_string()))?;
        redirect_url.set_query(Some(&format!("token={}&hint=email-confirm", token)));

        self.send_email(to, redirect_url, user_name, lang, "confirm.html")
            .await
    }

    pub async fn send_email_change(
        &self,
        to: &str,
        token: &str,
        lang: Option<Language>,
        user_name: &str,
    ) -> Result<(), EmailSenderError> {
        let mut redirect_url = self
            .settings
            .link_url
            .join("email-verify")
            .map_err(|err| EmailSenderError::SendFailed(err.to_string()))?;
        redirect_url.set_query(Some(&format!("token={}&hint=email-change", token)));

        self.send_email(to, redirect_url, user_name, lang, "change.html")
            .await
    }

    pub async fn send_email_login(
        &self,
        to: &str,
        redirect_url: Url,
        user_name: &str,
        lang: Option<Language>,
    ) -> Result<(), EmailSenderError> {
        self.send_email(to, redirect_url, user_name, lang, "login.html")
            .await
    }

    pub async fn send_email_register(
        &self,
        to: &str,
        redirect_url: Url,
        user_name: &str,
        lang: Option<Language>,
    ) -> Result<(), EmailSenderError> {
        self.send_email(to, redirect_url, user_name, lang, "register.html")
            .await
    }
}

/// Generate a (crypto) hashed version of an email.
pub fn hash_email(email_address: &str) -> String {
    let hash = digest::digest(&digest::SHA256, email_address.as_bytes());
    let hash = hex::encode(hash);
    log::debug!("Hashing email: {email_address:?} -> [{hash}]");
    hash
}
