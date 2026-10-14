use opentelemetry::{
    global,
    metrics::{Counter, Histogram, UpDownCounter},
};
use std::sync::LazyLock;

/// Metrics of the hub. Unlike the request metrics, the hub works asynchronously: commands are queued and processed
/// by a single loop, messages are pushed to the clients and background consumers run on their own schedule.
/// Labels have a small fixed set of values, ids of users and connections are only available in the traces.
pub struct HubMetrics {
    /// Live connections.
    pub connections: UpDownCounter<i64>,
    /// Connections removed by the hub, by reason.
    pub connections_dropped: Counter<u64>,
    /// Processed control commands, by command.
    pub commands: Counter<u64>,
    /// Processing time of a command in the command loop, by command.
    pub command_duration: Histogram<f64>,
    /// Time a command waits in the queue before the command loop picks it up, by command.
    pub command_queue_delay: Histogram<f64>,
    /// Messages pushed to the connections, by kind and outcome.
    pub messages_sent: Counter<u64>,
    /// Comments delivered to the connections by the chat dispatcher.
    pub chat_delivered: Counter<u64>,
    /// Duration of the ticks of the background consumers, by consumer.
    pub tick_duration: Histogram<f64>,
    /// Messages received from the clients, by kind and outcome.
    pub ws_messages_received: Counter<u64>,
}

static METRICS: LazyLock<HubMetrics> = LazyLock::new(HubMetrics::new);

/// The metrics of the hub. Without a configured meter provider the instruments are no-ops.
pub fn hub_metrics() -> &'static HubMetrics {
    &METRICS
}

impl HubMetrics {
    fn new() -> Self {
        let meter = global::meter("shine-builder-hub");
        Self {
            connections: meter
                .i64_up_down_counter("hub_connections")
                .with_description("Live hub connections")
                .build(),
            connections_dropped: meter
                .u64_counter("hub_connections_dropped")
                .with_description("Connections removed by the hub")
                .build(),
            commands: meter
                .u64_counter("hub_commands")
                .with_description("Processed hub control commands")
                .build(),
            command_duration: meter
                .f64_histogram("hub_command_duration")
                .with_unit("s")
                .with_description("Processing time of a hub control command")
                .build(),
            command_queue_delay: meter
                .f64_histogram("hub_command_queue_delay")
                .with_unit("s")
                .with_description("Time a hub control command waits before it is processed")
                .build(),
            messages_sent: meter
                .u64_counter("hub_messages_sent")
                .with_description("Messages pushed to the hub connections")
                .build(),
            chat_delivered: meter
                .u64_counter("hub_chat_delivered")
                .with_description("Chat comments delivered to the connections")
                .build(),
            tick_duration: meter
                .f64_histogram("hub_tick_duration")
                .with_unit("s")
                .with_description("Duration of the ticks of the hub background consumers")
                .build(),
            ws_messages_received: meter
                .u64_counter("ws_messages_received")
                .with_description("Messages received from the websocket clients")
                .build(),
        }
    }
}
