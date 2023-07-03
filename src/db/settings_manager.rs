use std::sync::Arc;

use crate::app_config::AppConfig;

struct Inner {
    home_url: String,
    auth_providers: Vec<String>,
}

#[derive(Clone)]
pub struct SettingsManager(Arc<Inner>);

impl SettingsManager {
    pub fn new(config: &AppConfig) -> Self {
        Self(Arc::new(Inner {
            home_url: config.home_url.to_string(),
            auth_providers: config.auth.openid.keys().cloned().collect(),
        }))
    }

    pub fn home_url(&self) -> &str {
        &self.0.home_url
    }

    pub fn auth_providers(&self) -> &[String] {
        &self.0.auth_providers
    }
}
