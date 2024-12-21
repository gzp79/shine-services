use chrono::Duration;
use url::Url;

pub struct TokenSettings {
    pub ttl_access_token: Duration,
    pub ttl_single_access: Duration,
    pub ttl_api_key: Duration,
}

pub struct SettingsService {
    pub app_name: String,
    pub app_version: String,
    pub home_url: Url,
    pub error_url: Url,
    pub token: TokenSettings,
    pub external_providers: Vec<String>,
    pub full_problem_response: bool,
    pub page_redirect_time: Option<u32>,
    pub super_user_api_key_hash: Option<String>,
}
