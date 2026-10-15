use opentelemetry::{
    metrics::{Counter, Meter},
    KeyValue, Value,
};
use tracing::{Event, Level, Subscriber};
use tracing_log::NormalizeEvent;
use tracing_subscriber::{layer::Context, Layer};

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
            Value::from(event.metadata().target())
        } else {
            let normalized = event.normalized_metadata();
            Value::from(normalized.as_ref().map_or("log", |m| m.target()).to_string())
        };
        self.events
            .add(1, &[KeyValue::new("level", level), KeyValue::new("target", target)]);
    }
}
