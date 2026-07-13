use axum::{
    body::Body,
    http::{HeaderValue, Request},
    response::Response,
};
use futures::future::BoxFuture;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(Clone)]
pub struct SecurityHeaders {
    headers: Arc<Vec<(&'static str, HeaderValue)>>,
}

impl SecurityHeaders {
    pub fn new() -> Self {
        Self {
            headers: Arc::new(vec![
                (
                    "strict-transport-security",
                    HeaderValue::from_static("max-age=31536000; includeSubDomains"),
                ),
                ("x-content-type-options", HeaderValue::from_static("nosniff")),
                ("x-frame-options", HeaderValue::from_static("DENY")),
                ("referrer-policy", HeaderValue::from_static("no-referrer")),
                (
                    "content-security-policy",
                    HeaderValue::from_static(
                        "default-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'",
                    ),
                ),
                ("cache-control", HeaderValue::from_static("no-store")),
            ]),
        }
    }
}

impl Default for SecurityHeaders {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for SecurityHeaders {
    type Service = SecurityHeadersMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SecurityHeadersMiddleware {
            inner,
            headers: Arc::clone(&self.headers),
        }
    }
}

#[derive(Clone)]
#[must_use]
pub struct SecurityHeadersMiddleware<S> {
    inner: S,
    headers: Arc<Vec<(&'static str, HeaderValue)>>,
}

impl<S> Service<Request<Body>> for SecurityHeadersMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let future = self.inner.call(request);
        let headers_to_insert = Arc::clone(&self.headers);
        Box::pin(async move {
            let mut response: Response = future.await?;
            let response_headers = response.headers_mut();
            for (name, value) in headers_to_insert.iter() {
                response_headers.insert(*name, value.clone());
            }
            Ok(response)
        })
    }
}
