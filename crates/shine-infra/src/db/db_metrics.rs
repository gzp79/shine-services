use opentelemetry::{global, metrics::Histogram, KeyValue};
use std::{future::Future, sync::LazyLock, time::Instant};

static DB_OPERATION_DURATION: LazyLock<Histogram<f64>> = LazyLock::new(|| {
    global::meter("shine-infra-db")
        .f64_histogram("db_client_operation_duration")
        .with_unit("s")
        .with_description("Duration of the database client operations")
        .build()
});

/// Run a database operation and record its duration. The operation name must have a low cardinality
/// (statement name, redis command), never any query parameter.
pub async fn time_db_operation<T, E, F>(system: &'static str, operation: impl AsRef<str>, future: F) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
{
    let start = Instant::now();
    let result = future.await;
    DB_OPERATION_DURATION.record(
        start.elapsed().as_secs_f64(),
        &[
            KeyValue::new("db.system", system),
            KeyValue::new("db.operation", operation.as_ref().to_owned()),
            KeyValue::new("outcome", if result.is_ok() { "ok" } else { "error" }),
        ],
    );
    result
}
