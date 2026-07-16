use regex::bytes::Regex;
use std::time::Duration;

pub struct WsSettings {
    pub allowed_origins: Vec<Regex>,
    pub allowed_hosts: Vec<Regex>,
    pub auth_check_interval: Duration,
}

impl WsSettings {
    pub fn is_allowed_origin(&self, origin: &str) -> bool {
        self.allowed_origins.iter().any(|r| r.is_match(origin.as_bytes()))
    }

    pub fn is_allowed_host(&self, host: &str) -> bool {
        self.allowed_hosts.iter().any(|r| r.is_match(host.as_bytes()))
    }
}

pub struct BuilderSettings {
    pub ws: WsSettings,
}
