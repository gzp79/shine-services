use crate::health::StatusProvider;
use async_trait::async_trait;
use axum::{body::Body, http::Request, response::Response};
use futures::future::BoxFuture;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll},
};
use tower::{Layer, Service};

#[derive(Clone)]
pub struct InFlightService {
    count: Arc<AtomicUsize>,
}

impl InFlightService {
    pub fn new() -> Self {
        Self {
            count: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn get(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }

    pub fn create_layer(&self) -> InFlightRequestLayer {
        InFlightRequestLayer {
            service: self.clone(),
        }
    }
}

impl Default for InFlightService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatusProvider for InFlightService {
    fn name(&self) -> &'static str {
        "http"
    }

    async fn status(&self) -> serde_json::Value {
        serde_json::json!({
            "inFlightRequests": self.get()
        })
    }
}

struct InFlightGuard {
    count: Arc<AtomicUsize>,
}

impl InFlightGuard {
    fn new(count: Arc<AtomicUsize>) -> Self {
        count.fetch_add(1, Ordering::Relaxed);
        Self { count }
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::Relaxed);
    }
}

#[derive(Clone)]
pub struct InFlightRequestLayer {
    service: InFlightService,
}

impl<S> Layer<S> for InFlightRequestLayer {
    type Service = InFlightRequestMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        InFlightRequestMiddleware {
            inner,
            service: self.service.clone(),
        }
    }
}

#[derive(Clone)]
#[must_use]
pub struct InFlightRequestMiddleware<S> {
    inner: S,
    service: InFlightService,
}

impl<S> Service<Request<Body>> for InFlightRequestMiddleware<S>
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
        let guard = InFlightGuard::new(Arc::clone(&self.service.count));
        let future = self.inner.call(request);
        Box::pin(async move {
            let _guard = guard;
            future.await
        })
    }
}
