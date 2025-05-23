use crate::repositories::DBConfig;
use serde::{Deserialize, Serialize};
use shine_infra::web::FeatureConfig;
use std::collections::{HashMap, HashSet};
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSessionConfig {
    pub external_login_cookie_secret: String,
    pub token_cookie_secret: String,
    pub email_token_secret: String,

    /// The maximum time to live for an access (remember me) token in seconds
    pub ttl_access_token: usize,
    /// The maximum time to live for a single access (one-time-use) token in seconds
    pub ttl_single_access: usize,
    /// The maximum time to live for an api-key in seconds
    pub ttl_api_key: usize,
    /// The maximum time to live for an email login in seconds
    pub ttl_email_token: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ExternalUserInfoExtensions {
    GithubEmail,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuth2Config {
    pub authorization_url: String,
    pub token_url: String,
    pub user_info_url: String,
    pub user_info_mapping: HashMap<String, String>,
    pub extensions: HashSet<ExternalUserInfoExtensions>,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
    pub ignore_certificates: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OIDCConfig {
    pub discovery_url: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub scopes: Vec<String>,
    pub ignore_certificates: Option<bool>,
    /// Maximum time to store the discovered OIDC client information, like JWKS.
    pub ttl_client: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthConfig {
    /// The name of the application
    pub app_name: String,
    /// The URL of the website
    pub home_url: Url,
    /// The URL to handle permanent links and similar redirect URLs
    pub link_url: Url,
    /// The default redirection URL for users in case of an error during the authentication process.
    pub error_url: Url,
    /// The URL base for authentication services. This serves as:
    /// - The source for certain cookie protection parameters, including domain and path.
    /// - The base URL for managing the redirection of external login authentication flows.
    pub auth_base_url: Url,

    /// Hash of the master key to manage user roles. In production once a user is created it's highly
    /// recommended to disable this feature.
    pub super_user_api_key_hash: Option<String>,

    /// Authentication related cookie configuration.
    #[serde(flatten)]
    pub auth_session: AuthSessionConfig,

    /// The time interval before redirecting the user from embedded pages. If not provided, no redirection occurs,
    /// and a value of 0 signifies an immediate redirect.
    pub page_redirect_time: Option<u32>,

    /// Initiates OIDC discovery at startup to identify and rectify any potential misconfigurations.
    pub openid_startup_discovery: bool,
    /// A list of external providers utilizing the (interactive) OpenID Connect login flow.
    pub openid: HashMap<String, OIDCConfig>,
    /// List of external providers utilizing the (interactive) OAuth2 login flow
    pub oauth2: HashMap<String, OAuth2Config>,
}

impl AuthConfig {
    pub fn collect_providers(&self) -> Vec<String> {
        let mut providers = Vec::new();
        providers.extend(self.openid.keys().cloned());
        providers.extend(self.oauth2.keys().cloned());
        providers
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "idEncoder")]
pub enum IdEncoderConfig {
    #[serde(rename_all = "camelCase")]
    Optimus { prime: u64, random: u64 },

    #[serde(rename_all = "camelCase")]
    Harsh { salt: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoNameConfig {
    pub base_name: String,
    #[serde(flatten)]
    pub id_encoder: IdEncoderConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum MailerConfig {
    #[serde(rename_all = "camelCase")]
    Smtp {
        email_domain: String,
        smtp_url: String,
        use_tls: Option<bool>,
        smtp_username: String,
        smtp_password: String,
    },
}

/// The application configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub db: DBConfig,
    pub name: AutoNameConfig,
    pub auth: AuthConfig,
    pub mailer: MailerConfig,
}

impl FeatureConfig for AppConfig {
    const NAME: &'static str = "identity";
}

#[cfg(test)]
mod test {
    use axum_extra::extract::cookie;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
    use ring::{
        aead,
        rand::{SecureRandom, SystemRandom},
    };
    use shine_test::test;

    #[test(skip = "This is not a test but a helper to generate cookie secret")]
    fn generate_cookie_secret() {
        let key = cookie::Key::generate();
        println!("{}", B64.encode(key.master()));
    }

    #[test(skip = "This is not a test but a helper to generate an email secret")]
    fn generate_email_token_secret() {
        let rng = SystemRandom::new();
        let mut key = vec![0u8; aead::AES_256_GCM.key_len()];
        rng.fill(&mut key).unwrap();
        println!("{}", B64.encode(key));
    }
}
