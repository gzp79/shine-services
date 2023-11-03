use axum::{
    async_trait, extract::FromRequestParts, headers::UserAgent, http::request::Parts, RequestPartsExt, TypedHeader,
};
use std::convert::Infallible;

#[derive(Debug, PartialEq, Eq)]
/// General client info used for human readable site identification
pub struct SiteInfo {
    pub agent: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

#[async_trait]
impl<S> FromRequestParts<S> for SiteInfo
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let agent = parts
            .extract::<TypedHeader<UserAgent>>()
            .await
            .map(|u| u.to_string())
            .unwrap_or_default();

        Ok(SiteInfo {
            agent,
            country: None,
            region: None,
            city: None,
        })
    }
}
