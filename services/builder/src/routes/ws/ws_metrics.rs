use opentelemetry::{global, metrics::Counter};
use shine_infra::telemetry::label;
use std::sync::LazyLock;

/// Metrics of the websocket.
pub struct WsMetrics {
    messages_received: Counter<u64>,
    socket_timeouts: Counter<u64>,
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
            socket_timeouts: meter
                .u64_counter("ws_socket_timeouts")
                .with_description("Websockets ended because the peer stopped reading (write) or went silent (idle)")
                .build(),
        }
    }

    /// A message of the given kind was received, and handled with the given outcome.
    pub fn message_received(&self, kind: &'static str, outcome: &'static str) {
        self.messages_received
            .add(1, &[label("kind", kind), label("outcome", outcome)]);
    }

    /// A socket was ended by the server for being stuck:
    /// - `write` (the peer does not read)
    /// - `idle` (nothing received).
    pub fn socket_timed_out(&self, kind: &'static str) {
        self.socket_timeouts.add(1, &[label("kind", kind)]);
    }
}
