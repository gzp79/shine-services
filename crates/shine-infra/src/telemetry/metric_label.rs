use opentelemetry::KeyValue;

/// Histogram bucket boundaries for durations in seconds, 0.5ms to 10s. The SDK defaults are sized for
/// milliseconds and would put every sub-5s sample into the first bucket.
pub const SECONDS_BUCKETS: &[f64] = &[
    0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Metric label.
pub fn label(key: &'static str, value: &'static str) -> KeyValue {
    KeyValue::new(key, value)
}
