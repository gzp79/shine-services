use opentelemetry::{
    metrics::{Counter, Meter},
    KeyValue, Value,
};
use tracing::{Event, Level, Subscriber};
use tracing_log::NormalizeEvent;
use tracing_subscriber::{layer::Context, Layer};

/// Target of the start-up self-test log lines. They exercise every level on purpose, so the counter skips them.
pub const SELF_TEST_TARGET: &str = "self_test";

/// Counts the warn and error events by level and target (module), so a rising rate can be alerted on
/// before it turns into an outage.
pub struct LogMetricsLayer {
    events: Counter<u64>,
}

impl LogMetricsLayer {
    pub fn new(meter: &Meter) -> Self {
        Self {
            events: meter
                .u64_counter("log_events")
                .with_description("Warn and error log events, by level and target")
                .build(),
        }
    }
}

impl<S: Subscriber> Layer<S> for LogMetricsLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let level = match *event.metadata().level() {
            Level::ERROR => "error",
            Level::WARN => "warn",
            _ => return,
        };
        let target = if !event.is_log() {
            event.metadata().target().to_string()
        } else {
            let normalized = event.normalized_metadata();
            normalized.as_ref().map_or("log", |m| m.target()).to_string()
        };
        if target == SELF_TEST_TARGET {
            return;
        }
        let target = Value::from(target);
        self.events
            .add(1, &[KeyValue::new("level", level), KeyValue::new("target", target)]);
    }
}
