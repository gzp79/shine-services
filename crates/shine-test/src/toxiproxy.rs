//! Client of the [toxiproxy](https://github.com/Shopify/toxiproxy) HTTP API, to inject network faults
//! between a test and its services (see `services/docker-compose.yml`).
//!
//! Faults are global to the proxy, so tests using it must not run in parallel with tests that use the
//! same proxy (`#[test(serial = ...)]`), and should hold a [`Toxiproxy::reset_guard`] so no fault
//! outlives the test, even a failing one.
//! The calls panic on failure: a fault that could not be set up must fail the test.

use serde_json::{json, Value};
use std::env;

/// Toxiproxy API endpoint, e.g. `http://localhost:8474`.
pub const TOXIPROXY_URL_ENV: &str = "SHINE_TEST_TOXIPROXY_URL";

pub struct Toxiproxy {
    client: reqwest::Client,
    url: String,
}

impl Toxiproxy {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.into().trim_end_matches('/').to_string(),
        }
    }

    /// The toxiproxy of [`TOXIPROXY_URL_ENV`], `None` (skip the test) when it is not set.
    pub fn from_env() -> Option<Self> {
        match env::var(TOXIPROXY_URL_ENV) {
            Ok(url) => Some(Self::new(url)),
            Err(_) => {
                tracing::warn!("Missing {TOXIPROXY_URL_ENV}, skipping test");
                None
            }
        }
    }

    /// Enables every proxy and removes every toxic.
    pub async fn reset(&self) {
        self.call(self.client.post(format!("{}/reset", self.url))).await;
    }

    /// Resets now, and again when the guard is dropped.
    pub async fn reset_guard(&self) -> ResetGuard {
        self.reset().await;
        ResetGuard { url: self.url.clone() }
    }

    /// The host the proxies listen on, as seen by the test: the host of the API endpoint.
    pub fn host(&self) -> String {
        reqwest::Url::parse(&self.url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_string))
            .unwrap_or_else(|| panic!("invalid toxiproxy url: {}", self.url))
    }

    pub async fn proxy(&self, name: &str) -> Proxy<'_> {
        let info = self.call(self.client.get(format!("{}/proxies/{name}", self.url))).await;
        Proxy {
            toxiproxy: self,
            name: name.to_string(),
            listen: info["listen"].as_str().unwrap_or_default().to_string(),
        }
    }

    async fn call(&self, request: reqwest::RequestBuilder) -> Value {
        let response = request.send().await.expect("toxiproxy is not reachable");
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        assert!(status.is_success(), "toxiproxy call failed with {status}: {body}");
        serde_json::from_str(&body).unwrap_or(Value::Null)
    }
}

/// Resets the toxiproxy when dropped.
pub struct ResetGuard {
    url: String,
}

impl Drop for ResetGuard {
    fn drop(&mut self) {
        // Drop cannot await, and may run on a runtime thread: reset from a thread with its own runtime.
        let url = self.url.clone();
        let reset = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
            runtime.block_on(async {
                reqwest::Client::new()
                    .post(format!("{url}/reset"))
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(std::io::Error::other)
            })
        });
        if !matches!(reset.join(), Ok(Ok(()))) {
            tracing::error!("Failed to reset toxiproxy at {}", self.url);
        }
    }
}

pub struct Proxy<'a> {
    toxiproxy: &'a Toxiproxy,
    name: String,
    listen: String,
}

impl Proxy<'_> {
    /// The listen address as reported by toxiproxy, e.g. `[::]:6379`.
    pub fn listen(&self) -> &str {
        &self.listen
    }

    /// The port clients connect to.
    pub fn listen_port(&self) -> u16 {
        self.listen
            .rsplit(':')
            .next()
            .and_then(|port| port.parse().ok())
            .unwrap_or_else(|| panic!("unexpected listen address of proxy {}: {}", self.name, self.listen))
    }

    /// A disabled proxy refuses new connections and closes the open ones.
    pub async fn set_enabled(&self, enabled: bool) {
        let url = format!("{}/proxies/{}", self.toxiproxy.url, self.name);
        self.toxiproxy
            .call(self.toxiproxy.client.post(url).json(&json!({ "enabled": enabled })))
            .await;
    }

    pub async fn add_toxic(&self, toxic: &Toxic) {
        let url = format!("{}/proxies/{}/toxics", self.toxiproxy.url, self.name);
        let body = json!({
            "name": toxic.name,
            "type": toxic.kind,
            "stream": toxic.stream.as_str(),
            "toxicity": toxic.toxicity,
            "attributes": toxic.attributes,
        });
        self.toxiproxy.call(self.toxiproxy.client.post(url).json(&body)).await;
    }

    pub async fn remove_toxic(&self, name: &str) {
        let url = format!("{}/proxies/{}/toxics/{name}", self.toxiproxy.url, self.name);
        self.toxiproxy.call(self.toxiproxy.client.delete(url)).await;
    }
}

/// Direction of the traffic a toxic applies to.
#[derive(Clone, Copy, Debug)]
pub enum Stream {
    /// Client to server.
    Upstream,
    /// Server to client.
    Downstream,
}

impl Stream {
    fn as_str(self) -> &'static str {
        match self {
            Stream::Upstream => "upstream",
            Stream::Downstream => "downstream",
        }
    }
}

/// A fault applied to every connection of a proxy, the open ones included.
#[derive(Clone, Debug)]
pub struct Toxic {
    pub name: String,
    pub kind: &'static str,
    pub stream: Stream,
    /// Probability of a connection being affected.
    pub toxicity: f32,
    pub attributes: Value,
}

impl Toxic {
    fn new(name: &str, kind: &'static str, stream: Stream, attributes: Value) -> Self {
        Self {
            name: name.to_string(),
            kind,
            stream,
            toxicity: 1.0,
            attributes,
        }
    }

    /// Closes each connection once `bytes` passed since the toxic was added. Upstream, a command cut after
    /// a given byte is a request the server may have executed while the client sees a dropped connection.
    pub fn limit_data(name: &str, stream: Stream, bytes: u64) -> Self {
        Self::new(name, "limit_data", stream, json!({ "bytes": bytes }))
    }

    /// Delays the data by `latency_ms`.
    pub fn latency(name: &str, stream: Stream, latency_ms: u64) -> Self {
        Self::new(name, "latency", stream, json!({ "latency": latency_ms, "jitter": 0 }))
    }

    /// Stops the data and closes the connection after `timeout_ms` (never with 0, until removed).
    pub fn timeout(name: &str, stream: Stream, timeout_ms: u64) -> Self {
        Self::new(name, "timeout", stream, json!({ "timeout": timeout_ms }))
    }

    /// Resets the connection (TCP RST) after `timeout_ms`.
    pub fn reset_peer(name: &str, stream: Stream, timeout_ms: u64) -> Self {
        Self::new(name, "reset_peer", stream, json!({ "timeout": timeout_ms }))
    }
}
