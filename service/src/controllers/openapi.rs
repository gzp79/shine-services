use crate::app_config::SERVICE_NAME;
use shine_service::axum::ApiPath;

pub enum ApiKind<'a> {
    Absolute(&'a str),
    Api(&'a str),
    Doc(&'a str),
    Page(&'a str),
    AuthPage(&'a str, &'a str),
}

impl<'a> ApiPath for ApiKind<'a> {
    fn path(&self) -> String {
        match self {
            ApiKind::Absolute(path) => path.to_string(),
            ApiKind::Api(path) => format!("/{}/api{}", SERVICE_NAME, path),
            ApiKind::Doc(path) => format!("/{}/doc{}", SERVICE_NAME, path),
            ApiKind::Page(path) => format!("/{}{}", SERVICE_NAME, path),
            ApiKind::AuthPage(provider, path) => format!("/{}/auth/{}{}", SERVICE_NAME, provider, path),
        }
    }
}
