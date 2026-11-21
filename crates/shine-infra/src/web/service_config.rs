use regex::bytes::Regex;
use serde::{Deserialize, Serialize};

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
    /// Regex patterns for the allowed WebSocket origins.
    #[serde(default, with = "crate::serde::anchored")]
    pub allowed_origins: Vec<Regex>,
    /// Indicates if the full problem response should be returned. In production, it should be `false`.
    pub full_problem_response: bool,
    /// The secret for the used captcha solution.
    pub captcha_secret: String,
    /// The user session secret for the used cookie validation
    pub session_secret: String,
    /// The user session time to live - auto logout after this time of "inactivity"
    pub session_ttl: u64,
    /// The get up-to-date session information of the current user
    pub session_redis_cns: String,
    /// Expose x-powered-by response header with service name and version. Default: false.
    #[serde(default)]
    pub expose_powered_by: bool,
    /// Expose Swagger UI and OpenAPI documentation endpoints. Default: false.
    #[serde(default)]
    pub expose_api_docs: bool,
}

impl ServiceConfig {
    pub fn is_allowed_origin(&self, origin: &str) -> bool {
        self.allowed_origins.iter().any(|r| r.is_match(origin.as_bytes()))
    }
}
