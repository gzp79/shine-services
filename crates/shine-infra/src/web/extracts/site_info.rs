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
    if s.len() == 2 && s.is_ascii() && s.chars().all(|c| c.is_ascii_alphanumeric()) {
        Some(s.to_ascii_uppercase())
    } else {
        None
    }
}

fn sanitize_site_text(raw: &str) -> Option<String> {
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

        let get_sanitized_header = |name: &str, sanitizer: fn(&str) -> Option<String>| {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| sanitizer(s))
        };

        Ok(SiteInfo {
            agent,
            country: get_sanitized_header("cf-ipcountry", sanitize_country),
            region: get_sanitized_header("cf-region", sanitize_site_text),
            city: get_sanitized_header("cf-ipcity", sanitize_site_text),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_country;

    #[test]
    fn sanitize_country_normalizes_valid_two_letter_codes() {
        assert_eq!(sanitize_country("fi"), Some("FI".to_string()));
        assert_eq!(sanitize_country(" T1 "), Some("T1".to_string()));
        assert_eq!(sanitize_country("XX"), Some("XX".to_string()));
        assert_eq!(sanitize_country("1a"), Some("1A".to_string()));
    }

    #[test]
    fn sanitize_country_rejects_ugly_inputs() {
        assert_eq!(sanitize_country("country"), None);
        assert_eq!(sanitize_country("A-"), None);
        assert_eq!(sanitize_country(""), None);
    }
}
