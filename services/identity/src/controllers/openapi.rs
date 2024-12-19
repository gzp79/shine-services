use shine_service::axum::ApiPath;

pub enum ApiKind<'a> {
    Absolute(&'a str),
    Api(&'a str),
    Doc(&'a str),
    Page(&'a str),
}

impl ApiPath for ApiKind<'_> {
    fn path(&self) -> String {
        match self {
            ApiKind::Absolute(path) => path.to_string(),
            ApiKind::Api(path) => format!("/api{}", path),
            ApiKind::Doc(path) => format!("/doc{}", path),
            ApiKind::Page(path) => path.to_string(),
        }
    }
}
