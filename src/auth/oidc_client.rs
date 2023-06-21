use crate::{
    auth::{AuthBuildError, OIDCConfig},
    db::SettingsManager,
};
use axum::response::Html;
use oauth2::{reqwest::async_http_client, AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl};
use openidconnect::{
    core::{CoreClient, CoreJsonWebKeySet, CoreProviderMetadata},
    IssuerUrl, UserInfoUrl,
};
use tera::{Error as TeraError, Tera};

pub(in crate::auth) struct OIDCClient {
    pub provider: String,
    pub client: CoreClient,
}

impl OIDCClient {
    pub async fn new(provider: &str, config: &OIDCConfig) -> Result<Self, AuthBuildError> {
        let client_id = ClientId::new(config.client_id.clone());
        let client_secret = ClientSecret::new(config.client_secret.clone());
        let redirect_url = RedirectUrl::new(config.redirect_url.to_string())
            .map_err(|err| AuthBuildError::RedirectUrl(format!("{err}")))?;

        log::info!("Redirect url for provider {}: {:?}", provider, redirect_url);

        // Use OpenID Connect Discovery to fetch the provider metadata.

        let client = if let Some(discovery_url) = &config.discovery_url {
            let discovery_url =
                IssuerUrl::new(discovery_url.clone()).map_err(|err| AuthBuildError::InvalidIssuer(format!("{err}")))?;
            let provider_metadata = CoreProviderMetadata::discover_async(discovery_url, async_http_client)
                .await
                .map_err(|err| AuthBuildError::Discovery(format!("{err}")))?;
            CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
                .set_redirect_uri(redirect_url)
        } else if let Some(endpoints) = &config.endpoints {
            let issuer_url = IssuerUrl::new("http://github.com".into()).unwrap(); //todo
            let auth_url = AuthUrl::new(endpoints.authorization_url.clone())
                .map_err(|err| AuthBuildError::InvalidAuth(format!("{err}")))?;
            let token_url = TokenUrl::new(endpoints.token_url.clone())
                .map_err(|err| AuthBuildError::InvalidToken(format!("{err}")))?;
            let user_info_url = UserInfoUrl::new(endpoints.user_info_url.clone())
                .map_err(|err| AuthBuildError::InvalidUserInfo(format!("{err}")))?;
            CoreClient::new(
                client_id,
                Some(client_secret),
                issuer_url,
                auth_url,
                Some(token_url),
                Some(user_info_url),
                CoreJsonWebKeySet::default(),
            )
            .set_redirect_uri(redirect_url)
        } else {
            return Err(AuthBuildError::InvalidEndpoints);
        };

        Ok(Self {
            provider: provider.to_string(),
            client,
        })
    }
}

pub(in crate::auth) fn create_redirect_page(
    tera: &Tera,
    settings_manager: &SettingsManager,
    title: &str,
    target: &str,
    target_url: Option<&str>,
) -> Result<Html<String>, TeraError> {
    let mut context = tera::Context::new();
    context.insert("title", title);
    context.insert("target", target);
    context.insert("redirect_url", target_url.unwrap_or(settings_manager.home_url()));
    let html = Html(tera.render("redirect.html", &context)?);
    Ok(html)
}
