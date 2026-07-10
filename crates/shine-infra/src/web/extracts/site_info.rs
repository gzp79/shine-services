use axum::{
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap},
    RequestPartsExt,
};
use axum_extra::{headers::UserAgent, TypedHeader};
use std::convert::Infallible;

const MAX_GEO_LEN: usize = 64;

fn sanitize_country(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.len() == 2 && s.chars().all(|c| c.is_ascii_alphabetic()) {
        Some(s.to_ascii_uppercase())
    } else {
        None
    }
}

fn sanitize_geo(raw: &str) -> Option<String> {
    let s: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | ',' | '.'))
        .take(MAX_GEO_LEN)
        .collect();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// General client info used for human readable site identification
pub struct SiteInfo {
    pub agent: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

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

        let headers = parts.extract::<HeaderMap>().await.unwrap_or_default();

        Ok(SiteInfo {
            agent,
            country: headers
                .get("cf-ipcountry")
                .and_then(|c| c.to_str().ok())
                .and_then(sanitize_country),
            region: headers
                .get("cf-region")
                .and_then(|c| c.to_str().ok())
                .and_then(sanitize_geo),
            city: headers
                .get("cf-ipcity")
                .and_then(|c| c.to_str().ok())
                .and_then(sanitize_geo),
        })
    }
}
