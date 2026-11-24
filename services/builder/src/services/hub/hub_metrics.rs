use crate::models::messages::DisconnectReason;
use opentelemetry::{
    global,
    metrics::{Counter, Histogram, UpDownCounter},
};
use shine_infra::telemetry::{label, SECONDS_BUCKETS};
use std::{sync::LazyLock, time::Duration};

/// Metrics of the hub. Unlike the request metrics, the hub works asynchronously: control commands are queued and processed
/// by a single loop, messages are pushed to the clients and background consumers run on their own schedule.
/// Labels have a small fixed set of values, ids of users and connections are only available in the traces.
/// The instruments are private: every event has a method, so the names and labels live in one place.
pub struct HubMetrics {
    connections: UpDownCounter<i64>,
    connections_dropped: Counter<u64>,
    connects_rejected: Counter<u64>,
    control_queue_depth: UpDownCounter<i64>,
    control_commands: Counter<u64>,
    control_command_duration: Histogram<f64>,
    control_command_queue_delay: Histogram<f64>,
    messages_sent: Counter<u64>,
    registry_errors: Counter<u64>,
    tick_duration: Histogram<f64>,
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
            connects_rejected: meter
                .u64_counter("hub_connects_rejected")
                .with_description("Connection requests refused before the connection was registered")
                .build(),
            control_queue_depth: meter
                .i64_up_down_counter("hub_control_queue_depth")
                .with_description("Control commands waiting for the hub command loop")
                .build(),
            control_commands: meter
                .u64_counter("hub_control_commands")
                .with_description("Processed hub control commands")
                .build(),
            control_command_duration: meter
                .f64_histogram("hub_control_command_duration")
                .with_unit("s")
                .with_boundaries(SECONDS_BUCKETS.to_vec())
                .with_description("Processing time of a hub control command")
                .build(),
            control_command_queue_delay: meter
                .f64_histogram("hub_control_command_queue_delay")
                .with_unit("s")
                .with_boundaries(SECONDS_BUCKETS.to_vec())
                .with_description("Time a hub control command waits before it is processed")
                .build(),
            messages_sent: meter
                .u64_counter("hub_messages_sent")
                .with_description("Messages pushed to the hub connections")
                .build(),
            registry_errors: meter
                .u64_counter("hub_registry_errors")
                .with_description("Failed calls to the hub connection registry")
                .build(),
            tick_duration: meter
                .f64_histogram("hub_tick_duration")
                .with_unit("s")
                .with_boundaries(SECONDS_BUCKETS.to_vec())
                .with_description("Duration of the ticks of the hub background consumers")
                .build(),
        }
    }

    /// A connection was registered.
    pub fn connection_added(&self) {
        self.connections.add(1, &[]);
    }

    /// A connection was removed from the registry, whatever the reason.
    pub fn connection_removed(&self) {
        self.connections.add(-1, &[]);
    }

    /// A connection was removed by the hub, as opposed to closed by its client.
    pub fn connection_dropped(&self, reason: DisconnectReason) {
        self.connections_dropped.add(1, &[label("reason", reason.as_str())]);
    }

    /// A connection request was refused before it reached the hub, by reason.
    pub fn connect_rejected(&self, reason: &'static str) {
        self.connects_rejected.add(1, &[label("reason", reason)]);
    }

    /// A control command entered the queue of the command loop.
    pub fn control_queued(&self) {
        self.control_queue_depth.add(1, &[]);
    }

    /// A control command left the queue: the loop took it, or it could not be queued after all.
    pub fn control_dequeued(&self) {
        self.control_queue_depth.add(-1, &[]);
    }

    /// The command loop took a command, which waited `queue_delay` in the queue.
    pub fn control_command_started(&self, command: &'static str, queue_delay: Duration) {
        self.control_dequeued();
        self.control_command_queue_delay
            .record(queue_delay.as_secs_f64(), &[label("command", command)]);
    }

    /// The command loop finished a command.
    pub fn control_command_finished(&self, command: &'static str, duration: Duration) {
        self.control_commands.add(1, &[label("command", command)]);
        self.control_command_duration
            .record(duration.as_secs_f64(), &[label("command", command)]);
    }

    /// A message was offered to a connection's queue; `outcome` is `ok`, `full` or `closed`.
    pub fn message_sent(&self, kind: &'static str, outcome: &'static str) {
        self.messages_sent
            .add(1, &[label("kind", kind), label("outcome", outcome)]);
    }

    /// A call to the connection registry failed, by operation.
    pub fn registry_error(&self, operation: &'static str) {
        self.registry_errors.add(1, &[label("operation", operation)]);
    }

    /// A tick of a background consumer took `duration`.
    pub fn tick_finished(&self, consumer: &'static str, duration: Duration) {
        self.tick_duration
            .record(duration.as_secs_f64(), &[label("consumer", consumer)]);
    }
}
