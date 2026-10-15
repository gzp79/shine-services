use opentelemetry::KeyValue;

/// Metric label.
pub fn label(key: &'static str, value: &'static str) -> KeyValue {
    KeyValue::new(key, value)
}
