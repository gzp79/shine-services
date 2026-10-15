use opentelemetry::{
    metrics::{Counter, Meter},
    KeyValue,
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
        // Events of the log crate carry their real target only in the normalized metadata
        let normalized = event.normalized_metadata();
        let target = normalized.as_ref().unwrap_or(event.metadata()).target();
        self.events.add(
            1,
            &[
                KeyValue::new("level", level),
                KeyValue::new("target", target.to_string()),
            ],
        );
    }
}
