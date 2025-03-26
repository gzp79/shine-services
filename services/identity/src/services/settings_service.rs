use chrono::Duration;
use ring::aead;
use url::Url;

pub struct TokenSettings {
    pub ttl_access_token: Duration,
    pub ttl_single_access: Duration,
    pub ttl_api_key: Duration,
    pub ttl_email_token: Duration,
    pub email_key: aead::LessSafeKey,
}

pub struct SettingsService {
    pub app_name: String,
    pub home_url: Url,
    pub link_url: Url,
    pub error_url: Url,
    pub token: TokenSettings,
    pub external_providers: Vec<String>,
    pub page_redirect_time: Option<u32>,
    pub super_user_api_key_hash: Option<String>,
}
