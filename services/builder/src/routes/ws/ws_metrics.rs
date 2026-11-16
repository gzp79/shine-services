use opentelemetry::{global, metrics::Counter};
use std::sync::LazyLock;

/// Metrics of the websocket endpoint: what the clients send. Labels have a small fixed set of values.
pub struct WsMetrics {
    /// Messages received from the clients, by kind and outcome.
    pub messages_received: Counter<u64>,
}

static METRICS: LazyLock<WsMetrics> = LazyLock::new(WsMetrics::new);

/// The metrics of the websocket endpoint. Without a configured meter provider the instruments are no-ops.
pub fn ws_metrics() -> &'static WsMetrics {
    &METRICS
}

impl WsMetrics {
    fn new() -> Self {
        let meter = global::meter("shine-builder-ws");
        Self {
            messages_received: meter
                .u64_counter("ws_messages_received")
                .with_description("Messages received from the websocket clients")
                .build(),
        }
    }
}
