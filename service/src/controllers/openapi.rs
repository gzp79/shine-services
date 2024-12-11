use shine_service::axum::ApiPath;

pub enum ApiKind<'a> {
    Absolute(&'a str),
    Api(&'a str),
    Doc(&'a str),
    AuthPage(&'a str, &'a str),
}

impl<'a> ApiPath for ApiKind<'a> {
    fn path(&self) -> String {
        match self {
            ApiKind::Absolute(path) => path.to_string(),
            ApiKind::Api(path) => format!("/api{}", path),
            ApiKind::Doc(path) => format!("/doc{}", path),
            ApiKind::AuthPage(provider, path) => format!("/auth/{}{}", provider, path),
        }
    }
}
