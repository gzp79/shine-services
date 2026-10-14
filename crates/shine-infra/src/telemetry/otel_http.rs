use axum::{
    extract::MatchedPath,
    http::{header, HeaderMap, Method, Request, Response, Uri, Version},
};
use opentelemetry::{propagation::Extractor, Context};
use std::{borrow::Cow, error::Error as StdError};
use tracing::{field::Empty, info_span, Span};

/// Target of spans exported to the trace backend only, they are hidden from the console log.
pub const TRACING_TARGET: &str = "otel::tracing";
/// Target of spans shown in the console log only, they are not exported to the trace backend.
pub const CONSOLE_TARGET: &str = "otel::console";

/// Request span for the console log, a compact counterpart of the span of [`make_span_from_request`].
pub fn make_console_span<B>(req: &Request<B>) -> Span {
    info_span!(target: CONSOLE_TARGET, "request", method = %req.method(), uri = %req.uri())
}

#[inline]
#[must_use]
pub fn http_method(method: &Method) -> Cow<'static, str> {
    match method {
        &Method::CONNECT => "CONNECT".into(),
        &Method::DELETE => "DELETE".into(),
        &Method::GET => "GET".into(),
        &Method::HEAD => "HEAD".into(),
        &Method::OPTIONS => "OPTIONS".into(),
        &Method::PATCH => "PATCH".into(),
        &Method::POST => "POST".into(),
        &Method::PUT => "PUT".into(),
        &Method::TRACE => "TRACE".into(),
        other => other.to_string().into(),
    }
}

#[inline]
#[must_use]
pub fn http_flavor(version: Version) -> Cow<'static, str> {
    match version {
        Version::HTTP_09 => "0.9".into(),
        Version::HTTP_10 => "1.0".into(),
        Version::HTTP_11 => "1.1".into(),
        Version::HTTP_2 => "2.0".into(),
        Version::HTTP_3 => "3.0".into(),
        other => format!("{other:?}").into(),
    }
}

#[inline]
pub fn url_scheme(uri: &Uri) -> &str {
    uri.scheme_str().unwrap_or_default()
}

#[inline]
pub fn user_agent<B>(req: &Request<B>) -> &str {
    req.headers()
        .get(header::USER_AGENT)
        .map_or("", |h| h.to_str().unwrap_or(""))
}

#[inline]
pub fn http_host<B>(req: &Request<B>) -> &str {
    req.headers()
        .get(header::HOST)
        .map_or(req.uri().host(), |h| h.to_str().ok())
        .unwrap_or("")
}

#[must_use]
pub fn extract_context(headers: &HeaderMap) -> Context {
    pub struct HeaderExtractor<'a>(pub &'a HeaderMap);

    impl Extractor for HeaderExtractor<'_> {
        /// Get a value for a key from the HeaderMap.  If the value is not valid ASCII, returns None.
        fn get(&self, key: &str) -> Option<&str> {
            self.0.get(key).and_then(|value| value.to_str().ok())
        }

        /// Collect all the keys from the HeaderMap.
        fn keys(&self) -> Vec<&str> {
            self.0.keys().map(|value| value.as_str()).collect::<Vec<_>>()
        }
    }

    let extractor = HeaderExtractor(headers);
    opentelemetry::global::get_text_map_propagator(|propagator| propagator.extract(&extractor))
}

pub fn make_span_from_request<B>(req: &Request<B>) -> Span {
    let http_method = http_method(req.method());
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "", |mp| mp.as_str());

    let name = format!("{http_method} {}", if route.is_empty() { "<unmatched>" } else { route });

    info_span!(
        target: TRACING_TARGET,
        "HTTP request",
        http.request.method = %http_method,
        http.route = %route,
        http.client.address = Empty,
        http.response.status_code = Empty, // set on response
        network.protocol.version = %http_flavor(req.version()),
        server.address = http_host(req),
        user_agent.original = user_agent(req),
        url.path = req.uri().path(),
        url.scheme = url_scheme(req.uri()),
        otel.name = %name,
        otel.kind = ?opentelemetry::trace::SpanKind::Server,
        otel.status_code = Empty, // set on response
        enduser.id = Empty, // set when the user is authenticated, see record_user_id
        trace_id = Empty, // set on response
        //request_id = Empty, // set
        exception.message = Empty, // set on response
        "span.type" = "web", // non-official open-telemetry key, only supported by Datadog
    )
}

pub fn update_span_from_response<B>(span: &Span, response: &Response<B>) {
    let status = response.status();
    span.record("http.response.status_code", status.as_u16());

    if status.is_server_error() {
        span.record("otel.status_code", "ERROR");
    }
}

/// Header carrying the trace id of the request, to find the trace of a response.
pub const TRACE_ID_HEADER: &str = "x-trace-id";

/// Add the trace id of the span to the response. Nothing is added if the span is not sampled/exported.
pub fn add_trace_id_header<B>(span: &Span, response: &mut Response<B>) {
    use opentelemetry::trace::TraceContextExt as _;
    use tracing_opentelemetry::OpenTelemetrySpanExt as _;

    let span_context = span.context().span().span_context().clone();
    if span_context.is_valid() && span_context.is_sampled() {
        let trace_id = span_context.trace_id().to_string();
        if let Ok(value) = header::HeaderValue::from_str(&trace_id) {
            span.record("trace_id", trace_id);
            response
                .headers_mut()
                .insert(header::HeaderName::from_static(TRACE_ID_HEADER), value);
        }
    }
}

pub fn update_span_from_error<E>(span: &Span, error: &E)
where
    E: StdError,
{
    span.record("otel.status_code", "ERROR");
    span.record("exception.message", error.to_string());
    error.source().map(|s| span.record("exception.message", s.to_string()));
}

pub fn update_span_from_response_or_error<B, E>(span: &Span, response: &Result<Response<B>, E>)
where
    E: StdError,
{
    match response {
        Ok(response) => {
            update_span_from_response(span, response);
        }
        Err(err) => {
            update_span_from_error(span, err);
        }
    }
}

/// Span context of the current span, to correlate work done later (see [`link_span`]) without keeping the span alive.
pub fn current_span_context() -> opentelemetry::trace::SpanContext {
    use opentelemetry::trace::TraceContextExt as _;
    use tracing_opentelemetry::OpenTelemetrySpanExt as _;

    Span::current().context().span().span_context().clone()
}

/// Link the span to another span (of any trace) it was caused by.
pub fn link_span(span: &Span, origin: opentelemetry::trace::SpanContext) {
    use tracing_opentelemetry::OpenTelemetrySpanExt as _;

    if origin.is_valid() {
        span.add_link(origin);
    }
}

/// Tag the current request span with the authenticated user.
/// Must be called while the request span is current (for example in extractors and handlers).
pub fn record_user_id(user_id: uuid::Uuid) {
    Span::current().record("enduser.id", tracing::field::display(user_id));
}
