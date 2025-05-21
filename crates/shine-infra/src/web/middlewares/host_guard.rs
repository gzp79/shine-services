use crate::web::responses::{IntoProblemResponse, Problem, ProblemConfig};
use axum::{
    body::Body,
    extract::{Extension, Request},
    response::{IntoResponse, Response},
    RequestExt, Router,
};
use futures::future::BoxFuture;
use regex::bytes::Regex;
use std::{collections::HashMap, sync::Arc};
use std::{
    convert::Infallible,
    task::{Context, Poll},
};
use tower::{Layer, Service};

/// Mark endpoints with a host-guard domain.
#[derive(Debug, Clone)]
pub struct HostGuardDomain(String);

impl HostGuardDomain {
    pub fn new(domain: String) -> Self {
        Self(domain)
    }
}

/// Helper trait trait to add the HostGuardDomain
pub trait HostGuardRouteExt {
    fn with_host_domain(self, domain: String) -> Self;
}

impl<B> HostGuardRouteExt for Router<B>
where
    B: Clone + Send + Sync + 'static,
{
    fn with_host_domain(self, domain: String) -> Self {
        self.layer(Extension(HostGuardDomain::new(domain.clone())))
    }
}

type HostsForDomains = HashMap<String, Vec<Regex>>;

/// Middleware to guard against requests from authorized hosts.
/// Each domain may have distinct host patterns and if domain extension is provided, the "default" is used
#[derive(Clone)]
pub struct HostGuard {
    allowed_hosts: Arc<HostsForDomains>,
}

impl HostGuard {
    pub fn new(allowed_hosts: HostsForDomains) -> Self {
        Self {
            allowed_hosts: Arc::new(allowed_hosts),
        }
    }
}

impl<S> Layer<S> for HostGuard {
    type Service = HostGuardMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        HostGuardMiddleware { inner, layer: self.clone() }
    }
}

#[derive(Clone)]
#[must_use]
pub struct HostGuardMiddleware<I> {
    inner: I,
    layer: HostGuard,
}

impl<I> Service<Request<Body>> for HostGuardMiddleware<I>
where
    I: Service<Request, Error = Infallible> + Clone + Send + Sync + 'static,
    I::Response: IntoResponse,
    I::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        let mut inner = self.inner.clone();

        let allowed_hosts = self.layer.allowed_hosts.clone();
        Box::pin(async move {
            let problem_config = request
                .extract_parts::<Extension<ProblemConfig>>()
                .await
                .expect("Missing ProblemConfig extension");
            let domain = request
                .extract_parts::<Extension<HostGuardDomain>>()
                .await
                .map(|Extension(domain)| domain.0.clone())
                .unwrap_or("default".to_string());

            let host = request
                .headers()
                .get("host")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("");
            let allowed_hosts = allowed_hosts.get(&domain).expect("No host guard domain found");

            for allowed_host in allowed_hosts {
                if allowed_host.is_match(host.as_bytes()) {
                    return inner.call(request).await.map(|x| x.into_response());
                }
            }
            let response: Response = Problem::forbidden()
                .with_detail(format!("Host not allowed for domain: {}", domain))
                .into_response(&problem_config)
                .into_response();
            Ok(response)
        })
    }
}
