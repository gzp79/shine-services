use crate::repositories::DBConfig;
use config::ConfigError;
use serde::{Deserialize, Serialize};
use shine_service::axum::telemetry::TelemetryConfig;
use shine_service::service::CoreConfig;
use thiserror::Error as ThisError;
use tower_http::cors::AllowOrigin;
use url::Url;

pub const SERVICE_NAME: &str = "identity";

#[derive(Debug, ThisError)]
#[error("Pre-init configuration is not matching to the final configuration")]
pub struct PreInitConfigError;

impl From<PreInitConfigError> for ConfigError {
    fn from(err: PreInitConfigError) -> Self {
        ConfigError::Foreign(Box::new(err))
    }
}

#[derive(Debug, ThisError)]
#[error("CORS configuration is not a valid")]
pub struct CORSConfigError;

impl From<CORSConfigError> for ConfigError {
    fn from(err: CORSConfigError) -> Self {
        ConfigError::Foreign(Box::new(err))
    }
}

/// The application configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsConfig {
    pub cert: String,
    pub key: String,
}

/// The application configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceConfig {
    pub tls: Option<TlsConfig>,
    pub port: u16,
    /// Regular expression for the allowed origins.
    pub allowed_origins: Vec<String>,
    /// Indicates if the full problem response should be returned. In production, it should be `false`.
    pub full_problem_response: bool,
    /// The secret for the used captcha solution.
    pub captcha_secret: String,
}

impl ServiceConfig {
    pub fn cors_allowed_origin(&self) -> Result<AllowOrigin, CORSConfigError> {
        let allowed_origins = self
            .allowed_origins
            .iter()
            .map(|r| regex::bytes::Regex::new(r))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_err| CORSConfigError)?;
        Ok(AllowOrigin::predicate(move |origin, _| {
            let origin = origin.as_bytes();
            allowed_origins.iter().any(|r| r.is_match(origin))
        }))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSessionConfig {
    pub cookie_name_suffix: Option<String>,

    pub session_secret: String,
    pub external_login_cookie_secret: String,
    pub token_cookie_secret: String,

    /// The maximum time to live for a session in seconds
    pub ttl_session: usize,
    /// The maximum time to live for an access (remember me) token in seconds
    pub ttl_access_token: usize,
    /// The maximum time to live for a single access (one-time-use) token in seconds
    pub ttl_single_access: usize,
    /// The maximum time to live for an api-key in seconds
    pub ttl_api_key: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthConfig {
    /// The name of the application
    pub app_name: String,
    /// The default redirection URL for users
    pub home_url: Url,
    /// The default redirection URL for users in case of an err
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
    page_redirect_time: Option<u32>,
    /// Enable to display error details on pages. From a security standpoint, it is not advisable to enable this feature
    /// as it may expose unwanted information. Therefore, it is recommended to disable this feature in production.
    page_error_detail: Option<bool>,
    // /// Initiates OIDC discovery at startup to identify and rectify any potential misconfigurations.
    // openid_startup_discovery: bool,
    // /// A list of external providers utilizing the (interactive) OpenID Connect login flow.
    // pub openid: HashMap<String, OIDCConfig>,
    // /// List of external providers utilizing the (interactive) OAuth2 login flow
    // pub oauth2: HashMap<String, OAuth2Config>,
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

/// The application configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(flatten)]
    pub core: CoreConfig,

    pub service: ServiceConfig,
    pub telemetry: TelemetryConfig,
    pub db: DBConfig,
    pub auth: AuthConfig,
    pub user_name: AutoNameConfig,
}

impl AppConfig {
    pub async fn new(stage: &str) -> Result<AppConfig, ConfigError> {
        let pre_init = CoreConfig::new(stage)?;
        let builder = pre_init.create_config_builder()?;
        let config = builder.build().await?;
        log::debug!("configuration values: {:#?}", config);

        let cfg: AppConfig = config.try_deserialize()?;
        if pre_init != cfg.core {
            return Err(PreInitConfigError.into());
        }

        log::info!("configuration: {:#?}", cfg);
        Ok(cfg)
    }
}
