use crate::repositories::ExternalUserInfo;
use reqwest::{header, Client as HttpClient};
use serde::Deserialize;
use url::Url;

pub(in crate::auth) async fn get_github_user_email(
    client: &HttpClient,
    mut external_user_info: ExternalUserInfo,
    app_name: &str,
    token: &str,
) -> Result<ExternalUserInfo, String> {
    if external_user_info.email.is_none() {
        let url = Url::parse("https://api.github.com/user/emails").unwrap();
        let response = client
            .get(url)
            .bearer_auth(token)
            .header(header::USER_AGENT, app_name)
            .send()
            .await
            .map_err(|err| format!("Request error: {err}"))?;

        #[derive(Deserialize, Debug)]
        struct Email {
            email: String,
            primary: bool,
        }

        let email_info = if response.status().is_success() {
            response.json::<Vec<Email>>().await.map_err(|err| format!("{err}"))?
        } else {
            return Err(format!(
                "({}), {}",
                response.status(),
                response.text().await.unwrap_or_default(),
            ));
        };
        log::info!("{:?}", email_info);

        external_user_info.email = email_info
            .into_iter()
            .find(|email| email.primary)
            .map(|email| email.email);
    }

    Ok(external_user_info)
}
